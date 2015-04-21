// This module exposes an API for managing stream priorities and dependancies associated with a connection.
// The current implementation effectively round robins all streams without dependancies. Ignores weights.
// The user of the API must explicity requeue unfinished streams
//TODO: Implement weights
use std::collections::{HashMap, HashSet, VecDeque};

// Notes: Stream Priority

// Initialized with Headers Frame
// Can be modified by sending priority frames

// Prioritized by marking stream as dependant on another.

// Stream w/ 0 deps stream dep of 0x0

// Default: dependant streams are unordered.
// Exclusive: becomes sole dependancy of parent, adopt sibling streams as children.

// Child streams are only allocated resources when parent chain is closed.

// Dep Weighting weight between 1 and 256
// siblings share proportional resources if progress on parent not able to be made.

// Reprioritization.
// Dep streams move with parent if parent is reprioritized.

// if moved with exclusive flag. new parent's children are adopted by moved dependency stream.

// if moved to be dependant on child, child and parent switch roles. retains weight. (watchout for exclusive flag)


#[derive(Eq, PartialEq, Debug, Clone)]
pub struct StreamPriority {
    //weight
    parent: u32, // id: 0 == no parent
    depth: u32,
    is_exclusive: bool, // set on the parent after exclusive child is set
    exclusive_child: u32, //defaults to 0, set only when set exclusive is called
    children: HashSet<u32>,
}

impl StreamPriority {

    // creates an node
    fn new () -> StreamPriority {
        StreamPriority {
            parent: 0,
            depth: 1,
            is_exclusive: false,
            exclusive_child: 0,
            children: HashSet::new()
        }
    }

    // sets the parent of a given stream
    fn set_parent (&mut self, parent_id: u32) -> &mut StreamPriority {
        self.parent = parent_id;
        self
    }

    // sets the depth of a give stream
    fn set_depth (&mut self, depth: u32) -> &mut StreamPriority {
        self.depth = depth;
        self
    }

    // adds a child to the given stream
    fn add_child (&mut self, child_id: u32) -> &mut StreamPriority {
        self.children.insert(child_id);
        self
    }

    // removes a child from the list
    fn remove_child (&mut self, child_id: &u32) -> &mut StreamPriority {
        self.children.remove(child_id);
        self
    }

    // sets exclusive flag on stream
    fn set_exclusive (&mut self, is_exclusive: bool) -> &mut StreamPriority {
        self.is_exclusive = is_exclusive;
        self
    }

}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct PriorityManager {
    streams: HashMap<u32, StreamPriority>,
    queue: VecDeque<u32> // note: could also consider a heap
}


impl PriorityManager {
    // creates an instance of a priority manager for the connection
    pub fn new () -> PriorityManager {
        PriorityManager {
            streams: HashMap::new(),
            queue: VecDeque::new()
        }
    }

    // gets the first independant stream id from the queue
    pub fn get (&mut self) -> Option<u32> {
        let mut stream_id = self.queue.pop_front();
        loop {
            match stream_id {
                Some(x) => {
                    if self.streams[&x].parent == 0 {
                        break // independant stream found
                    } else {
                        stream_id = self.queue.pop_front();
                    }
                },
                None => {
                    break
                },
            }
        }
        stream_id
    }

    // pushes stream id onto queue, also creates a new streampriority if not present
    pub fn add (&mut self, stream_id: u32) {
        // Tries to add a new stream to streams. Insert fails quietly if value is present.
        if !self.contains(&stream_id) {
            self.streams.insert(stream_id, StreamPriority::new());
        }
        // adds to queue only if not dependant
        if self.streams[&stream_id].parent == 0 {
            self.queue.push_back(stream_id);
        }
    }

    // adds new stream id to hashmap with parent pointer set if parent key exists
    pub fn add_with_dependancy (&mut self, stream_id: u32, parent_id: u32) {
        if !self.contains(&stream_id) && self.contains(&parent_id) {
            self.streams.insert(stream_id, StreamPriority::new());
            self.set_dependant(stream_id, parent_id);
        } // note: else should err
    }

