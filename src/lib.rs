//! Skiplist implementation

#![deny(missing_docs)]

use std::{
    alloc::Layout,
    borrow::Borrow,
    marker::PhantomData,
    mem::{self, MaybeUninit, offset_of},
    ptr::NonNull,
};

/// This is a helper trait to improve the ergonomics of working with `Node` and `NodeHead`
/// in the same code. Using this trait, the caller can use the same methods for both types.
trait NodePtrExt<K, V> {
    unsafe fn into_head(self) -> NonNull<NodeHead<K, V>>;
    unsafe fn layout(self) -> Layout;

    /// Returns a shared reference to the forward list associated with `self`
    unsafe fn forward_list<'a>(self) -> &'a [NodePointer<K, V>];

    /// Returns an exclusive reference to the forward list associated with `self`
    unsafe fn forward_list_mut<'a>(self) -> &'a mut [NodePointer<K, V>];
}

type NodePointer<K, V> = Option<NonNull<NodeHead<K, V>>>;

#[repr(C)]
struct NodeHead<K, V> {
    level: usize,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> NodeHead<K, V> {
    pub fn new(levels: usize) -> Result<NonNull<Self>, std::alloc::LayoutError> {
        let layout = Layout::new::<Self>();
        let (layout, array_offset) = layout.extend(Layout::array::<NodePointer<K, V>>(levels)?)?;
        let layout = layout.pad_to_align();

        let the_memory = unsafe { std::alloc::alloc(layout) };
        assert!(!the_memory.is_null());
        unsafe {
            std::ptr::write(
                the_memory as *mut Self,
                NodeHead {
                    level: levels,
                    _phantom: PhantomData,
                },
            )
        };

        let array_ptr =
            unsafe { the_memory.add(array_offset) as *mut MaybeUninit<NodePointer<K, V>> };
        let array = unsafe { std::slice::from_raw_parts_mut(array_ptr, levels) };
        array.fill(MaybeUninit::new(None));

        Ok(unsafe { NonNull::new_unchecked(the_memory.cast()) })
    }

    const fn array_offset() -> usize {
        let size = size_of::<Self>();
        let align = align_of::<NodePointer<K, V>>();
        // Bitbanging array offset considering alignment of NodePointer
        // Add align - 1 to size, pushing it above an align boundry
        // Then we round down by zeroing out the least significant bits of the value
        // This gives us `size` rounded up to a multiple of `align`
        // (this of course, only works if we are working with multiples of two)
        (size + align - 1) & !(align - 1)
    }

    /// Given a valid pointer to a `NodeHead`, returns a shared reference to the forward list.
    /// An array of `NodePointer<K, V>` *must* be present after `level` in memory.
    unsafe fn forward_list_inner<'a>(head: NonNull<Self>) -> &'a [NodePointer<K, V>] {
        unsafe {
            let array_ptr: NonNull<NodePointer<K, V>> = head.byte_add(Self::array_offset()).cast();
            std::slice::from_raw_parts(array_ptr.as_ptr(), head.as_ref().level)
        }
    }

    /// Given a valid pointer to a `NodeHead`, returns an exclusive reference to the forward list.
    /// An array of `NodePointer<K, V>` *must* be present after `level` in memory.
    unsafe fn forward_list_mut_inner<'a>(head: NonNull<Self>) -> &'a mut [NodePointer<K, V>] {
        unsafe {
            let array_ptr: NonNull<NodePointer<K, V>> = head.byte_add(Self::array_offset()).cast();
            std::slice::from_raw_parts_mut(array_ptr.as_ptr(), head.as_ref().level)
        }
    }
}

impl<K, V> NodePtrExt<K, V> for NonNull<NodeHead<K, V>> {
    unsafe fn forward_list<'a>(self) -> &'a [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_inner(self) }
    }

    unsafe fn forward_list_mut<'a>(self) -> &'a mut [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_mut_inner(self) }
    }

    unsafe fn into_head(self) -> NonNull<NodeHead<K, V>> {
        self
    }

    unsafe fn layout(self) -> Layout {
        unsafe {
            let levels = self.as_ref().level;
            let layout = Layout::new::<NodeHead<K, V>>();
            let (layout, _) = layout
                .extend(Layout::array::<NodePointer<K, V>>(levels).unwrap_unchecked())
                .unwrap_unchecked();
            layout.pad_to_align()
        }
    }
}

#[repr(C)]
struct Node<K, V> {
    key: K,
    value: V,
    head: NodeHead<K, V>, // last field — forward array follows in memory
}

