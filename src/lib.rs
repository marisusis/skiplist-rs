use std::{
    alloc::Layout,
    borrow::Borrow,
    marker::PhantomData,
    mem::{self, MaybeUninit, offset_of},
    ptr::NonNull,
};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

// struct Link<K, V> {
//     width: usize,
//     target: NonNull<Node<K, V>>,
// }
//

trait ForwardList<K, V> {
    unsafe fn forward_list<'a>(self) -> &'a [NodePointer<K, V>];
    unsafe fn forward_list_mut<'a>(self) -> &'a mut [NodePointer<K, V>];
}

trait NodePtrExt<K, V> {
    unsafe fn into_head(self) -> NonNull<NodeHead<K, V>>;
    unsafe fn layout(self) -> Layout;
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

        unsafe {
            let the_memory = std::alloc::alloc(layout);
            assert!(!the_memory.is_null());
            std::ptr::write(
                the_memory as *mut Self,
                NodeHead {
                    level: levels,
                    _phantom: PhantomData,
                },
            );

            let array_ptr = the_memory.add(array_offset) as *mut MaybeUninit<NodePointer<K, V>>;
            let array = std::slice::from_raw_parts_mut(array_ptr, levels);
            array.fill(MaybeUninit::new(None));

            Ok(NonNull::new_unchecked(the_memory.cast()))
        }
    }

    const fn array_offset() -> usize {
        let size = size_of::<Self>();
        let align = align_of::<NodePointer<K, V>>();
        (size + align - 1) & !(align - 1)
    }

    /// Array of `NodePointer<K, V>` must be present after `level` in memory
    unsafe fn forward_list_inner<'a>(head: NonNull<Self>) -> &'a [NodePointer<K, V>] {
        unsafe {
            let array_ptr: NonNull<NodePointer<K, V>> = head.byte_add(Self::array_offset()).cast();
            std::slice::from_raw_parts(array_ptr.as_ptr(), head.as_ref().level)
        }
    }

    /// Array of `NodePointer<K, V>` must be present after `level` in memory
    unsafe fn forward_list_mut_inner<'a>(head: NonNull<Self>) -> &'a mut [NodePointer<K, V>] {
        unsafe {
            let array_ptr: NonNull<NodePointer<K, V>> = head.byte_add(Self::array_offset()).cast();
            std::slice::from_raw_parts_mut(array_ptr.as_ptr(), head.as_ref().level)
        }
    }
}

impl<K, V> ForwardList<K, V> for NonNull<NodeHead<K, V>> {
    unsafe fn forward_list<'a>(self) -> &'a [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_inner(self) }
    }

    unsafe fn forward_list_mut<'a>(self) -> &'a mut [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_mut_inner(self) }
    }
}

impl<K, V> NodePtrExt<K, V> for NonNull<NodeHead<K, V>> {
    unsafe fn into_head(self) -> NonNull<NodeHead<K, V>> {
        self
    }

    unsafe fn layout(self) -> Layout {
        unsafe {
            let levels = self.as_ref().level;
            let layout = Layout::new::<Self>();
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

    /// The `NodeHead` must have been allocated as part of a `Node`.
    unsafe fn from_head(head: NonNull<NodeHead<K, V>>) -> NonNull<Self> {
        unsafe { head.byte_sub(offset_of!(Node<K, V>, head)).cast() }
    }

    fn layout(levels: usize) -> Result<(Layout, usize), std::alloc::LayoutError> {
        let layout = Layout::new::<Self>();
        let (layout, offset) = layout.extend(Layout::array::<NodePointer<K, V>>(levels)?)?;
        Ok((layout.pad_to_align(), offset))
    }
}

impl<K, V> ForwardList<K, V> for NonNull<Node<K, V>> {
    unsafe fn forward_list<'a>(self) -> &'a [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_inner(self.into_head()) }
    }

    unsafe fn forward_list_mut<'a>(self) -> &'a mut [NodePointer<K, V>] {
        unsafe { NodeHead::forward_list_mut_inner(self.into_head()) }
    }
}

impl<K, V> NodePtrExt<K, V> for NonNull<Node<K, V>> {
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

// impl<K, V> std::fmt::Display for Node<K, V>
// where
//     K: std::fmt::Debug,
//     V: std::fmt::Debug,
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.write_str(format!("({:?}, {:?})", self.key, self.value).as_str())
//     }
// }

pub struct SkipList<K, V> {
    len: usize,
    level: usize,
    head: NonNull<NodeHead<K, V>>,
}

/// Private fields
impl<K, V> SkipList<K, V> {
    const MAX_LEVEL: usize = 29;
    const P: f64 = 0.2;

