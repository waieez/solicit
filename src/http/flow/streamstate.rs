//! This module implements stream state management to help discover connection errors before an expensive parse is made.
//! It manages the state of the streams associated with the connection
// TODO: Implement Control Flow, Priority
// TODO: Refactor using try!
// TODO: return errors where appropriate
// TODO: import flags/bitmasks for flags from each frame

//use super::super::{HttpError, HttpResult, StreamId};
//use super::super::frame::{RawFrame, FrameHeader, unpack_header};

use std::collections::HashMap;
use super::super::frame::{RawFrame};

// move StreamStatus/States into mod.rs?

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct StreamStatus {
    state: StreamStates,
    priority: Option<u32>,
    dependancy: Option<u32>,
    is_exclusive: bool,
    expects_continuation: bool,
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
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub enum StreamStates {
    Idle,
    Open,
    Closed,
    ReservedLocal,
    ReservedRemote,
    HalfClosedLocal,
    HalfClosedRemote,
}

// enum LocalIdentity { // needs a better name
//     Client,
//     Server
// }

pub struct StreamManager {
    // Stream Concurrency
    // Limit the max streams a peer can open using Settings Frame
    // Open/Half Closed count towards max. reserved don't
    // if endpoint recieves HEADERS frame that goes over max, PROTOCOL_ERROR or REFUSED_STREAM.
    // Can reduce max streams. but must either close streams or wait for them to close.
    last_server_id: u32,
    last_client_id: u32,
    pub max_concurrent_streams: u32, //config?
    pub streams: HashMap<u32, StreamStatus>, //streams should perhaps default to idle
    is_server: bool, // use localidentity instead of boolean?
}

// Perhaps could be used as an abstraction on top of the connection
// For now is used to quickly check/manage the state of the stream
impl StreamManager {

    // Configures the connections' settings for all streams
    pub fn new (max_streams: u32, is_server: bool) -> StreamManager {
        StreamManager {
            last_client_id: 0, //TODO: Figure out proper defaults
            last_server_id: 0,
            max_concurrent_streams: max_streams,
            streams: HashMap::new(),
            is_server: is_server
        }
        //should perhaps initialize the first stream to be open as well.
    }

    // helper method to set the state of a stream.
    fn set_state (&mut self, stream_id: &u32, state: StreamStates) {
        debug!("new state {:?}", &stream_id);
        match self.get_stream_status(&stream_id) {
            Some(stream) => {
                stream.set_state(state);
            },
            None => {
                () // should error
            },
        };
    }

    // Convenience wrapper for getting a stream's status using an id.
    fn get_stream_status (&mut self, stream_id: &u32) -> Option<&mut StreamStatus> {
        self.streams.get_mut(&stream_id)
    }

    fn open_idle (&mut self, stream_id: u32, by_peer: bool) {// maybe by_peer defaults to false?
        debug!("opening stream {:?}", &stream_id);
        //clients can only open odd streams, servers even.
        //potentially create streams using this api.
        if self.check_valid_open_request(stream_id, by_peer) { // checked before already in check_valid_frame
            self.streams.insert(stream_id, StreamStatus::new());
        }
    }

    // API to manually set the state of a stream to be open (if stream id supplied is valid)
    // Streams opened by peers use transition_state
    fn open (&mut self, stream_id: u32, by_peer: bool) {// maybe defaults to false?
        self.open_idle(stream_id, by_peer);
        self.set_state(&stream_id, StreamStates::Open);
    }

    // fn open_half_closed?

    // peer argument should be more descriptive
    fn check_valid_open_request (&mut self, stream_id: u32, by_peer: bool) -> bool {
        match by_peer {
            //RECV
            true => {
                // server is receiving a client request to open a stream
                if self.is_server && stream_id > self.last_client_id && stream_id & 2 == 1 {
                    true
                // client is receiving a server request to open a stream
                } else if !self.is_server && stream_id > self.last_server_id && stream_id % 2 == 0 {
                    true
                } else {
                    false // should err ?
                }
            }

            //SEND
            false => {
                // server attempting to open a new stream
                if self.is_server && stream_id > self.last_server_id && stream_id % 2 == 0 {
                    true
                // client attempting to open a new stream
                } else if !self.is_server && stream_id > self.last_client_id && stream_id % 2 == 1 {
                    true
                } else {
                    false // should err ?
                }
            },
        }
    }

    // API to manually set the state of a stream to be closed
    pub fn close (&mut self, stream_id: u32) {
        //potentially close streams using this api.
        debug!("closing stream {:?}", &stream_id);
        self.set_state(&stream_id, StreamStates::Closed);
    }
    // alternative implementation,
    // each frame type has its own function
    // pass configuration as param

    //current state
    //flag type if any
    //continuation?
    //send or recv

    // Assumes frame is valid for stream and updates stream's status
    // Accepts a frame checks the implied state transition and applies it
    // fn transition_state (&mut self, frame: &RawFrame, by_peer: bool) { //nix this
    //     // let length = frame.header.0;
    //     let frame_type = frame.header.1;
    //     let flag = frame.header.2;
    //     let stream_id = frame.header.3;

    //     match frame_type {
    //         //header or data frame send/recv end stream flag
    //         //TODO: differentiate between sending and recv ES for open?
    //         //the diagram looks like more frames can be sent/recieved
    //         0x1 | 0x0 if flag == 0x1 => self.set_state(&stream_id, StreamStates::Closed),

    //         // push promise or continuation frame w/ end headers flag
    //         0x5 | 0x9 if flag == 0x4 => {
    //             if by_peer {
    //             // recieves end headers flag
    //                 self.set_state(&stream_id, )
    //             } else {
    //             // send end headers flag
    //                 self.set_state(&stream_id, )
    //             }
    //         },

    //         // push promise or continuation w/out end headers flag, expect continuation frame
    //         0x5 | 0x9 if flag == 0x0 => { // do I need to check for valid flags?
    //             match self.get_stream_status(&stream_id) {
    //                 None => (),
    //                 Some(status) => {
    //                     // send or recv?
    //                     status.set_continue();
    //                 }
    //             }
    //             self.set_state(&stream_id, StreamStates::Idle); // could a stream transition from here to open?
    //             //state should remain idle until end headers flag sent
    //         },

    //         _ => ()
    //     }

    //     //self.set_state(stream_id, state)
    // }

    // Assumes valid header frame for state
    // fn handle_header (&mut self, frame: &RawFrame, by_peer: bool) { // parse frame here?
    //     let flag = frame.header.2;
    //     let stream_id = frame.header.3;

    //     // first check if state is idle (rather, if its in the hashmap)
    //     // if state is idle, force open 
    //     match self.get_stream_status(&stream_id) {
    //         None => self.open(),
    //         Some(_status) => _status
    //     };

    //     //todo: refactor to use headersframe::headersflag
    //     match by_peer {
    //         // recv, perhaps less ergonomic but easier to distinguish send/recv for consistency
    //         true => { //bitmask? are end stream and end headers mututally exclusive?
    //             match flag {
    //                 // end stream if set, transition to half closed remote
    //                 0x1 => self.set_state(&stream_id, StreamStates::HalfClosedRemote), // transition to half_closed remote
    //                 // end headers, if not set expect continuation
    //                 0x4 => {
    //                     // if the state is not either of 
    //                     self.set_state(&stream_id, StreamStates)
    //                 },
    //                 // padded
    //                 // 0x8
    //                 //priority
    //                 // 0x20
    //                 _ => (),
    //             };
    //         },

    //         //send 
    //         false =>  {

    //         },
    //     };
    // }

    // fn handle_continuation () {
    //     let flag = frame.header.2;
    //     let stream_id = frame.header.3;

    //     match by_peer {
    //         // recv
    //         true => {
                
    //         },

    //         //send 
    //         false =>  {

    //         },
    //     };
    // }

    // fn handle_rst_stream () {
    //     let flag = frame.header.2;
    //     let stream_id = frame.header.3;

    //     match by_peer {
    //         // recv
    //         true => {
                
    //         },

    //         //send 
    //         false =>  {

    //         },
    //     };
    // }


    // fn send_frame?

    // Recieves a RawFrame and does validation checks between the frame and the state of the associated stream.
    // If validated, returns true. Else, returns the error thrown during validation.
    // Also updates the state of the stream implied by the recieved frames.
    pub fn recv_frame (&mut self, frame: &RawFrame) -> bool {
        // What happens if the raw frame to be processed errors? Connection Error and Close Stream?

        // let length = frame.header.0;
        let frame_type = frame.header.1;
        // let flag = frame.header.2;
        let stream_id = frame.header.3;

        // If the frame is valid, (new stream?, continuation?, otherwise still valid?)
        let is_valid = self.check_valid_frame(&frame); //by_peer?


        if !is_valid {
            //return an error
            //close stream due to connection error?
            false
        } else {
            // state transition, by peer
            // self.transition_state(&frame, false);

            // process frame?
            // return a processed frame?
            // match frame_type {
            //     //Data
            //     0x0 => {
            //     },
            //     //Header
            //     0x1 => {
            //         //Idle -> Open
            //         //if doesn't contain end_headers flag,
            //         //expect continuation
            //     },
            //     //Priority
            //     0x2 => {
            //     },
            //     //RST
            //     0x3 => {
            //     },
            //     //Setting
            //     0x4 => {
            //     },
            //     //PushPromise
            //     0x5 => {
            //         // only in idle
            //     },
            //     //Ping
            //     0x6 => {
            //     },
            //     //Goaway
            //     0x7 => {
            //     },
            //     //WindowsUpdate
            //     0x8 => {
            //     },
            //     //Continuation
            //     0x9 => {
            //         //Sent after Header or PP,
            //         //If doesn't contain end
            //     },
            //     _ => {
            //         // should not enter here
            //     }
            // };
            true
        }
    }



    // Checks if the incoming frame is valid for the particular stream.
    // First identifies if incoming frame is associated with a stream
    // If not and is a valid header, opens the stream
    // Else, checks to see if it is a continuation frame
    // If not, does a final check for validity
    pub fn check_valid_frame(&mut self, frame: &RawFrame) -> bool {

        let frame_type = frame.header.1;
        let stream_id = frame.header.3;

        // Check to see if this id is valid and it is the beginning of a stream.
        let valid_opener = match frame_type {
            // If Header or Push Promise, peer attempting to open/reserve a new stream
            // Stream id's must be increasing, respond to unexpected id's with PROTOCOL_ERROR
            // currently check_valid_frame is only used on recieve
            0x1 | 0x5 => {
                self.check_valid_open_request(stream_id, false)
            },
            _ => false //conn err?
        };

        //TODO: Refactor Match of Doom
        match self.get_stream_status(&stream_id) {
            // If id not in current list of streams, perhaps it's a new one.
            // should not reach this level if check_valid_open_request is false
            None => {
                //cant borrow self again
                valid_opener
            },
            Some(status) => {
                // If stream is expecting continuation frame. Check if this is a continuation frame,
                println!("status: {:?} {:?}", status.expects_continuation, frame_type);
                match status.expects_continuation  {
                    true if frame_type != 0x9 => false, //should err
                    false if frame_type == 0x9 => false, //connection err?
                    _ => {
                        check_state(&status, &frame)
                    }
                }
            }
        }

        
        // perhaps return an option with error instead of bool
    }
}

fn check_state (status: &StreamStatus, frame: &RawFrame) -> bool {
    match status.state { //todo: match states based on by_peer
        StreamStates::Idle => check_idle(&frame),
        StreamStates::Open => check_open(&frame),
        StreamStates::Closed => check_closed(&frame),
        StreamStates::ReservedLocal => check_reserved_local(&frame),
        StreamStates::ReservedRemote => check_reserved_remote(&frame),
        StreamStates::HalfClosedLocal => check_half_closed_local(&frame),
        StreamStates::HalfClosedRemote => check_half_closed_remote(&frame),
    }
}

// helpers for each state, each state only allows a certain type of frames
// TODO: implement acutal filters
fn check_idle (frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Idle
    // SEND/REC: HEADERS --> Open
    // SEND: PP (on another stream), reserves stream (send:local, recv:remote)
    // for now just validate
    match frame_type {
        // header, pp
        // if not end_header, must be followed by continuation frame
        // if header contains ES flag, should transition to close immediately
        0x1 | 0x5 => true,
        _ => false
    }
    // must Protocol Err if stream id is 0x0
}

fn check_open (frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // A stream in the "open" state may be used by both peers to send frames of any type. 
    // In this state, sending peers observe advertised stream level flow control limits (Section 5.2).
    // From this state either endpoint can send a frame with an END_STREAM flag set, 
    // which causes the stream to transition into one of the "half closed" states: 
    // an endpoint sending an END_STREAM flag causes the stream state to become "half closed (local)";
    // an endpoint receiving an END_STREAM flag causes the stream state to become "half closed (remote)".
    // Either endpoint can send a RST_STREAM frame from this state, causing it to transition immediately to "closed".

    //priority
    //stream dependency
    //weight
    //end_headers?
        //if not next frame must be continuation
    match frame_type {
        0x3 => true, // continuation
        0x9 => true, // rst
        _ => false
    }
}

fn check_closed (frame: &RawFrame) -> bool {
    // let frame_type = frame.header.1;
    // Closed
    // SEND: PRIORITY, else ERR
    // RECV: After RECV RST_STREAM or End_STREAM flag, if RECV anything ERR STREAM_CLOSED

    // for now to simplify, just return false
    false

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
}

fn check_reserved_local (frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Reserved Local
    // Associated with open stream initiated by remote peer
    // Send: HEADERS --> half closed (remote)
    // Send: Either endpoint RST_STREAM --> Closed
    match frame_type {
        0x2 => true, // May RECV: PRIORITY/WINDOW_UPDATE
        0x8 => true, // window update
        _ => false
    }
}

fn check_reserved_remote (frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Reserved Remote
    // Reserved by remote peer
    match frame_type {
        0x1 => true, // Recv: HEADERS --> Half Closed
        0x3 => true, // Either endpoint Send: RST_STREAM --> Closed
        0x2 => true, // May RECV: PRIORITY/WINDOW_UPDATE
        0x8 => true, // window update
        _ => false
    }
}

fn check_half_closed_local (frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Half Closed Local (READING)
    // SEND: WINDOW_UPDATE, PRIORITY, RST Stream
    match frame_type {
        0x3 => true, // End_STREAM flag or RST_STREAM --> Close
        _ => true, // Can Recv any frame
    }
}

fn check_half_closed_remote (frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Half Closed Remote (Writing)
    // no longer used by peer to send frames, no longer obligated to maintain reciever flow control window
    // Error w/ STREAM_CLOSED when when recv frames not WINDOW_UPDATE, PRIORITY, RST_STREAM
    match frame_type {
        0x2 => true,
        0x8 => true, //window update
        _ => false
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StreamManager,
        StreamStates
    };

    use super::super::super::frame::{RawFrame, pack_header};

    //TODO: implement flag checks for stream states using bitmasking
    /// Sets the given flag for the frame.
    // fn set_flag(&mut self, flag: HeadersFlag) {
    //     self.flags |= flag.bitmask();
    // }

    /// Builds a test frame of the given type with the given header and
    /// payload, by using the `Frame::from_raw` method.
    pub fn build_test_rawframe (stream_id: u32, frame_type: &str, flags: &str) -> RawFrame {
        let data = b"123";
        // (length, frame_type, flags, stream_id)

        let _flag = {
            match flags {
                "endstream" => 0x1,
                "endheaders" => 0x4,
                //priority
                _ => 0x0
            }
        };

        let _type = {
            match frame_type {
                "data" => 0x0,
                "headers" => 0x1,
                "priority" => 0x2,
                "rststream" => 0x3,
                "settings" => 0x4,
                "pushpromise" => 0x5,
                "ping" => 0x6,
                "goaway" => 0x7,
                "windowsupdate" => 0x8,
                "continuation" => 0x9,
                _ => 0x7 // should probably go away if not these types
            }
        };

        let header = (data.len() as u32, _type, 0x0, stream_id);
        let buf = {
            let mut buf = Vec::new();
            buf.extend(pack_header(&header).to_vec().into_iter());
            buf.extend(data.to_vec().into_iter());
            buf
        };
        RawFrame::from_buf(&buf).unwrap()
    }


    // #[test]
    fn test_check_valid_frame () {

    }

    // Tests for Opening a stream
    #[test]
    fn test_open_stream () {
        let mut stream_manager = StreamManager::new(4, false);
        stream_manager.open(1, false);
        assert_eq!(stream_manager.streams[&1].state, StreamStates::Open);
    }

    // A new open stream should not immediately accept continuation frames unless the expect_continuation is set.
    #[test]
    fn test_open_with_continuation () {
        let mut stream_manager = StreamManager::new(4, false);
        stream_manager.open(1, false);

        let raw_continue = build_test_rawframe(1, "continuation", "endstream");

        let check_fail = stream_manager.check_valid_frame(&raw_continue);
        assert_eq!(check_fail, false);

        stream_manager.get_stream_status(&1).unwrap().set_continue(true);

        let check_again = stream_manager.check_valid_frame(&raw_continue);
        assert_eq!(check_again, true);
    }

    //Connections open through a series of exchanges.
    //After the connection is established, Peer sends HeadersFrame that transitions the a stream from Idle to Open
    // #[test]
    fn test_implicit_open () {
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(1, "headers", "endstream");
        stream_manager.recv_frame(&raw_header);
        // this should validate the frame
        // transition state of stream to Open if the id is valid
        // error if id is invalid

        //let check_pass = stream_manager.check_valid_frame(&raw_header);

        //assert_eq!(check_pass, true);
    }


    //Tests for Closing a stream

    // Manual Close

    // Connection Errors should close a stream

    // End Flags on different frames
    //Header, Data, Continuation

    // RST_Frame

    // Test for Data Frames

}
