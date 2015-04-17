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
    should_end: bool
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
}

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
        println!("fn open_idle: inside open idle..., ln134");
        if self.check_valid_open_request(stream_id, by_peer) { // checked before already in check_valid_frame
            self.streams.insert(stream_id, StreamStatus::new());

            //if it passes check_valid, update the appropriate id to the newly opened one.
            match stream_id % 2 == 0 {
                true => self.last_server_id = stream_id,
                false => self.last_client_id = stream_id,
            }
        }
    }

    // API to manually set the state of a stream to be open (if stream id supplied is valid)
    fn open (&mut self, stream_id: u32, by_peer: bool) {// maybe defaults to false?

        //TODO: Check for Max Concurrency

        let not_set = match self.get_stream_status(&stream_id) {
            None => true,
            Some(status) => false,
        };

        // Exhaustively ensures the stream has not already been set
        // Checks if stream_id provided is valid
        if not_set {
            println!("fn open: pass control to open_idle");
            self.open_idle(stream_id, by_peer);
        };

        println!("fn open: called open on stream: {:?} {:?}, ln 163", &stream_id, not_set);
        // And only opens the stream if it was originally set to idle
        if self.streams[&stream_id].state == StreamStates::Idle {
            println!("fn open: stream state is idle.., forcing open..", );
            self.set_state(&stream_id, StreamStates::Open);
        };
        // else err, tried to manually open a stream that has transitioned
    }

    // fn open_half_closed?

    // peer argument should be more descriptive
    fn check_valid_open_request (&mut self, stream_id: u32, by_peer: bool) -> bool {
        match by_peer {
            //RECV
            true => {
                println!("check valid for recv frame");
                // server is receiving a client request to open a stream
                if self.is_server && stream_id > self.last_client_id && stream_id & 2 == 1 {
                    true
                // client is receiving a server request to open a stream
                } else if !self.is_server && stream_id > self.last_server_id && stream_id % 2 == 0 {
                    println!("check for client, {:?} {:?}", stream_id, self.last_server_id);
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
        debug!("fn close: closing stream {:?}, ln210", &stream_id);
        self.set_state(&stream_id, StreamStates::Closed);
    }

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
        let is_valid = self.check_valid_frame(&frame, true); //by_peer?

        println!("fn recv_frame: is the frame valid? {:?}, ln230", is_valid);

        if !is_valid {
            //return an error
            //close stream due to connection error?
            false
        } else {
            // state transition, by peer
            // self.transition_state(&frame, false);

            // process frame?
            // return a processed frame?
            match frame_type {
                //Data
                0x0 => {
                },
                //Header
                0x1 => {
                    self.handle_header(true, &frame);
                },
                //Priority
                0x20 => {
                },
                //RST
                0x3 => {
                    self.handle_rst_stream(true, &frame);
                },
                //Setting
                0x4 => {
                },
                //PushPromise
                0x5 => {
                    self.handle_push_promise(true, &frame);
                },
                //Ping
                0x6 => {
                },
                //Goaway
                0x7 => {
                },
                //WindowsUpdate
                0x8 => {
                },
                //Continuation
                0x9 => {
                    self.handle_continuation(true, &frame);
                },
                _ => {
                    // should not enter here
                }
            };
            true
        }
    }

    // Assumes valid header frame for state
    //A HEADERS frame carries the END_STREAM flag that signals the end of a stream.
    //However, a HEADERS frame with the END_STREAM flag set can be followed by CONTINUATION frames on the same stream.
    //Logically, the CONTINUATION frames are part of the HEADERS frame.
    fn handle_header (&mut self, by_peer: bool, frame: &RawFrame) { // parse frame here?
        let flag = frame.header.2;
        let stream_id = frame.header.3;

        // first check if stream is in hashmap
        let state = match self.get_stream_status(&stream_id) {
            None => StreamStates::Idle,
            Some(_status) => _status.state.clone(),
        };

        // if state is idle (not in hashmap), force open
        if state == StreamStates::Idle {
            println!("fn handle header: opening the stream... {:?}, ln299", &stream_id);
            self.open(stream_id, by_peer);
        };

        // finally, extract the streamstatus
        let mut status = self.get_stream_status(&stream_id).unwrap();

        // end stream set on headers should not close stream until 
        // end headers flag recieved on header/continuation frame

        // todo: modify streamstatus to check if the stream should end if
        // expect continue is false and should end is true
        // transition state if condition passes

        //todo: bitmask to extract relevant flag
        //endheaders
        if flag == 0x4 {
            match status.state {
                StreamStates::ReservedLocal if !by_peer => {
                    status.set_state(StreamStates::HalfClosedRemote);
                },

                StreamStates::ReservedRemote if by_peer => {
                    status.set_state(StreamStates::HalfClosedLocal);
                },
                _ => {// Has already, transitioned to Open from Idle and should stay open
                    // Half Closed, Closed should have erred in validiy checks
                }
            }
            status.set_continue(false);
        } else {
            status.set_continue(true);
        };
        // increment active stream count

        //todo: bitmask to extract relevant flag
        //endstream
        if flag == 0x1 {
            status.set_end(true);

            match status.state {
                //if open and send/recv end stream flag
                StreamStates::Open => {
                    if by_peer {
                        // if recv -> half_closed remote 
                        status.set_state(StreamStates::HalfClosedRemote);
                    } else {
                        //if send -> half closed local
                        status.set_state(StreamStates::HalfClosedLocal);
                    }
                },

                // If Half Closed, and End Stream Flag and End Header Flag Set, close stream.
                StreamStates::HalfClosedRemote |
                StreamStates::HalfClosedLocal 
                if status.expects_continuation == false &&
                status.should_end == true => {
                    status.set_state(StreamStates::Closed);
                },

                _ => {
                    // should probably err
                }
            };
        };
    }

    // TODO: PP has very nuanced implementation details, take care that they are covered
    // eg ignoring/rejecting pp's
    fn handle_push_promise (&mut self, by_peer: bool, frame: &RawFrame) {
        let flag = frame.header.2;
        let stream_id = frame.header.3;
        
        // first check if stream is in hashmap
        let exists = match self.get_stream_status(&stream_id) {
            None => false,
            Some(_status) => true,
        };

        // if state is idle (not in hashmap), reserve it
        if !exists {
            self.open_idle(stream_id, by_peer);
            let mut status = self.get_stream_status(&stream_id).unwrap();
            match flag {
                // end header present
                0x4 => {
                    if by_peer {
                        status.set_state(StreamStates::ReservedRemote);
                    } else {
                        status.set_state(StreamStates::ReservedLocal);
                    };
                },

                _ => { // status: idle, continue: true
                    status.set_continue(true);
                }
            }
        } else {
            // should err with validity check
        }

    }

    // Continuation frames can only follow push promise or headers frames.
    // they may carry a end headers flag which could transition to halfclosed states
    fn handle_continuation (&mut self, by_peer: bool, frame: &RawFrame) {
        let flag = frame.header.2;
        let stream_id = frame.header.3;
        let status = self.get_stream_status(&stream_id).unwrap();

        match flag {
            //end header
            0x4 => { // continuation can imply 4 state transitions
                status.set_continue(false);

                match status.state {
                    StreamStates::Idle => { // initiated by push promise
                        if by_peer {
                            status.set_state(StreamStates::ReservedRemote);
                        } else {
                            status.set_state(StreamStates::ReservedLocal);
                        };
                    },

                    StreamStates::ReservedLocal if !by_peer => { // initiated by header in reserved
                        status.set_state(StreamStates::HalfClosedRemote);
                    },

                    StreamStates::ReservedRemote if by_peer => { // initiated by header in reserved
                        status.set_state(StreamStates::HalfClosedLocal);
                    },

                    //Transitioned from Open with ES flag
                    StreamStates::HalfClosedLocal if by_peer => {
                        //recv continuation frame with EH flag, should close stream
                    },
                    //Transitioned from Open with ES flag
                    StreamStates::HalfClosedRemote if !by_peer => {
                        //send continuation frame with EH should, should close stream
                    }
                    _ => (), //Open, should remain open
                    //Closed should have erred in validity check
                    //invalid continuation frame in half closed should err?
                }

                if status.should_end {
                    status.set_state(StreamStates::Closed);
                    //todo: update active streams
                };
            },
            // check_continue manages continuation expectations
            // if end header flag not set, expect continuation, state unchanged.
            _ => (),
        };
    }

    fn handle_rst_stream (&mut self, by_peer: bool, frame: &RawFrame) {
        // first check if stream is in hashmap
        let stream_id = frame.header.3;
        self.close(stream_id);
    }

    // Checks if the incoming frame is valid for the particular stream.
    // First identifies if incoming frame is associated with a stream
    // If not and is a valid header, opens the stream
    // Else, checks to see if it is a continuation frame
    // If not, does a final check for validity
    pub fn check_valid_frame(&mut self, frame: &RawFrame, by_peer: bool) -> bool {

        let frame_type = frame.header.1;
        let stream_id = frame.header.3;

        // Check to see if this id is valid and it is the beginning of a stream.
        let valid_so_far = match frame_type {
            // If Header or Push Promise, peer attempting to open/reserve a new stream
            // Stream id's must be increasing, respond to unexpected id's with PROTOCOL_ERROR
            // currently check_valid_frame is only used on recieve
            0x1 | 0x5 => {
                self.check_valid_open_request(stream_id, by_peer)
            },
            _ => {
                self.check_continue(&frame)
            }
        };

        println!("fn chk valid: valid so far? {:?}", valid_so_far);
        if valid_so_far {
            self.check_state(by_peer, &frame)
        } else {
            false
        }
    }

    fn check_continue (&mut self, frame: &RawFrame) -> bool {

        let frame_type = frame.header.1;
        let stream_id = frame.header.3;

        match self.get_stream_status(&stream_id) {
            // If id not in current list of streams, perhaps it's a new one.
            None => {
                // continuation not expected
                false
            },
            Some(status) => {
                // If stream is expecting continuation frame. Check if this is a continuation frame,
                println!("status: {:?} {:?}", status.expects_continuation, frame_type);
                match status.expects_continuation  {
                    true if frame_type != 0x9 => false, //should err
                    false if frame_type == 0x9 => false, //connection err?
                    _ => {
                        //check_state(by_peer, &status, &frame)
                        true
                    }
                }
            }
        }
    }

    fn check_state (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        // by the time check state is called, valid open request or valid continuation
        let stream_id = frame.header.3;

        let state = match self.get_stream_status(&stream_id) { //todo: match states based on by_peer
            None => StreamStates::Idle,
            Some(status) => status.state.clone(),
        };

        println!("State is {:?}", state);

        match state {
            StreamStates::Idle => self.check_idle(by_peer, &frame),
            StreamStates::Open => self.check_open(by_peer, &frame),
            StreamStates::Closed => self.check_closed(by_peer, &frame),
            StreamStates::ReservedLocal => self.check_reserved_local(by_peer, &frame),
            StreamStates::ReservedRemote => self.check_reserved_remote(by_peer, &frame),
            StreamStates::HalfClosedLocal => self.check_half_closed_local(by_peer, &frame),
            StreamStates::HalfClosedRemote => self.check_half_closed_remote(by_peer, &frame),
        }
    }
    

    // Helpers for each state, each state only allows a certain type of frames
    // This section effectively filters out invalid frames for a given stream state

    // All streams start in the **idle** state. In this state, no frames have been exchanged.
    //
    // * Sending or receiving a HEADERS frame causes the stream to become "open".
    //
    // When the HEADERS frame contains the END_STREAM flags, then two state transitions happen.
    fn check_idle (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        let frame_type = frame.header.1;
        match frame_type {
            0x1 => true, // headers -> half closed local | half closed remote if ES flag
            0x3 if !by_peer => true, // rst -> closed
            0x5 => true, //push promise
            0x20 => true, // priority
            _ => false // PROTOCOL_ERROR
        }
        // must Protocol Err if stream id is 0x0
    }

    // A stream in the **reserved (local)** state is one that has been promised by sending a
    // PUSH_PROMISE frame.
    //
    // * The endpoint can send a HEADERS frame. This causes the stream to open in a "half closed
    //   (remote)" state.
    // * Either endpoint can send a RST_STREAM frame to cause the stream to become "closed". This
    //   releases the stream reservation.
    // * An endpoint may receive PRIORITY frame in this state.
    // * An endpoint MUST NOT send any other type of frame in this state.
    fn check_reserved_local (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        let frame_type = frame.header.1;
        // Reserved Local
        // Associated with open stream initiated by remote peer
        // Send: HEADERS --> half closed (remote)
        // Send: Either endpoint RST_STREAM --> Closed
        match frame_type {
            0x1 if !by_peer => true, // send headers -> half closed remote
            0x9 if !by_peer => true, // continuation
            0x3 => true, // rst -> closed
            0x20 => true, // priority
            0x8 => true, // window update
            _ => false // PROTOCOL_ERROR
        }
    }

    // A stream in the **reserved (remote)** state has been reserved by a remote peer.
    //
    // * Either endpoint can send a RST_STREAM frame to cause the stream to become "closed". This
    //   releases the stream reservation.
    // * Receiving a HEADERS frame causes the stream to transition to "half closed (local)".
    // * An endpoint MAY send PRIORITY frames in this state to reprioritize the stream.
    // * Receiving any other type of frame MUST be treated as a stream error of type PROTOCOL_ERROR.
    fn check_reserved_remote (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        let frame_type = frame.header.1;
        // Reserved Remote
        // Reserved by remote peer
        match frame_type {
            0x1 if by_peer => true, //headers -> half closed local
            0x9 if by_peer => true, //continuation
            0x3 => true, // rst -> closed
            0x20 => true, // priority
            0x8 => true, // window update
            _ => false // PROTOCOL_ERROR
        }
    }

    // The **open** state is where both peers can send frames. In this state, sending peers observe
    // advertised stream level flow control limits.
    //
    // * From this state either endpoint can send a frame with a END_STREAM flag set, which causes
    //   the stream to transition into one of the "half closed" states: an endpoint sending a
    //   END_STREAM flag causes the stream state to become "half closed (local)"; an endpoint
    //   receiving a END_STREAM flag causes the stream state to become "half closed (remote)".
    // * Either endpoint can send a RST_STREAM frame from this state, causing it to transition
    //   immediately to "closed".
    fn check_open (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        let frame_type = frame.header.1;
        //priority
        //stream dependency
        //weight
        //end_headers?
        //if not next frame must be continuation
        match frame_type {
            0x3 => true, // rst -> closed
            0x9 => true, // continuation -> closed if ES flag
            _ => false // 
        }
    }

    // A stream that is **half closed (local)** cannot be used for sending frames.
    //
    // * A stream transitions from this state to "closed" when a frame that contains a END_STREAM
    //   flag is received, or when either peer sends a RST_STREAM frame.
    // * An endpoint MAY send or receive PRIORITY frames in this state to reprioritize the stream.
    // * WINDOW_UPDATE can be sent by a peer that has sent a frame bearing the END_STREAM flag.
    fn check_half_closed_local (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        let frame_type = frame.header.1;
        let flag = frame.header.2;
        // Half Closed Local (READING)
        // SEND: WINDOW_UPDATE, PRIORITY, RST Stream
        match frame_type { //TODO: clarify allowed frames
            0x9 => true, // continuation with eh?
            0x3 => true, // rst | ES flag -> closed
            0x20 => true,
            0x8 if !by_peer => true, // send window update
            _ => true // Can Recv any frame
        }
    }

    // A stream that is **half closed (remote)** is no longer being used by the peer to send frames.
    // In this state, an endpoint is no longer obligated to maintain a receiver flow control window
    // if it performs flow control.
    //
    // * If an endpoint receives additional frames for a stream that is in this state it MUST
    //   respond with a stream error of type STREAM_CLOSED.
    // * A stream can transition from this state to "closed" by sending a frame that contains a
    //   END_STREAM flag, or when either peer sends a RST_STREAM frame.
    // * An endpoint MAY send or receive PRIORITY frames in this state to reprioritize the stream.
    // * A receiver MAY receive a WINDOW_UPDATE frame on a "half closed (remote)" stream.
    fn check_half_closed_remote (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        let frame_type = frame.header.1;
        // Half Closed Remote (Writing)
        // no longer used by peer to send frames, no longer obligated to maintain reciever flow control window
        // Error w/ STREAM_CLOSED when when recv frames not WINDOW_UPDATE, PRIORITY, RST_STREAM
        match frame_type {
            0x9 => true, // continuation with eh?
            0x3 => true, // rst | ES flag -> closed
            0x20 => true,
            0x8 if by_peer => true, // recv window update
            _ => false // PROTOCOL_ERROR
        }
    }

    // The **closed** state is the terminal state.
    //
    // * An endpoint MUST NOT send frames on a closed stream. An endpoint that receives a frame
    //   after receiving a RST_STREAM or a frame containing a END_STREAM flag on that stream MUST
    //   treat that as a stream error of type STREAM_CLOSED.
    // * WINDOW_UPDATE, PRIORITY or RST_STREAM frames can be received in this state for a short
    //   period after a frame containing an END_STREAM flag is sent.  Until the remote peer receives
    //   and processes the frame bearing the END_STREAM flag, it might send either frame type.
    //   Endpoints MUST ignore WINDOW_UPDATE frames received in this state, though endpoints MAY
    //   choose to treat WINDOW_UPDATE frames that arrive a significant time after sending
    //   END_STREAM as a connection error of type PROTOCOL_ERROR.
    // * If this state is reached as a result of sending a RST_STREAM frame, the peer that receives
    //   the RST_STREAM might have already sent - or enqueued for sending - frames on the stream
    //   that cannot be withdrawn. An endpoint that sends a RST_STREAM frame MUST ignore frames that
    //   it receives on closed streams after it has sent a RST_STREAM frame. An endpoint MAY choose
    //   to limit the period over which it ignores frames and treat frames that arrive after this
    //   time as being in error.
    // * An endpoint might receive a PUSH_PROMISE frame after it sends RST_STREAM. PUSH_PROMISE
    //   causes a stream to become "reserved". If promised streams are not desired, a RST_STREAM
    //   can be used to close any of those streams.
    fn check_closed (&mut self, by_peer: bool, frame: &RawFrame) -> bool {
        // let frame_type = frame.header.1;
        // Closed
        // SEND: PRIORITY, else ERR
        // RECV: After RECV RST_STREAM or End_STREAM flag, if RECV anything ERR STREAM_CLOSED
        let frame_type = frame.header.1;

        match frame_type {
            0x20 => true,// priority
            0x3 if !by_peer => true, //sending rst
            _ => false // STREAM CLOSED
        }

        // for now to simplify, just return false

        //   notes: 
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
}


#[cfg(test)]
mod tests {
    use super::{
        StreamManager,
        StreamStates
    };

    use super::super::super::frame::{RawFrame, pack_header};

    //TODO: implement flag checks for stream states using bitmasking
    // a raw frame could have many flags set
    // Sets the given flag for the frame.
    // fn set_flag(&mut self, flag: HeadersFlag) {
    //     self.flags |= flag.bitmask();
    // }

    // BeforeEach?

    /// Builds a test frame of the given type with the given header and
    /// payload, by using the `Frame::from_raw` method.
    pub fn build_test_rawframe (stream_id: u32, frame_type: &str, flags: &str) -> RawFrame {
        let data = b"123";
        // (length, frame_type, flags, stream_id)

        let _flag = {
            match flags {
                "endstream" => 0x1,
                "endheaders" => 0x4,
                _ => 0x0
            }
        };

        let _type = {
            match frame_type {
                "data" => 0x0,
                "headers" => 0x1,
                "priority" => 0x20,
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

        let header = (data.len() as u32, _type, _flag, stream_id);
        let buf = {
            let mut buf = Vec::new();
            buf.extend(pack_header(&header).to_vec().into_iter());
            buf.extend(data.to_vec().into_iter());
            buf
        };
        RawFrame::from_buf(&buf).unwrap()
    }

    // Tests for the external API of StreamManager
    // Tests that check_valid allows the appropriate frames through, handlers are tested individually
    #[test]
    fn test_recv_frame () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");
        let raw_continue = build_test_rawframe(stream_id, "continuation", "endheaders");

        let check_pass1 = stream_manager.recv_frame(&raw_header);
        assert_eq!(check_pass1, true);
        //should be status: open, expect continue: true, should end: true

        println!("starting second test...............");
        let check_pass2 = stream_manager.recv_frame(&raw_continue);
        assert_eq!(check_pass2, true);
    }

    // Tests for Opening a stream
    #[test]
    fn test_open_stream () {
        let stream_id = 1;
        let mut stream_manager = StreamManager::new(4, false);

        stream_manager.open(stream_id, false);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
    }

    #[test]
    fn test_implicit_open () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");

        stream_manager.recv_frame(&raw_header);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
    }

    // Calling close closes a given stream
    #[test]
    fn test_close_stream () {
        let stream_id = 1;
        let mut stream_manager = StreamManager::new(4, false);
        stream_manager.open(stream_id, false);
        stream_manager.close(stream_id);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
    }

    // A new stream defaults to not expect continuation, receiving a continuation frame should immediately be rejected
    #[test]
    fn test_check_continue () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_continue = build_test_rawframe(stream_id, "continuation", "none");

        let check_fail = stream_manager.check_continue(&raw_continue);
        assert_eq!(check_fail, false);
    }

    // Tests check_valid_frame rejects inappropriate frames for a given stream.
    // A new open stream should not immediately accept continuation frames unless the expect_continuation is set.
    #[test]
    fn test_open_with_continuation () {

        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");
        let raw_continue = build_test_rawframe(stream_id, "continuation", "none");
        let raw_continue_end = build_test_rawframe(stream_id, "continuation", "endheaders");

        stream_manager.recv_frame(&raw_header);
        stream_manager.recv_frame(&raw_continue);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);

        stream_manager.recv_frame(&raw_continue_end);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
    }

    //Tests for handlers
    // Receiving a header with out a set flag 'opens' a new stream. The stream now expects continuation frames
    #[test]
    fn test_handle_header_continue () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");

        stream_manager.handle_header(true, &raw_header);

        let updated_server_id = stream_manager.last_server_id;
        let stream_status = stream_manager.get_stream_status(&stream_id).unwrap();

        // handle header(recv) should create a streamstatus with status: open and expect_continue to be true
        assert_eq!(stream_status.state, StreamStates::Open);
        assert_eq!(stream_status.expects_continuation, true);
        // the newly created stream should update the id
        assert_eq!(updated_server_id, 2);
    }

    // Receiving a header with the end stream flag transitions the stream to HalfClosedRemote
    #[test]
    fn test_handle_header_end_stream () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");

        stream_manager.handle_header(true, &raw_header);

        let updated_server_id = stream_manager.last_server_id;
        let stream_status = stream_manager.get_stream_status(&stream_id).unwrap();

        // handle header(recv) should create a streamstatus with status: closed and expect_continue to be false
        assert_eq!(stream_status.state, StreamStates::HalfClosedRemote);
        assert_eq!(stream_status.expects_continuation, true);
        assert_eq!(stream_status.should_end, true);
        // the newly created stream should update the id
        assert_eq!(updated_server_id, 2);
    }

    // Receiving a rst stream frame should close the stream
    // TODO: implement a configurable window for receiving incoming frames
    #[test]
    fn test_handle_rst () {
        let stream_id = 1;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_rst_stream = build_test_rawframe(stream_id, "rststream", "none");

        stream_manager.open(stream_id, false);
        stream_manager.handle_rst_stream(false, &raw_rst_stream);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
    }

    // Receiving a continuation frame with the end header flag set should disable continuation
    #[test]
    fn test_handle_continuation () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");
        let raw_continue_end = build_test_rawframe(stream_id, "continuation", "endheaders");

        stream_manager.recv_frame(&raw_header); // state should be open, expects continue

        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);

        stream_manager.handle_continuation(true, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
    }


    // Test for Data Frames

    // Connection Errors should close a stream

}
