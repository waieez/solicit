//! This module implements stream state management to help discover connection errors before an expensive parse is made.
//! It manages the state of the streams associated with the connection

// TODO: Implement Control Flow, Priority
// TODO: Refactor using try!
// TODO: return errors where appropriate

//use super::super::{HttpError, HttpResult, StreamId};
//use super::super::frame::{RawFrame, FrameHeader, unpack_header};

use std::collections::HashMap;
use super::super::frame::{RawFrame, Flag};
use super::{StreamStates, StreamStatus, Flags};
use super::utils;

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
            //enable push?
        }
        //should perhaps initialize the first stream to be open as well.
    }

    // helper method to set the state of a stream.
    fn set_state (&mut self, stream_id: &u32, state: StreamStates) {
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
    pub fn get_stream_status (&mut self, stream_id: &u32) -> Option<&mut StreamStatus> {
        self.streams.get_mut(&stream_id)
    }

    // todo: refactor open, open_idle, etc signatures to match recieving, stream_id
    fn open_idle (&mut self, stream_id: u32, receiving: bool) {
        //clients can only open odd streams, servers even.
        //potentially create streams using this api.
        if self.check_valid_open_request(stream_id, receiving) { // checked before already in check_valid_frame
            self.streams.insert(stream_id, StreamStatus::new());

            //if it passes check_valid, update the appropriate id to the newly opened one.
            match stream_id % 2 == 0 {
                true => self.last_server_id = stream_id,
                false => self.last_client_id = stream_id,
            }
        }
    }

    // API to manually set the state of a stream to be open (if stream id supplied is valid)
    fn open (&mut self, stream_id: u32, receiving: bool) {// maybe defaults to false?

        //TODO: Check for Max Concurrency

        let not_set = match self.get_stream_status(&stream_id) {
            None => true,
            Some(status) => false,
        };

        // Exhaustively ensures the stream has not already been set
        // Checks if stream_id provided is valid
        if not_set {
            self.open_idle(stream_id, receiving);
        };

        // And only opens the stream if it was originally set to idle
        if self.streams[&stream_id].state == StreamStates::Idle {
            self.set_state(&stream_id, StreamStates::Open);
        };
        // else err, tried to manually open a stream that has transitioned
    }

    // fn open_half_closed?

    // peer argument should be more descriptive
    pub fn check_valid_open_request (&mut self, stream_id: u32, receiving: bool) -> bool {
        match receiving {
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
        self.set_state(&stream_id, StreamStates::Closed);
    }

    // fn send_frame?

    // Recieves a RawFrame and does validation checks between the frame and the state of the associated stream.
    // If validated, returns true. Else, returns the error thrown during validation.
    // Also updates the state of the stream implied by the recieved frames.
    pub fn handle_frame (&mut self, receiving: bool, frame: &RawFrame) -> bool {
        // What happens if the raw frame to be processed errors? Connection Error and Close Stream?

        // let length = frame.header.0;
        let frame_type = frame.header.1;
        // let flag = frame.header.2;
        let stream_id = frame.header.3;

        // If the frame is valid, (new stream?, continuation?, otherwise still valid?)
        let is_valid = utils::check_valid_frame(self, &frame, receiving); //receiving?

        if !is_valid {
            //return an error
            //close stream due to connection error?
            debug!("failed initial checks, terminating... stream id: {:?}, frame type: {:?}, ", stream_id, frame_type);
            false
        } else {
            // state transition, by peer
            // self.transition_state(&frame, false);

            // process frame?
            // return a processed frame?
            match frame_type {
                //Data
                0x0 => {
                    self.handle_data(receiving, &frame);
                },
                //Header
                0x1 => {
                    self.handle_header(receiving, &frame);
                },
                //Priority
                0x20 => {
                    self.handle_priority(receiving, &frame);
                },
                //RST
                0x3 => {
                    self.handle_rst_stream(receiving, &frame);
                },
                //Setting
                0x4 => {
                },
                //PushPromise
                0x5 => {
                    self.handle_push_promise(receiving, &frame);
                },
                //Ping
                0x6 => {
                },
                //Goaway
                0x7 => {
                },
                //WindowUpdate
                0x8 => {
                    self.handle_window_update(receiving, &frame);
                },
                //Continuation
                0x9 => {
                    self.handle_continuation(receiving, &frame);
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
    fn handle_header (&mut self, receiving: bool, frame: &RawFrame) { // parse frame here?
        let flag = frame.header.2;
        let stream_id = frame.header.3;

        // first check if stream is in hashmap
        let state = match self.get_stream_status(&stream_id) {
            None => StreamStates::Idle,
            Some(_status) => _status.state.clone(),
        };

        // if state is idle and not in hashmap, force open
        // collision with push promise should not be an issue, since push promises with EH flag set transitions to reserved states
        // also, header would be rejected by continuation checks if the EH flag is not set
        if state == StreamStates::Idle {
            self.open(stream_id, receiving);
        };

        // finally, extract the streamstatus
        let mut status = self.get_stream_status(&stream_id).unwrap();

        // Priority Flag

        if Flags::EndHeaders.is_set(flag) {
            match status.state {
                StreamStates::ReservedLocal if !receiving => {
                    status.set_state(StreamStates::HalfClosedRemote);
                },

                StreamStates::ReservedRemote if receiving => {
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

        if Flags::EndStream.is_set(flag) {
            status.set_end(true);

            match status.state {
                //if open and send/recv end stream flag
                StreamStates::Open => {
                    if receiving {
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
                    //decrement active stream count
                },

                _ => {
                    // should probably err
                }
            };
        };
    }

    // TODO: PP has very nuanced implementation details, take care that they are covered
    // eg ignoring/rejecting pp's
    fn handle_push_promise (&mut self, receiving: bool, frame: &RawFrame) {
        let flag = frame.header.2;
        let stream_id = frame.header.3;
        
        // first check if stream is in hashmap
        let exists = match self.get_stream_status(&stream_id) {
            None => false,
            Some(_status) => true,
        };

        // if state is idle (not in hashmap), reserve it
        if !exists {
            self.open_idle(stream_id, receiving);
            let mut status = self.get_stream_status(&stream_id).unwrap();
            status.set_reserved(true);

            if Flags::EndHeaders.is_set(flag) {
                if receiving {
                    status.set_state(StreamStates::ReservedRemote);
                } else {
                    status.set_state(StreamStates::ReservedLocal);
                };
                status.set_continue(false);
            } else {
                status.set_continue(true);
            };

        } else {
            // should err with validity check
        };

    }

    // Continuation frames can only follow push promise or headers frames.
    // they may carry a end headers flag which could transition to halfclosed states
    fn handle_continuation (&mut self, receiving: bool, frame: &RawFrame) {
        let flag = frame.header.2;
        let stream_id = frame.header.3;
        let status = self.get_stream_status(&stream_id).unwrap();

        if Flags::EndHeaders.is_set(flag) {
            status.set_continue(false);

            match status.state {
                StreamStates::Idle if status.is_reserved => { // initiated by push promise
                    if receiving {
                        status.set_state(StreamStates::ReservedRemote);
                    } else {
                        status.set_state(StreamStates::ReservedLocal);
                    };
                    status.set_reserved(false);
                },

                StreamStates::ReservedLocal if !receiving => { // initiated by header in reserved
                    status.set_state(StreamStates::HalfClosedRemote);
                },

                StreamStates::ReservedRemote if receiving => { // initiated by header in reserved
                    status.set_state(StreamStates::HalfClosedLocal);
                },

                _ => (), //Open, should remain open
                //Closed should have erred in validity check
                //invalid continuation frame in half closed should err?
            };

            if status.should_end {
                status.set_state(StreamStates::Closed);
                //todo: update active streams
            };
        };
    }

    // Data frames are subject to flow control, 
    // half-closed streams transition to closed if the End Stream flag is set
    fn handle_data (&mut self, receiving: bool, frame: &RawFrame) {
        let flag = frame.header.2;
        let stream_id = frame.header.3;

        // Flow Control

        if Flags::EndStream.is_set(flag) {
            self.close(stream_id);
        } else {
            // process frame?
        };

    }

    //Updates the priority status fields of a given stream
    fn handle_priority (&mut self, receiving: bool, frame: &RawFrame) {
        //actually requires parsing the frame to extract relevant info
        //the streams it depends on
        //exclusive?
    }

    //modifes flow control for the stream
    fn handle_window_update (&mut self, receiving: bool, frame: &RawFrame) {
        //also requires parsing the frame to extract information
    }

    // Closes the stream
    fn handle_rst_stream (&mut self, receiving: bool, frame: &RawFrame) {
        // first check if stream is in hashmap
        let stream_id = frame.header.3;
        self.close(stream_id);
    }
}


#[cfg(test)]
mod tests {
    use super::super::super::frame::{RawFrame, pack_header};
    use super::super::{StreamStates, Flags, utils};
    use super::{
        StreamManager,
    };

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

    // pub enum Flags {
    //     EndStream = 0x1,
    //     EndHeaders = 0x4,
    //     Padded = 0x8,
    //     Priority = 0x20,
    // }

    //tests for incoming flags
    #[test]
    fn test_bitmask_flag () {
        let check_pass = Flags::EndStream.is_set(0x1);
        assert_eq!(check_pass, true);
    }

    // Tests for the external API of StreamManager
    // Tests that check_valid allows the appropriate frames through, handlers are tested individually
    #[test]
    fn test_handle_frame () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");
        let raw_continue = build_test_rawframe(stream_id, "continuation", "endheaders");

        let check_pass1 = stream_manager.handle_frame(true, &raw_header);
        assert_eq!(check_pass1, true);
        //should be status: open, expect continue: true, should end: true

        println!("starting second test...............");
        let check_pass2 = stream_manager.handle_frame(true, &raw_continue);
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

        stream_manager.handle_frame(true, &raw_header);
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

    //TODO: move/refactor test
    // A new stream defaults to not expect continuation, receiving a continuation frame should immediately be rejected
    #[test]
    fn test_check_continue () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_continue = build_test_rawframe(stream_id, "continuation", "none");

        let check_fail = utils::check_continue(&mut stream_manager, &raw_continue);
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

        stream_manager.handle_frame(true, &raw_header);
        stream_manager.handle_frame(true, &raw_continue);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);

        stream_manager.handle_frame(true, &raw_continue_end);

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

        stream_manager.handle_frame(true, &raw_header); // state should be open, expects continue

        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);

        stream_manager.handle_continuation(true, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
    }

    // Receiving a PP frame should transition the state to Reserved Remote
    #[test]
    fn test_handle_push_promise () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_ppromise = build_test_rawframe(stream_id, "pushpromise", "none");
        let raw_continue_end = build_test_rawframe(stream_id, "continuation", "endheaders");

        stream_manager.handle_push_promise(true, &raw_ppromise);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Idle);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);
        assert_eq!(stream_manager.streams[&stream_id].is_reserved, true);
    }

    // Receiving a Data frame with End Stream flag set should transition state from half closed to closed
    #[test]
    fn test_handle_data () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_data = build_test_rawframe(stream_id, "data", "endstream");

        stream_manager.open(stream_id, true);
        stream_manager.set_state(&stream_id, StreamStates::HalfClosedLocal);
        stream_manager.handle_data(true, &raw_data);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
    }

    //Tests for flow control
    //todo: refactor tests to be less redudant while maintaining integration garuntees

    // Idle -> Open -> Closed
    #[test]
    fn test_simple_integration () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");
        let raw_continue_end = build_test_rawframe(stream_id, "continuation", "endheaders");
        let raw_rst = build_test_rawframe(stream_id, "rststream", "none");

        stream_manager.handle_frame(true, &raw_header);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);

        stream_manager.handle_frame(true, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);

        stream_manager.handle_frame(true, &raw_rst);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
    }

    // Idle -> ResRemote -> HalfClosed Local -> Closed
    #[test]
    fn test_simple_push_promise_integration () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_ppromise = build_test_rawframe(stream_id, "pushpromise", "endheaders");
        let raw_header = build_test_rawframe(stream_id, "headers", "endheaders");
        let raw_rst = build_test_rawframe(stream_id, "rststream", "none");

        // Peer Reserves Stream, RECV PP w/ EH flag set on continuation frame
        stream_manager.handle_frame(true, &raw_ppromise);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::ReservedRemote);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
        assert_eq!(stream_manager.streams[&stream_id].is_reserved, true);

        // peer sends headers, transitions to half closed local
        stream_manager.handle_frame(true, &raw_header);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::HalfClosedLocal);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);

        //Recv a rst frame, transition to closed
        stream_manager.handle_frame(true, &raw_rst);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
    }

    #[test]
    fn test_recv_push_promise_integration_with_continuation () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_ppromise = build_test_rawframe(stream_id, "pushpromise", "none");
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");
        let raw_continue_end = build_test_rawframe(stream_id, "continuation", "endheaders");
        let raw_rst = build_test_rawframe(stream_id, "rststream", "none");

        // Peer Reserves Stream, RECV PP w/ EH flag set on continuation frame
        //State should be Idle until continuation with EH flag set
        stream_manager.handle_frame(true, &raw_ppromise);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Idle);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);
        assert_eq!(stream_manager.streams[&stream_id].is_reserved, true);

        //Recv a continuation frame with EH, transition to reserved state
        stream_manager.handle_frame(true, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::ReservedRemote);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);

        //Recv header transition stream until continuation ends
        stream_manager.handle_frame(true, &raw_header);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::ReservedRemote);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);


        //Recv a continuation frame with EH, transition to reserved half closed
        stream_manager.handle_frame(true, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
    }

    //mirrored test for sending
    #[test]
    fn test_send_push_promise_integration_with_continuation () {
        let stream_id = 1;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_ppromise = build_test_rawframe(stream_id, "pushpromise", "none");
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");
        let raw_continue_end = build_test_rawframe(stream_id, "continuation", "endheaders");
        let raw_rst = build_test_rawframe(stream_id, "rststream", "none");

        // Reserves Stream, Send PP w/ EH flag set on continuation frame
        //State should be Idle until continuation with EH flag set
        stream_manager.handle_frame(false, &raw_ppromise);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Idle);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);
        assert_eq!(stream_manager.streams[&stream_id].is_reserved, true);

        //Send a continuation frame with EH, transition to reserved state
        stream_manager.handle_frame(false, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::ReservedLocal);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);

        //Recv header transition stream until continuation ends
        stream_manager.handle_frame(false, &raw_header);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::ReservedLocal);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, true);


        //Recv a continuation frame with EH, transition to reserved half closed
        stream_manager.handle_frame(false, &raw_continue_end);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
        assert_eq!(stream_manager.streams[&stream_id].expects_continuation, false);
    }

    // Test for Data Frames

    // Connection Errors should close a stream
}