    fn random_level(&mut self) -> usize {
        let mut new_level = 1;
        while rand::random::<f64>() < Self::P {
            new_level += 1;
        }

        new_level.min(Self::MAX_LEVEL)
    }
}

impl<K, V> SkipList<K, V> {
    pub fn new() -> Self {
        SkipList {
            len: 0,
            level: 0,
            head: NodeHead::new(Self::MAX_LEVEL).unwrap(),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn level(&self) -> usize {
        self.level.max(1)
    }

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
    V: Unpin,
{
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // TODO make array
        let mut update = vec![self.head; Self::MAX_LEVEL];
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { x.forward_list() }[i] {
                if unsafe { Node::from_head(next_node).as_ref() }.key < key {
                    x = next_node;
                } else {
                    break;
                }
            }

            update[i] = x;
        }

        if let Some( x) = unsafe { x.forward_list() }[0]
            && unsafe { Node::from_head(x).as_ref() }.key == key
        {
            unsafe {
                let x: &mut Node<K, V> = Node::from_head(x).as_mut();
                let old_value = mem::replace(&mut x.value, value);
                Some(old_value)
                // TODO: return old value
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
                let target_node = update[i];
                // println!(
                //     "level {i} new_level {new_level}\nis header {}\n{}",
                //     target_list.as_ptr() == self.forward.as_ptr(),
                //     target_list.display_forward()
                // );
                unsafe {
                    new_node.forward_list_mut()[i] = target_node.forward_list()[i];
                    target_node.forward_list_mut()[i] = Some(new_node.into_head());
                }
            }
            self.len += 1;
            None
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + ?Sized,
    {
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { x.forward_list() }[i] {
                if unsafe { Node::from_head(next_node).as_ref() }.key.borrow() < key {
                    x = next_node;
                } else {
                    break;
                }
            }
        }

        match unsafe { x.forward_list() }[0] {
            Some(x) => {
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

    pub fn delete(&mut self, key: K) -> Option<V>
    where
        V: Unpin,
    {
        let mut update = vec![self.head; Self::MAX_LEVEL];
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { x.forward_list() }[i] {
                if unsafe { Node::from_head(next_node).as_ref() }.key < key {
                    x = next_node;
                } else {
                    break;
                }
            }

            update[i] = x;
        }

        if let Some(x) = unsafe { x.forward_list() }[0]
            && unsafe { Node::from_head(x).as_ref() }.key == key
        {
            for i in 0..level {
                let target_node = &mut unsafe { update[i].forward_list_mut() }[i];
                if let Some(t) = target_node
                    && t.as_ptr() == x.as_ptr()
                {
                    let _ = mem::replace(target_node, unsafe { x.forward_list() }[i]);
                } else {
                    break;
                }
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

                // TODO: a better way?
                self.level = self
                    .head
                    .forward_list()
                    .iter()
                    .filter(|e| e.is_some())
                    .count();
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
    use rand::{RngExt, SeedableRng as _, seq::IteratorRandom};
    use std::collections::HashMap;

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

        default_list.insert(3i32, "asdf".into());
        default_list.insert(3i32, "fdsa".into());
        default_list.insert(3i32, "hello, world!".into());
        assert!(!default_list.is_empty());
        assert_eq!(default_list.len(), 1);
        let result = default_list.get(&3i32);
        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value, &"hello, world!".to_string());
    }

    #[test]
    fn random_inserts() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1284);
        let mut rng = rand::rng();
        let mut list = SkipList::new();

        let items = 10;

        // TODO Fix collisions
        let mut random_items = HashMap::with_capacity(items);
        for i in 0..items {
            let key: u64 = rng.random_range(0..30);
            random_items.insert(key, i);
            list.insert(key, i);
            eprintln!("{list}");
        }

        // Verify we can find all the items
        random_items
            .iter()
            .choose(&mut rng)
            .into_iter()
            .for_each(|(key, value)| {
                let result = list.get(key);
                let found_value = result.expect("list should contain this key");
                assert_eq!(*value, *found_value);
            });

        let mut deleted_items = Vec::with_capacity(items);

        // delete them one at a time

        eprintln!("{list}");
        let total_items = random_items.len();
        random_items.into_iter().for_each(|item| {
            deleted_items.push(item);
            eprintln!("deleting {}", item.0);
            list.delete(item.0).unwrap();
            eprintln!("deleted {}", item.0);
            eprintln!("{list}");

            for deleted in deleted_items.iter() {
                assert!(list.get(&deleted.0).is_none(), "item still exists");
                assert_eq!(list.len(), total_items - deleted_items.len());
            }
        });

        eprintln!("{list}");
        //panic!("WHAT IS HAPPENING");

        // empty list
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
        assert_eq!(list.level(), 1);
    }

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