    // modifies a given stream's dependancy, (child depends on parent)
    pub fn set_dependant (&mut self, child_id: u32, parent_id: u32) {
        if self.contains(&child_id) && self.contains(&parent_id) {
            if self.streams[&parent_id].is_exclusive &&
                self.streams[&parent_id].children.len() == 1 {
                // change parent id to be the child of the exclusive parent
                let redirect = self.streams[&parent_id].exclusive_child;
                self.set_dependant_stream(child_id, redirect);
            } else {
                self.set_dependant_stream(child_id, parent_id);
            }
        }
    }

    // sets a given stream as an exclusive dependancy
    // sets a stream's siblings as children of itself
    // exclusive child also intercepts and adopts incoming children (handled by set_dependant)
    pub fn set_as_exclusive (&mut self, child_id: u32, parent_id: u32) {
        // re-maps all children and sets flag on exclusive parent
        let valid = self.contains(&child_id);
        let mut orphans = None;

        if valid {
            match self.streams.get_mut(&parent_id) {
                Some(parent) => {
                    // sets flag and gets all siblings
                    parent.is_exclusive = true;
                    parent.exclusive_child = child_id;
                    parent.remove_child(&child_id);
                    let mut siblings = parent.children.clone();
                    // clears siblings and reinserts own id
                    parent.children.clear();
                    parent.add_child(child_id);

                    orphans = Some(siblings);
                }
                _ => (),
            };
        }

        // adopt previously orphaned siblings
        if let Some(children) = orphans {
            for child in children {
                self.set_dependant(child, child_id);
            }
        }; // else if invalid parent/child id do nothing
    }

    // removes the stream id and all parent/child associations from priority manager
    // forces stream's parent (if any) to adopt its children (if any)
    pub fn retire (&mut self, stream_id: u32) {
        // note: should retired streams be assigned a dependancy?

        if self.contains(&stream_id) {
            // remove item from hashmap and update depth for all descendants
            let depth = self.streams[&stream_id].depth;
            self.update_tree_depths(&stream_id, depth-1);
            let stream = self.streams.remove(&stream_id).unwrap();

            //update children's parent
            for child_id in stream.children.iter().cloned() {

                self.streams.get_mut(&child_id).unwrap().set_parent(stream.parent);
                //if parent is not a stream, add to queue
                if stream.parent == 0 {
                    self.add(child_id);
                }
                //if parent is an actual stream, remove self from child list
                //and add all children(if any) to parent
                else if stream.parent != 0 && self.contains(&stream.parent) {
                    let grand_parent = self.streams.get_mut(&stream.parent).unwrap();

                    grand_parent.children.remove(&stream_id);
                    for child_id in stream.children.iter().cloned() {
                        grand_parent.add_child(child_id);
                    }
                }
            }
        }
    }
    //perhaps a method to destroy an entire dependancy tree

    // helpers for connecting nodes
    //TODO: refactor helpers into utils

    //helper for checking if id contained in stream
    fn contains (&mut self, stream_id: &u32) -> bool {
        self.streams.contains_key(&stream_id)
    }

    // detects cycles by checking the depth of the two nodes,
    // reoranizes/inserts (sub)trees and updates depths
    fn set_dependant_stream (&mut self, node_a: u32, node_b: u32) {

        let child_depth = self.streams[&node_a].depth;
        let parent_depth = self.streams[&node_b].depth;

        // cycles can be created by making an ancestor to be the child of a descendant
        if child_depth < parent_depth {
            let mut ancestor = node_b;
            let difference = parent_depth - child_depth;

            // get the ancestor at node_a's depth
            for n in (0..difference) {
                ancestor = self.streams[&ancestor].parent;
            }

            // swap the positions if they are equal
            // note: stream ids are unique and therefore should not collide
            if ancestor == node_a {
                self.swap(node_a, node_b);
            } else {
                // different ancestry, no possible cycles
                self.connect(node_a, node_b);
                //update all descendants with new depths
                self.update_tree_depths(&node_a, parent_depth+1);
            }

        } else {
            //they are at the same depth, 
            // or it is potentially a descendant deeper in the tree
            // therefore no chance of cycles
            self.connect(node_a, node_b);
            //update all descendants with new depths
            self.update_tree_depths(&node_a, parent_depth+1);
        }
    }

