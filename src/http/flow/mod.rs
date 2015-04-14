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

// States:
// Idle
// SEND/REC: HEADERS --> Open
// SEND: PP (on another stream), reserves stream (send:local, recv:remote)

// Reserved Local
// Associated with open stream initiated by remote peer
// Send: HEADERS --> half closed (remote)
// Send: Either endpoint RST_STREAM --> Closed
// May RECV: PRIORITY/WINDOW_UPDATE

// Reserved Remote
// Reserved by remote peer
// Recv: HEADERS --> Half Closed
// Either endpoint Send: RST_STREAM --> Closed
// May Recv: PRIORITY/WINDOW_UPDATE

// Half Closed Local (READING)
// SEND: WINDOW_UPDATE, PRIORITY, RST Stream
// End_STREAM flag or RST_STREAM --> Close
// Can Recv any frame

// Half Closed Remote (Writing)
// no longer used by peer to send frames, no longer obligated to maintain reciever flow control window
// Error w/ STREAM_CLOSED when when recv frames not WINDOW_UPDATE, PRIORITY, RST_STREAM

// Closed
// SEND: PRIORITY, else ERR
// RECV: After RECV RST_STREAM or End_STREAM flag, if RECV anything ERR STREAM_CLOSED

//   Exceptions: 
//   WINDOW_UPDATE or RST_STREAM
//     can RECV: WINDOW_UPDATE or RST_STREAM for a short period after DATA or HEADERS frame containing an END_STREAM flag is sent. 
//     Until the remote peer receives and processes RST_STREAM or the frame bearing the END_STREAM flag, it might send frames of these types. 
//     Endpoints MUST ignore WINDOW_UPDATE or RST_STREAM frames received in this state, 
//     though endpoints MAY choose to treat frames that arrive a significant time after sending END_STREAM as a connection error (Section 5.4.1) of type PROTOCOL_ERROR.
//   PRIORITY
//     can SEND: PRIORITY to prioritize streams dependant on closed stream.
//     should process PRIORITY frame, though can be ignored if stream removed from dep tree.
//   If Stream becomes closed after sending an RST_STREAM frame, can have a window for ignoring additional recived frames. after which should ERR.
//   Flow controlled frames (i.e., DATA) received after sending RST_STREAM are counted toward the connection flow control window. 
//     Even though these frames might be ignored, because they are sent before the sender receives the RST_STREAM, the sender will consider the frames to count against the flow control window.
//   PUSH_PROMISE
//     An endpoint might receive a PUSH_PROMISE frame after it sends RST_STREAM. 
//     PUSH_PROMISE causes a stream to become "reserved" even if the associated stream has been reset. 
//     Therefore, a RST_STREAM is needed to close an unwanted promised stream.

// STREAM_ID: u31
// init by Client: Odd
// init by Server: Even

// Stream id must be increasing, respond to unexpected id's with PROTOCOL_ERROR

// Stream Concurrency
// Limit the max streams it peer can open using Settings Frame
// Open/Half Closed count towards max. reserved don't
// if endpoint recieves HEADERS frame that goes over max, PROTOCOL_ERROR or REFUSED_STREAM.
// Can reduce max streams. but must either close streams or wait for them to close.




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

pub mod streamstate;