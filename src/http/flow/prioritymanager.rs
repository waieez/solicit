// This module exposes an API for managing stream priorities associated with a connection.
use std::collections::{HashMap, VecDeque};

pub struct StreamPriority {
    //weight
    parent: u32,
    children: Vec<u32>
}

impl StreamPriority {

    // creates an node
    fn new () {

    }

    // sets the parent of a given stream
    fn set_parent () {

    }

    // adds a child to the given stream
    fn add_child () {

    }

    // sets the current stream's siblings as children of itself
    fn set_exclusive () {

    }
}

pub struct PriorityManager {
    streams: HashMap<u32, StreamPriority>,
    queue: VecDeque<u32>
}


impl PriorityManager {
    // creates an instance of a priority manager for the connection
    pub fn new () {

    }

    // gets the first independant stream id from the queue
    pub fn get () {

    }

    // pushes stream id onto queue
    pub fn add () {

    }

    // adds stream id to hashmap with parent pointer set
    pub fn add_with_dependancy () {

    }

    // modifies a given's stream's dependancy
    pub fn set_as_dependancy () {

    }

    // set's a given stream as an exclusive dependancy
    pub fn set_as_exclusive () {

    }

    // removes the stream id and all parent/child associations from priority manager
    // forces stream's parent (if any) to adopt its children (if any)
    pub fn retire () {

    }
}


#[cfg(test)]
mod tests {
    use super::{PriorityManager, StreamPriority};

    
}