    // Swap Descendant/Ancestor
    fn swap (&mut self, node_a: u32, node_b: u32) {
        // node_b -> node_a -> root:X

        // get ref to X, node_a's parent
        let mut ref_x = self.streams[&node_a].parent;

        // get ref to child_id, node_b's parent
        let ref_a = self.streams[&node_b].parent;

        //get depths of node_a/node_b
        let depth = self.streams[&node_a].depth;

        //disconn node_b - node_a: node_b node_a X
        self.update_tree_depths(&node_b, depth);
        self.disconnect(node_b, ref_a);

        //conn node_b - X: node_a node_b -> X
        self.connect(node_b, ref_x);

        //disconn node_a - X: node_b -> node_a  X
        self.update_tree_depths(&node_a, depth+1);
        self.disconnect(node_a, ref_x);

        //finally, conn node_a - node_b: node_a -> node_b -> X
        self.connect(node_a, node_b);
    }

    // removes child's parent pointer and child from parent's set of children
    fn disconnect (&mut self, child_id: u32, parent_id: u32) {
        if parent_id > 0 { // stream 0 DNE and has no children
            self.streams.get_mut(&parent_id).unwrap().remove_child(&child_id);
        }
        self.streams.get_mut(&child_id).unwrap().parent = 0;
    }

    // sets child's parent pointer and adds child to parent's set of children
    fn connect (&mut self, child_id: u32, parent_id: u32) {
        let mut depth = 0;
        if parent_id > 0 { // stream 0 DNE and has no children
            depth = self.streams.get_mut(&parent_id).unwrap()
                .add_child(child_id)
                .depth;
        }
        let child = self.streams.get_mut(&child_id).unwrap()
            .set_parent(parent_id)
            .set_depth(depth+1);
    }