impl<K, V> Node<K, V> {
    /// Creates a skip list node with the given parameters.
    ///
    /// An array of `levels` levels will be allocated after the [`NodeHead`] field.
    fn new(key: K, value: V, levels: usize) -> NonNull<Self> {
        assert_ne!(levels, 0, "node must have at least 1 level");
        let (layout, array_offset) = Self::layout(levels).unwrap();

        // SAFETY: `the_memory` is a valid Node-sized allocation + an [NodePointer; levels]
        let node = unsafe {
            let the_memory = std::alloc::alloc(layout);
            std::ptr::write(
                the_memory as *mut Self,
                Self {
                    key,
                    value,
                    head: NodeHead::<K, V> {
                        level: levels,
                        _phantom: PhantomData,
                    },
                },
            );

            let array_ptr = the_memory.add(array_offset) as *mut MaybeUninit<NodePointer<K, V>>;
            let array = std::slice::from_raw_parts_mut(array_ptr, levels);
            array.fill(MaybeUninit::new(None));

            the_memory as *mut Self
        };
        NonNull::new(node).expect("allocation was null")
    }

    /// Converts a [`NodeHead`] pointer to a [`Node`] pointer.
    ///
    /// The [`NodeHead`] must have been allocated as part of a [`Node`].
    unsafe fn from_head(head: NonNull<NodeHead<K, V>>) -> NonNull<Self> {
        unsafe { head.byte_sub(offset_of!(Node<K, V>, head)).cast() }
    }

    fn layout(levels: usize) -> Result<(Layout, usize), std::alloc::LayoutError> {
        let layout = Layout::new::<Node<K, V>>();
        let (layout, offset) = layout.extend(Layout::array::<NodePointer<K, V>>(levels)?)?;
        Ok((layout.pad_to_align(), offset))
    }
}

impl<K, V> NodePtrExt<K, V> for NonNull<Node<K, V>> {
    unsafe fn forward_list<'a>(self) -> &'a [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_inner(self.into_head()) }
    }

    unsafe fn forward_list_mut<'a>(self) -> &'a mut [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_mut_inner(self.into_head()) }
    }
    unsafe fn into_head(self) -> NonNull<NodeHead<K, V>> {
        unsafe { self.byte_add(offset_of!(Node<K, V>, head)).cast() }
    }

    unsafe fn layout(self) -> Layout {
        unsafe {
            let levels = self.as_ref().head.level;
            Node::<K, V>::layout(levels).unwrap_unchecked().0
        }
    }
}

/// This is an implementation of the skip-list data structure from
/// [Pugh (1990)](https://doi.org/10.1145/78973.78977) in Rust.
///
/// TODO: finish this
pub struct SkipList<K, V> {
    len: usize,
    level: usize,
    head: NonNull<NodeHead<K, V>>,
}

impl<K, V> SkipList<K, V> {
    const MAX_LEVEL: usize = 29;
    const P: f64 = 0.25;

    /// Returns a random level sampled from a geometric distribution, clamped at `MAX_LEVEL`.
    fn random_level(&mut self) -> usize {
        let u: f64 = rand::random();
        let level = (u.ln() / Self::P.ln()) as usize + 1;
        level.min(Self::MAX_LEVEL)
    }
}

impl<K, V> SkipList<K, V> {
    /// Creates an empty skip list.
    pub fn new() -> Self {
        SkipList {
            len: 0,
            level: 1,
            // TODO: Further optimization: place NodeHead after the `level` field
            head: NodeHead::new(Self::MAX_LEVEL).unwrap(),
        }
    }

    /// Returns the number of items in the list.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the current number of levels in the skip list.
    pub fn level(&self) -> usize {
        self.level
    }

    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, V> std::fmt::Display for SkipList<K, V>
where
    K: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let forward_list = unsafe { self.head.forward_list() };
        f.write_str(
            forward_list
                .iter()
                .enumerate()
                .rev()
                .fold("".to_string(), |acc, item| {
                    let (i, mut cur_node) = item;
                    if cur_node.is_none() {
                        //return acc + "None\n";
                        return acc;
                    }

                    let mut temp = String::new();
                    while let Some(node) = cur_node {
                        cur_node = &unsafe { node.forward_list() }[i];
                        let node = unsafe { Node::from_head(*node).as_ref() };
                        temp += &format!("-> {:?}", node.key);
                    }
                    temp += "-> None";
                    acc + &temp + "\n"
                })
                .as_str(),
        )
    }
}

