// This module exposes an API for managing stream priorities associated with a connection.
use std::collections::{HashMap, VecDeque};

pub struct StreamPriority {
    //weight
    parent: u32,
    children: Vec<u32>
}

pub struct PriorityManager {
    streams: HashMap<u32, StreamPriority>,
    root: VecDeque<u32>
}