    // recursively updates the depth of each node in the (sub)tree
    fn update_tree_depths (&mut self, child_id: &u32, depth: u32) {

        let mut clone = None;
        match self.streams.get_mut(child_id) {
            None => (),
            Some(stream) => {
                stream.depth = depth;
                let children = stream.children.clone();
                clone = Some(children);
            },
        }

        if let Some(children) = clone {
            for child in children.iter() {
                self.update_tree_depths(child, depth+1);
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::{PriorityManager, StreamPriority};

    // StreamPriority

    // PriorityManager sanity check for underlying datastructures
    #[test]
    fn test_add () {
        let mut priority_manager = PriorityManager::new();
        let id = 1;
        priority_manager.add(id);
        let exists = priority_manager.streams.contains_key(&id);
        let queued = priority_manager.queue.pop_front();

        assert_eq!(exists, true);
        assert_eq!(queued, Some(id));
    }

    // Correctly maps child to parent and parent to child
    #[test]
    fn test_set_dependant () {
        let mut priority_manager = PriorityManager::new();
        let parent = 1;
        let child = 2;

        priority_manager.add(parent);
        priority_manager.add(child);

        priority_manager.set_dependant(child, parent);

        // child points to parent
        let child_dependancy = priority_manager.streams[&child].parent;
        assert_eq!(child_dependancy, parent);

        // parent contains child
        let child_set = priority_manager.streams[&parent].children.contains(&child);
        assert_eq!(child_set, true);
    }

    // Does not push onto queue
    #[test]
    fn test_add_with_dependancy () {
        let mut priority_manager = PriorityManager::new();
        let parent = 1;
        let child = 2;
        priority_manager.add(parent);
        priority_manager.add_with_dependancy(child, parent);
        priority_manager.add(child); // tests collisions

        //should not be in queue
        priority_manager.queue.pop_front();
        let empty = priority_manager.queue.is_empty();
        assert_eq!(empty, true);

        //should be in streams
        let exists = priority_manager.streams.contains_key(&child);
        assert_eq!(exists, true);
    }

    // Gets the first indpendant stream id
    #[test]
    fn test_get () {
        let mut priority_manager = PriorityManager::new();
        priority_manager.add(1);
        priority_manager.add(2);
        priority_manager.add(3);
        priority_manager.set_dependant(1, 3);
        priority_manager.set_dependant(2, 1);

        let stream_id = priority_manager.get();
        assert_eq!(stream_id, Some(3));

        let empty = priority_manager.queue.is_empty();
        assert_eq!(empty, true);
    }

    // remaps children to parent if exists
    #[test]
    fn test_retire () {
        let mut priority_manager = PriorityManager::new();
        let (root, parent, child1, child2, child3) = (1, 2, 3, 4, 5);

        priority_manager.add(root); //depth 1
        priority_manager.add_with_dependancy(parent, root); //depth 2
        priority_manager.add_with_dependancy(child1, parent); //depth 3
        priority_manager.add_with_dependancy(child2, parent); //depth 3
        priority_manager.add_with_dependancy(child3, child2); //depth 4

        let old_depth = priority_manager.streams[&child3].depth;
        assert_eq!(old_depth, 4);

        priority_manager.retire(parent);

        let children = priority_manager.streams[&root].children.len();
        assert_eq!(children, 2);

        let new_parent = priority_manager.streams[&child1].parent;
        assert_eq!(new_parent, root);

        priority_manager.retire(root);

        //combined queue should have root, child1, child2 [1, 3, 4]
        let length = priority_manager.queue.len();
        assert_eq!(length, 3);

        //depth should update
        let new_depth = priority_manager.streams[&child3].depth;
        assert_eq!(new_depth, 2);
    }

    #[test]
    fn test_set_exclusive () {
        let mut priority_manager = PriorityManager::new();
        let (root, parent, child1, child2, child3, child4) = (1, 2, 3, 4, 5, 6);

        priority_manager.add(root);
        priority_manager.add_with_dependancy(parent, root);
        priority_manager.add_with_dependancy(child1, root);
        priority_manager.add_with_dependancy(child2, root);
        priority_manager.add_with_dependancy(child3, child2);

        // Before Set Exlcusive:
        let root_children_before = priority_manager.streams[&root].children.len();
        let parent_children_before = priority_manager.streams[&parent].children.len();
        let old_depth = priority_manager.streams[&child3].depth;

        assert_eq!(root_children_before, 3);
        assert_eq!(parent_children_before, 0);
        assert_eq!(old_depth, 3);

        priority_manager.set_as_exclusive(parent, root);

        // tests for collision, 
        // adding child to root when exclusiveness set should addto exclusive child
        priority_manager.add_with_dependancy(child4, root);

        // After Set Exclusive:
        let root_children_after = priority_manager.streams[&root].children.len();
        let parent_children_after = priority_manager.streams[&parent].children.len();
        let new_depth = priority_manager.streams[&child3].depth;

        assert_eq!(root_children_after, 1);
        assert_eq!(parent_children_after, 3);
        //should update depth
        assert_eq!(new_depth, 4);
    }

    #[test]
    fn test_deny_cycles () {
        let mut priority_manager = PriorityManager::new();

        priority_manager.add(1);
        priority_manager.add(4);
        priority_manager.add_with_dependancy(2, 1);
        priority_manager.add_with_dependancy(3, 2);

        priority_manager.set_dependant(4, 3);
        priority_manager.set_dependant(2, 4);

        let a = priority_manager.streams[&1].parent;
        let d = priority_manager.streams[&4].parent;
        let b = priority_manager.streams[&2].parent;
        let c = priority_manager.streams[&3].parent;

        let depth_a = priority_manager.streams[&1].depth;
        let depth_d = priority_manager.streams[&4].depth;
        let depth_b = priority_manager.streams[&2].depth;
        let depth_c = priority_manager.streams[&3].depth;

        assert_eq!((a, d, b, c), (0, 1, 4, 2));
        assert_eq!((depth_a, depth_d, depth_b, depth_c), (1, 2, 3, 4));
    }
}