impl<K, V> SkipList<K, V>
where
    K: Ord + Eq,
{
    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, [`None`] is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical. See [std::collections]
    /// for more details.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // TODO make array
        let mut update = vec![self.head; Self::MAX_LEVEL];
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { *x.forward_list().get_unchecked(i) } {
                if unsafe { Node::from_head(next_node).as_ref() }.key < key {
                    x = next_node;
                } else {
                    break;
                }
            }

            update[i] = x;
        }

        if let Some(x) = unsafe { *x.forward_list().get_unchecked(0) }
            && unsafe { Node::from_head(x).as_ref() }.key == key
        {
            unsafe {
                let x: &mut Node<K, V> = Node::from_head(x).as_mut();
                let old_value = mem::replace(&mut x.value, value);
                Some(old_value)
            }
        } else {
            let new_level = self.random_level();
            if new_level > self.level() {
                update
                    .iter_mut()
                    .skip(self.level())
                    .take(new_level)
                    .for_each(|e| {
                        *e = self.head;
                    });
                self.level = new_level;
            }

            let new_node = Node::new(key, value, new_level);
            for i in 0..new_level {
                let target_node = *unsafe { update.get_unchecked(i) };

                unsafe {
                    *new_node.forward_list_mut().get_unchecked_mut(i) =
                        *target_node.forward_list().get_unchecked(i);
                    *target_node.forward_list_mut().get_unchecked_mut(i) =
                        Some(new_node.into_head());
                }
            }
            self.len += 1;
            None
        }
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + ?Sized,
    {
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            // SAFETY: `x` is a valid `NodeHead` with a forward list
            while let Some(next_node) = unsafe { *x.forward_list().get_unchecked(i) } {
                if unsafe { Node::from_head(next_node).as_ref() }.key.borrow() < key {
                    x = next_node;
                } else {
                    break;
                }
            }
        }

        match unsafe { x.forward_list() }[0] {
            Some(x) => {
                // Safety: All nodes after the sentinel node are instances of [`Node`]
                let node = unsafe { Node::from_head(x).as_ref() };
                if node.key.borrow() == key {
                    Some(&node.value)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Deletes a key from the map, returning the value at the key if the key was found in the map.
    /// Returns [`None`] otherwise.
    pub fn delete<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + ?Sized,
    {
        let mut update = vec![self.head; Self::MAX_LEVEL];
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            // SAFETY: `self.head` is a valid `NodeHead` AND all children are recursively guaranteed
            // to be valid `Node`s
            while let Some(next_node) = unsafe { *x.forward_list().get_unchecked(i) } {
                // SAFETY: Only the sentinel node is only a `NodeHead`, all other `NodeHead`s are
                // also `Node`s.
                if unsafe { Node::from_head(next_node).as_ref() }.key.borrow() < key {
                    x = next_node;
                } else {
                    break;
                }
            }

            update[i] = x;
        }

        // SAFETY: From previous constriants, `x` is a valid `NodeHead`
        if let Some(x) = unsafe { *x.forward_list().get_unchecked(0) }
            // SAFETY: `x` cannot be the sentinel head, therefore it must also be a `Node`
            && unsafe { Node::from_head(x).as_ref() }.key.borrow() == key
        {
            for i in 0..level {
                // SAFETY: `update` contains all valid nodes
                let source = unsafe { update.get_unchecked(i) };
                let target_node = unsafe { source.forward_list_mut().get_unchecked_mut(i) };
                if let Some(t) = target_node
                    && t.as_ptr() == x.as_ptr()
                {
                    // SAFETY: `x` is a valid node
                    let new = unsafe { *x.forward_list().get_unchecked(i) };

                    let _ = mem::replace(target_node, new);
                } else {
                    break;
                }
            }

            while self.level > 1
                && unsafe { self.head.forward_list().get_unchecked(self.level - 1) }.is_none()
            {
                self.level -= 1;
            }

            // SAFETY: All `NodeHead` instances here are part of a `Node`.
            unsafe {
                let node = Node::from_head(x);
                let value =
                    std::ptr::read(node.as_ptr().byte_add(offset_of!(Node<K, V>, value)).cast());
                std::ptr::drop_in_place(
                    node.as_ptr()
                        .byte_add(offset_of!(Node<K, V>, key))
                        .cast::<K>(),
                );
                let layout = node.layout();
                std::alloc::dealloc(node.as_ptr().cast(), layout);

                self.len -= 1;
                Some(value)
            }
        } else {
            None
        }
    }
}

impl<K, V> Drop for SkipList<K, V> {
    fn drop(&mut self) {
        // SAFETY: Head is always a valid `NodeHead`
        let mut maybe_next_node = unsafe { self.head.forward_list() }[0];
        while let Some(node_ptr) = maybe_next_node {
            // SAFETY: Every NodeHead that isnt `self.head` is also a `Node`
            let node = unsafe { Node::from_head(node_ptr) };

            // SAFETY: node is a pointer to a valid Node
            unsafe {
                maybe_next_node = node.forward_list()[0];
                let layout = node.layout();
                node.drop_in_place();
                std::alloc::dealloc(node.as_ptr().cast(), layout);
            }
        }

        // SAFETY: self.head is a valid `NodeHead` instance with corresponding layout
        unsafe {
            std::alloc::dealloc(self.head.as_ptr().cast(), self.head.layout());
        }
    }
}

impl<K, V> Default for SkipList<K, V> {
    fn default() -> Self {
        SkipList::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_layout() {
        let (layout, _) = Node::<(), ()>::layout(10).unwrap();
        assert_eq!(layout.size(), 88);

        let (layout, _) = Node::<(), ()>::layout(11).unwrap();
        assert_eq!(layout.size(), 96);

        let (layout, _) = Node::<(), ()>::layout(1).unwrap();
        assert_eq!(layout.size(), 16);

        let (layout, offset) = Node::<i32, u32>::layout(10).unwrap();
        assert_eq!(layout.size(), 80 + offset);

        let (layout, offset) = Node::<i32, u32>::layout(11).unwrap();
        assert_eq!(layout.size(), 88 + offset);

        let (layout, offset) = Node::<i32, u32>::layout(1).unwrap();
        assert_eq!(layout.size(), 8 + offset);
    }

    #[test]
    #[should_panic]
    fn node_zero_levels() {
        let _ = Node::new((), (), 0);
    }

    #[test]
    fn node_drop() {
        type Node1 = Node<String, &'static str>;
        for levels in 1..10 {
            let node1 = Node1::new("asdf".into(), "asdf", levels);
            assert_eq!(size_of_val(&node1), 8);
            unsafe {
                let layout = Node1::layout(levels).unwrap().0;
                node1.drop_in_place();
                std::alloc::dealloc(node1.as_ptr().cast(), layout);
            }
        }
    }

    #[test]
    fn default() {
        let mut default_list: SkipList<i32, String> = Default::default();
        assert!(default_list.is_empty());
        assert_eq!(default_list.len(), 0);

        default_list.insert(3i32, "one".into());
        default_list.insert(3i32, "two".into());
        default_list.insert(3i32, "hello, world!".into());
        assert!(!default_list.is_empty());
        assert_eq!(default_list.len(), 1);
        let result = default_list.get(&3i32);
        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value, &"hello, world!".to_string());
    }

    #[test]
    fn test_insert_many() {
        let mut list: SkipList<String, u64> = SkipList::new();
        let starting_value = 3u64;

        for i in 0..100 {
            let value = starting_value + i * 67;
            let key = value.to_string();
            list.insert(key.clone(), value);
            assert!(list.get(&key).is_some());
        }

        // Now we let list drop without any `delete`ing and see if miri complains
    }

    #[test]
    fn test_insert_and_remove_many() {
        let mut list: SkipList<String, u64> = SkipList::new();
        let starting_value = 3u64;
        let how_many = 10;

        for i in 0..how_many {
            let value = starting_value + i * 3737;
            let key = value.to_string();
            list.insert(key.clone(), value);
            assert_eq!(list.get(&key).unwrap(), &value);
            assert_eq!(list.len() as u64, i + 1);
        }

        for i in 0..how_many {
            let expected_value = starting_value + i * 3737;
            let key = expected_value.to_string();
            assert_eq!(list.get(&key).unwrap(), &expected_value);
        }

        for i in 0..how_many {
            let expected_value = starting_value + i * 3737;
            let key = expected_value.to_string();
            assert_eq!(list.delete(&key).unwrap(), expected_value);
            assert_eq!(list.len() as u64, how_many - 1 - i);
        }

        assert_eq!(list.len(), 0);
        assert_eq!(list.level(), 1);
    }

    #[test]
    fn super_alignment() {
        #[repr(C, align(128))]
        #[derive(Debug, PartialEq, Eq)]
        struct Value {
            pub value: u64,
            pub data: [u64; 100]
        }

        let mut list: SkipList<String, Value> = SkipList::new();
        for i in 0..10 {
            list.insert(i.to_string(), Value { value: i, data: [0; 100] });
        }

        for i in 0..10 {
            let key = i.to_string();
            assert_eq!(list.delete(&key).unwrap(), Value { value: i, data: [0; 100] });
            assert_eq!(list.len() as u64, 9 - i);
        }

        assert_eq!(list.len(), 0);
        assert_eq!(list.level(), 1);
    }
}
