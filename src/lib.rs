use std::{mem::MaybeUninit, ptr::NonNull};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

// struct Link<K, V> {
//     width: usize,
//     target: NonNull<Node<K, V>>,
// }
//

type NodePointer<K, V> = Option<NonNull<Node<K, V>>>;

struct Node<K, V> {
    key: K,
    value: V,
    forward: Box<[NodePointer<K, V>]>,
}

impl<K, V> Node<K, V> {
    fn new(key: K, value: V, forward: &[NodePointer<K, V>]) -> Self {
        Node {
            key,
            value,
            forward: Box::from(forward),
        }
    }

    #[allow(unused)]
    fn level(&self) -> usize {
        if self.forward.is_empty() {
            1
        } else {
            self.forward.len()
        }
    }
}

impl<K, V> Default for Node<K, V>
where
    K: Default,
    V: Default,
{
    fn default() -> Self {
        Self {
            key: Default::default(),
            value: Default::default(),
            forward: Default::default(),
        }
    }
}

impl<K, V> std::fmt::Display for Node<K, V>
where
    K: std::fmt::Debug,
    V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("({:?}, {:?})", self.key, self.value).as_str())
    }
}

#[derive(Debug)]
pub struct NotFound {}

#[allow(unused)]
trait DisplayForwardExt {
    fn display_forward(&self) -> String;
}

impl<K, V> DisplayForwardExt for [Option<NonNull<Node<K, V>>>]
where
    K: std::fmt::Debug,
    V: std::fmt::Debug,
{
    fn display_forward(&self) -> String {
        self.iter()
            .enumerate()
            .fold("".to_string(), |acc, e| match e {
                (i, Some(ptr)) => {
                    let node = unsafe { ptr.as_ref() };
                    acc + format!("level {i}: {node}\n").as_str()
                }
                (i, None) => acc + format!("level {i}: ---\n").as_str(),
            })
    }
}

pub struct SkipList<K, V> {
    len: usize,
    forward: Vec<NodePointer<K, V>>,
}

/// Private fields
impl<K, V> SkipList<K, V> {
    const MAX_LEVEL: usize = 29;
    const P: f64 = 0.5;

    fn random_level() -> usize {
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
            forward: vec![None; Self::MAX_LEVEL],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn level(&self) -> usize {
        self.forward.iter().filter(|e| e.is_some()).count().max(1)
    }

    pub fn is_empty(&self) -> bool {
        self.forward.iter().all(|e| e.is_none())
    }
}

impl<K, V> std::fmt::Display for SkipList<K, V>
where
    K: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            self.forward
                .iter()
                .enumerate()
                .fold("".to_string(), |acc, item| {
                    let (i, mut cur_node) = item;

                    let mut temp = String::new();
                    while let Some(node) = cur_node {
                        let node = unsafe { node.as_ref() };
                        cur_node = &node.forward[i];
                        temp += &format!("-> {:?}", node.key);
                    }

                    acc + &temp + "\n"
                })
                .as_str(),
        )
    }
}

