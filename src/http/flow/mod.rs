// A "stream" is an independent, bi-directional sequence of frames exchanged between the client and server within an HTTP/2 connection. 
// Streams have several important characteristics:
// A single HTTP/2 connection can contain multiple concurrently open streams, with either endpoint interleaving frames from multiple streams.
// Streams can be established and used unilaterally or shared by either the client or server.
// Streams can be closed by either endpoint.
// The order in which frames are sent on a stream is significant. 
//     Recipients process frames in the order they are received. In particular, the order of HEADERS, and DATA frames is semantically significant.
// Streams are identified by an integer. Stream identifiers are assigned to streams by the endpoint initiating the stream.


// STREAM STATES

//                          +--------+
//                  send PP |        | recv PP
//                 ,--------|  idle  |--------.
//                /         |        |         \
//               v          +--------+          v
//        +----------+          |           +----------+
//        |          |          | send H /  |          |
// ,------| reserved |          | recv H    | reserved |------.
// |      | (local)  |          |           | (remote) |      |
// |      +----------+          v           +----------+      |
// |          |             +--------+             |          |
// |          |     recv ES |        | send ES     |          |
// |   send H |     ,-------|  open  |-------.     | recv H   |
// |          |    /        |        |        \    |          |
// |          v   v         +--------+         v   v          |
// |      +----------+          |           +----------+      |
// |      |   half   |          |           |   half   |      |
// |      |  closed  |          | send R /  |  closed  |      |
// |      | (remote) |          | recv R    | (local)  |      |
// |      +----------+          |           +----------+      |
// |           |                |                 |           |
// |           | send ES /      |       recv ES / |           |
// |           | send R /       v        send R / |           |
// |           | recv R     +--------+   recv R   |           |
// | send R /  `----------->|        |<-----------'  send R / |
// | recv R                 | closed |               recv R   |
// `----------------------->|        |<----------------------'
//                          +--------+

//    send:   endpoint sends this frame
//    recv:   endpoint receives this frame

//    H:  HEADERS frame (with implied CONTINUATIONs)
//    PP: PUSH_PROMISE frame (with implied CONTINUATIONs)
//    ES: END_STREAM flag
//    R:  RST_STREAM frame

// Flow Control Scheme
// Scheme Ensures stream on same conn do not destrctively interfere.
// Used for individual streams and for connection as a whole.
// Req: Window Update

// Flow Control Principles

// 1. Flow control is specific to a connection. Both types of flow control are between the endpoints of a single hop, and not over the entire end-to-end path.
// 2. Flow control is based on window update frames. Receivers advertise how many octets they are prepared to receive on a stream and for the entire connection. 
//     This is a credit-based scheme.
// 3. Flow control is directional with overall control provided by the receiver. 
//     A receiver MAY choose to set any window size that it desires for each stream and for the entire connection.
//     A sender MUST respect flow control limits imposed by a receiver.
//     Clients, servers and intermediaries all independently advertise their flow control window as a receiver
//       and abide by the flow control limits set by their peer when sending.
// 4. The initial value for the flow control window is 65,535 octets for both new streams and the overall connection.
// 5. Only DATA Frames are subject to flow control. Ensures important control frames aren't blocked by flow control
// 6. Flow control cannot be disabled.
// 7. The HTTP/2 Spec defines the format/semantics of WINDOW_UPDATE. When it is sent, what values, etc up to implementation.

// Implementations are also responsible for managing how requests and responses are sent based on priority;
// choosing how to avoid head of line blocking for requests;
// and managing the creation of new streams.
// Algorithm choices for these could interact with any flow control algorithm.


// Appropriate Use of Flow Control
//  If not required, advertise a flow control window of the maximum size (2^31-1)
//  Flow control to limit memory use. However lead to suboptimal use of network resources.

// Stream Priority

// Initialized with Headers Frame
// Can be modified by sending priority frames

// prioritized by marking stream as dependant on another.

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

// Prioritization State Management
// ???

#[allow(unused_variables)]
#[allow(dead_code)] // enabled for terminal legibility
pub mod streamstate;
mod utils;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum StreamStates {
    Idle,
    Open,
    Closed,
    ReservedLocal,
    ReservedRemote,
    HalfClosedLocal,
    HalfClosedRemote,
}

pub enum Flags {
    EndStream = 0x1,
    EndHeaders = 0x4,
    Padded = 0x8,
    Priority = 0x20,
}

impl Flags {
    fn bitmask(self) -> u8 {
        self as u8
    }
    // Tests if the given flag is set for the frame.
    fn is_set(self, flags: u8) -> bool {
        (flags & self.bitmask()) != 0
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct StreamStatus {
    pub state: StreamStates,
    priority: Option<u32>,
    dependancy: Option<u32>,
    is_exclusive: bool,
    pub expects_continuation: bool,
    should_end: bool,
    is_reserved: bool,
    //children: Vec<u32>?
    //window_size:?
}

impl StreamStatus {
    fn new () -> StreamStatus {
        StreamStatus {
            state: StreamStates::Idle,
            priority: None,
            dependancy: None,
            is_exclusive: false,
            expects_continuation: false,
            should_end: false,
            is_reserved: false,
        }
    }

    fn set_state (&mut self, state: StreamStates) -> &mut StreamStatus {
        self.state = state;
        self
    }

    fn set_priority (&mut self, priority: Option<u32>) -> &mut StreamStatus {
        self.priority = priority;
        self
    }

    fn set_dependancy (&mut self, dependancy: Option<u32>, is_exclusive: bool) -> &mut StreamStatus {
        self.dependancy = dependancy;
        // setting a stream as dependant and exclusive will affect all children of dependancy
        self.is_exclusive = is_exclusive;
        self
    }

    fn remove_dependancies (&mut self) -> &mut StreamStatus {
        self.dependancy = None;
        self.is_exclusive = false;
        self
    }

    fn set_continue(&mut self, expects_continuation: bool) -> &mut StreamStatus {
        self.expects_continuation = expects_continuation;
        self
    }

    fn set_end(&mut self, should_end: bool) -> &mut StreamStatus {
        self.should_end = should_end;
        self
    }

    fn set_reserved(&mut self, reserved: bool) -> &mut StreamStatus {
        self.is_reserved = reserved;
        self
    }
}
