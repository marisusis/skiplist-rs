use rand::SeedableRng;
use std::{
    mem::{self, MaybeUninit},
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

type NodePointer<K, V> = Option<NonNull<Node<K, V>>>;

struct Node<K, V> {
    key: MaybeUninit<K>,
    value: MaybeUninit<V>,
    forward: Box<[NodePointer<K, V>]>,
}

impl<K, V> Node<K, V> {
    fn new(key: K, value: V, levels: usize) -> Self {
        Node {
            key: MaybeUninit::new(key),
            value: MaybeUninit::new(value),
            forward: vec![None; levels].into_boxed_slice(),
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
    head: NonNull<Node<K, V>>,
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

    fn node(&self) -> &Node<K, V> {
        self.as_ref()
    }
}

impl<K, V> AsRef<Node<K, V>> for SkipList<K, V> {
    fn as_ref(&self) -> &Node<K, V> {
        unsafe { self.head.as_ref() }
    }
}

impl<K, V> SkipList<K, V> {
    pub fn new() -> Self {
        SkipList {
            len: 0,
            head: NonNull::from(Box::leak(Box::new(Node {
                key: MaybeUninit::uninit(),
                value: MaybeUninit::uninit(),
                forward: vec![None; Self::MAX_LEVEL].into_boxed_slice(),
            }))),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn level(&self) -> usize {
        unsafe { self.head.as_ref() }
            .forward
            .iter()
            .filter(|e| e.is_some())
            .count()
            .max(1)
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
        f.write_str(
            self.as_ref()
                .forward
                .iter()
                .enumerate()
                .fold("".to_string(), |acc, item| {
                    let (i, mut cur_node) = item;

                    let mut temp = String::new();
                    while let Some(node) = cur_node {
                        let node = unsafe { node.as_ref() };
                        cur_node = &node.forward[i];
                        temp += &format!("-> {:?}", unsafe { node.key.assume_init_ref() });
                    }

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
    pub fn insert(&mut self, key: K, value: V) {
        // TODO make array
        let mut update = vec![self.head; Self::MAX_LEVEL];
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { x.as_ref() }.forward[i] {
                if *unsafe { next_node.as_ref().key.assume_init_ref() } < key {
                    x = next_node;
                } else {
                    break;
                }
            }

            update[i] = x;
        }

        if let Some(mut x) = unsafe { x.as_ref() }.forward[0]
            && *unsafe { x.as_ref().key.assume_init_ref() } == key
        {
            unsafe {
                let x: &mut Node<K, V> = x.as_mut();
                x.value.assume_init_drop();
                // TODO return old value?
                x.value.write(value);
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
            }

            let mut new_node = NonNull::from(Box::leak(Box::new(Node::new(key, value, new_level))));
            for i in 0..new_level {
                let mut target_node = update[i];
                // println!(
                //     "level {i} new_level {new_level}\nis header {}\n{}",
                //     target_list.as_ptr() == self.forward.as_ptr(),
                //     target_list.display_forward()
                // );
                unsafe { new_node.as_mut() }.forward[i] =
                    unsafe { target_node.as_ref() }.forward[i];
                unsafe { target_node.as_mut() }.forward[i] = Some(new_node);
            }
            self.len += 1;
        }
    }

    pub fn search(&self, key: K) -> Option<(K, &V)> {
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { x.as_ref() }.forward[i] {
                if *unsafe { next_node.as_ref().key.assume_init_ref() } < key {
                    x = next_node;
                } else {
                    break;
                }
            }
        }

        match unsafe { x.as_ref() }.forward[0] {
            Some(x) => {
                let x = unsafe { x.as_ref() };
                if *unsafe { x.key.assume_init_ref() } == key {
                    Some((key, unsafe { x.value.assume_init_ref() }))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn delete(&mut self, key: K) -> Option<V> {
        let mut update = vec![self.head; Self::MAX_LEVEL];
        let level = self.level();

        let mut x = self.head;

        for i in (0..level).rev() {
            while let Some(next_node) = unsafe { x.as_ref() }.forward[i] {
                if *unsafe { next_node.as_ref().key.assume_init_ref() } < key {
                    x = next_node;
                } else {
                    break;
                }
            }

            update[i] = x;
        }

        if let Some(x) = unsafe { x.as_ref() }.forward[0]
            && *unsafe { x.as_ref().key.assume_init_ref() } == key
        {
            for i in 0..level {
                let target_node = &mut unsafe { update[i].as_mut() }.forward[i];
                if let Some(t) = target_node
                    && t.as_ptr() == x.as_ptr()
                {
                    let old = mem::replace(target_node, unsafe { x.as_ref() }.forward[i]);
                } else {
                    break;
                }
            }

            let mut node: Box<Node<K, V>> = unsafe { Box::from_raw(x.as_ptr()) };
            unsafe { node.key.assume_init_drop() };
            self.len -= 1;

            Some(unsafe { node.value.assume_init_read() })
        } else {
            None
        }
    }
}

impl<K, V> Drop for SkipList<K, V> {
    fn drop(&mut self) {
        eprintln!("TODO: impl Drop for SkipList");
        let mut maybe_next_node = unsafe { self.head.as_ref() }.forward[0];
        while let Some(node_ptr) = maybe_next_node {
            let mut node: Box<Node<K, V>> = unsafe { Box::from_raw(node_ptr.as_ptr()) };
            unsafe {
                node.key.assume_init_drop();
                node.value.assume_init_drop();
            }
            maybe_next_node = node.forward[0];
        }
        let _ = unsafe { Box::from_raw(self.head.as_ptr()) };
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
    use rand::{RngExt, seq::IteratorRandom};
    use std::collections::HashMap;

    #[test]
    fn default() {
        let mut default_list: SkipList<i32, String> = Default::default();
        assert!(default_list.is_empty());
        assert_eq!(default_list.len(), 0);

        default_list.insert(3i32, "asdf".into());
        default_list.insert(3i32, "fdsa".into());
        default_list.insert(3i32, "hello, world!".into());
        println!("{}", default_list.as_ref().forward.display_forward());
        assert!(!default_list.is_empty());
        assert_eq!(default_list.len(), 1);
        let result = default_list.search(3i32);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, 3i32);
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
                let result = list.search(*key);
                let (found_key, found_value) = result.expect("list should contain this key");
                assert_eq!(*key, found_key);
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
            eprintln!("{list}");

            for deleted in deleted_items.iter() {
                assert!(list.search(deleted.0).is_none(), "item still exists");
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
