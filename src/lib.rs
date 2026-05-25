use std::{fmt::Display, mem::MaybeUninit, ptr::NonNull};

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

    fn level(&self) -> usize {
        if self.forward.is_empty() {
            1
        } else {
            self.forward.len()
        }
    }
}

impl<K, V> Drop for Node<K, V> {
    fn drop(&mut self) {
        assert!(self.forward.iter().all(|e| e.is_none()));
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

impl<K, V> SkipList<K, V>
where
    K: Ord + Eq + std::fmt::Debug,
    V: std::fmt::Debug,
{
    pub fn new() -> Self {
        SkipList {
            len: 0,
            forward: vec![None; 1],
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        // TODO make array
        let mut update: Vec<MaybeUninit<NonNull<[NodePointer<K, V>]>>> =
            vec![MaybeUninit::uninit(); Self::MAX_LEVEL];
        let level = self.level();
        let mut x_forward = self.forward.as_mut_slice();

        for i in (0..level).rev().inspect(|l| println!("for level {l}")) {
            // TODO unwrap unchecked
            println!("[forward list]\n{}", x_forward.display_forward());
            loop {
                match x_forward[i] {
                    Some(mut ptr) => {
                        let node = unsafe { ptr.as_mut() };
                        if node.key < key {
                            x_forward = node.forward.as_mut();
                        } else {
                            break;
                        }
                    }
                    None => break,
                }
            }
            update[i].write(x_forward.into());
        }

        // TODO account for root node here
        let mut x = x_forward[0].unwrap();
        if let Some(mut x) = x_forward[0]
            && unsafe { x.as_ref() }.key == key
        {
            unsafe { x.as_mut() }.value = value;
        } else {
            let new_level = Self::random_level();
            if new_level > self.level() {
                for i in self.level()..new_level {
                    update[i].write(self.forward.as_mut_slice().into());
                }
            }

            let new_node = Box::new(Node::new(key, value, &[None]));
            let new_node = NonNull::new(Box::leak(new_node));
            let mut new_forward = Vec::with_capacity(self.level());
            for i in 1..new_level {
                let target_list = unsafe { update[i].assume_init_mut().as_mut() };
                let target_node = target_list[i];
                new_forward.push(target_node);
                target_list[i] = new_node;
            }
            let new_node = unsafe { new_node.unwrap().as_mut() };
            new_node.forward = new_forward.into_boxed_slice();
        }
    }

    pub fn search(&self, key: K) -> Option<(K, &V)> {
        None
    }
}

impl<K, V> Default for SkipList<K, V> {
    fn default() -> Self {
        Self {
            len: 0,
            forward: vec![None; 1],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default() {
        let mut default_list: SkipList<i32, String> = Default::default();
        assert!(default_list.is_empty());
        assert_eq!(default_list.len(), 0);

        default_list.insert(3i32, "asdf".into());
        assert!(!default_list.is_empty());
        assert_eq!(default_list.len(), 1);
        let result = default_list.search(3i32);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, 3i32);
        assert_eq!(value, &"asdf".to_string());
    }

    #[test]
    #[should_panic]
    fn node_drop_panic() {
        let node2: Node<u32, ()> = Node::default();
        let node2 = Box::new(node2);
        let node2 = NonNull::new(Box::leak(node2));
        assert!(node2.is_some());
        let node = Node::new(1u32, (), &[Some(node2.unwrap()), Some(node2.unwrap())]);
        assert_eq!(node.level(), 2);
        drop(node);
    }

    #[test]
    fn random_inserts() {
        let mut list = SkipList::new();

        let items = 100;
        // TODO Fix collisions
        let mut random_items = Vec::<(u64, usize)>::with_capacity(items);
        for i in 0..items {
            let key = rand::random();
            random_items.push((key, i));
            list.insert(key, i);
        }

        // Verify we can find all the items
        for (key, value) in random_items.into_iter() {
            let result = list.search(key);
            let (found_key, found_value) = result.expect("list should contain this key");
            assert_eq!(key, found_key);
            assert_eq!(value, *found_value);
        }
    }

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