impl<K, V> SkipList<K, V>
where
    K: Ord + Eq + std::fmt::Debug,
    V: std::fmt::Debug,
{
    pub fn insert(&mut self, key: K, value: V) {
        // TODO make array
        let mut update: Vec<MaybeUninit<NonNull<[NodePointer<K, V>]>>> =
            vec![MaybeUninit::uninit(); Self::MAX_LEVEL];
        let level = self.level();
        let mut x_forward = self.forward.as_mut_slice();

        for i in (0..level).rev() {
            // TODO unwrap unchecked
            while let Some(mut ptr) = x_forward[i] {
                let node = unsafe { ptr.as_mut() };
                if node.key < key {
                    x_forward = node.forward.as_mut();
                } else {
                    break;
                }
            }
            update[i].write(x_forward.into());
        }

        if let Some(mut x) = x_forward[0]
            && unsafe { x.as_ref() }.key == key
        {
            unsafe { x.as_mut() }.value = value;
        } else {
            let new_level = Self::random_level();
            if new_level > self.level() {
                update
                    .iter_mut()
                    .skip(self.level())
                    .take(new_level)
                    .for_each(|e| {
                        e.write(self.forward.as_mut_slice().into());
                    });
            }

            let new_node = Box::new(Node::new(key, value, &[None]));
            let new_node = NonNull::new(Box::leak(new_node));
            let mut new_forward = Vec::with_capacity(self.level());
            for i in 0..new_level {
                let target_list = unsafe { update[i].assume_init_mut().as_mut() };
                // println!(
                //     "level {i} new_level {new_level}\nis header {}\n{}",
                //     target_list.as_ptr() == self.forward.as_ptr(),
                //     target_list.display_forward()
                // );
                let target_node = target_list[i];
                new_forward.push(target_node);
                target_list[i] = new_node;
            }
            let new_node = unsafe { new_node.unwrap().as_mut() };
            new_node.forward = new_forward.into_boxed_slice();
        }
        self.len += 1;
    }

    pub fn search(&self, key: K) -> Option<(K, &V)> {
        let level = self.level();
        let mut x_forward = self.forward.as_slice();

        for i in (0..level).rev() {
            // println!("[forward list]\n{}", x_forward.display_forward());
            while let Some(ptr) = x_forward[i] {
                let node = unsafe { ptr.as_ref() };
                if node.key < key {
                    x_forward = node.forward.as_ref();
                } else {
                    break;
                }
            }
        }

        match x_forward[0] {
            Some(x) => {
                let x = unsafe { x.as_ref() };
                if x.key == key {
                    Some((key, &x.value))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn delete(&mut self, key: K) -> Result<V, NotFound> {
        // TODO make array
        let mut update: Vec<MaybeUninit<NonNull<[NodePointer<K, V>]>>> =
            vec![MaybeUninit::uninit(); Self::MAX_LEVEL];
        let level = self.level();
        let mut x_forward = self.forward.as_mut_slice();

        for i in (0..level).rev() {
            // TODO unwrap unchecked
            while let Some(mut ptr) = x_forward[i] {
                let node = unsafe { ptr.as_mut() };
                if node.key < key {
                    x_forward = node.forward.as_mut();
                } else {
                    break;
                }
            }
            update[i].write(x_forward.into());
        }

        if let Some(x) = x_forward[0]
            && unsafe { x.as_ref() }.key == key
        {
            for i in 0..level {
                let target_list = unsafe { update[i].assume_init_mut().as_mut() };
                let target_node = target_list[i];
                if let Some(target_node) = target_node
                    && target_node.as_ptr() == x.as_ptr()
                {
                    target_list[i] = unsafe { x.as_ref() }.forward[i];
                } else {
                    break;
                }
            }
            //let value = unsafe { ManuallyDrop::take(&mut (*x.as_ptr()).value) };
            let x = unsafe { Box::from_raw(x.as_ptr()) };

            Ok(x.value)
        } else {
            Err(NotFound {})
        }
    }
}

impl<K, V> Drop for SkipList<K, V> {
    fn drop(&mut self) {
        eprintln!("TODO: impl Drop for SkipList");
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
    use rand::seq::IteratorRandom;
    use std::collections::HashMap;

    #[test]
    fn default() {
        let mut default_list: SkipList<i32, String> = Default::default();
        assert!(default_list.is_empty());
        assert_eq!(default_list.len(), 0);

        default_list.insert(3i32, "asdf".into());
        default_list.insert(3i32, "fdsa".into());
        default_list.insert(3i32, "hello, world!".into());
        println!("{}", default_list.forward.display_forward());
        assert!(!default_list.is_empty());
        assert_eq!(default_list.len(), 3);
        let result = default_list.search(3i32);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, 3i32);
        assert_eq!(value, &"hello, world!".to_string());
    }

    #[test]
    fn random_inserts() {
        let mut list = SkipList::new();

        let items = 1000;

        // TODO Fix collisions
        let mut random_items = HashMap::with_capacity(items);
        for i in 0..items {
            let key: u64 = rand::random_range(0..3000);
            random_items.insert(key, i);
            list.insert(key, i);
        }

        // Verify we can find all the items
        random_items
            .iter()
            .choose(&mut rand::rng())
            .into_iter()
            .for_each(|(key, value)| {
                let result = list.search(*key);
                let (found_key, found_value) = result.expect("list should contain this key");
                assert_eq!(*key, found_key);
                assert_eq!(*value, *found_value);
            });

        let mut deleted_items = Vec::with_capacity(items);

        // delete them one at a time

        eprintln!("{list}");
        random_items.into_iter().for_each(|item| {
            deleted_items.push(item);
            eprintln!("deleting {}", item.0);
            list.delete(item.0).unwrap();

            for deleted in deleted_items.iter() {
                assert!(list.search(deleted.0).is_none(), "item still exists");
            }
        });

        eprintln!("{list}");

        // empty list
        assert!(list.is_empty());
        assert_eq!(list.level(), 1);
    }

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
