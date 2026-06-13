# skiplist-rs

`skiplist-rs` is an implementation of the skip-list data structure from [Pugh, William. (1990) _Skip lists: a probabilistic alternative to balanced trees_](https://doi.org/10.1145/78973.78977) in Rust.

## Design

### Sentinel node

The root node is a `NodeHead` allocated with `MAX_LEVEL` levels. All `NodeHead`s that follow the sentinel are created as part of a `Node` allocation. For those, the `Node::from_head` method is used to convert a `NodeHead` pointer to a `Node` pointer.

### Node layout

Each `Node` is a `repr(C)` struct holding a key, value, and the node's level through a `NodeHead`. Using `std::alloc`, enough memory is allocated for the Node's defined members and additional memory for an array of `Option<NonNull<NodeHead<_,_>>>` pointers. Because the forward list is stored after the other members in `Node`, cache locality is improved in comparison to a heap-allocated container such as `Vec`, which would require jumping to another area of memory to traverse the forward list.
