//! This module implements stream state management to help discover connection errors before an expensive parse is made.
//! It manages the state of the streams associated with the connection

// TODO: Implement Control Flow
// TODO: Refactor using try!
// TODO: return errors where appropriate

use std::collections::HashMap;
use super::super::frame::{RawFrame};
use super::{StreamStates, StreamStatus};
use super::utils;
use super::handlers;

pub struct StreamManager {
    last_server_id: u32,
    last_client_id: u32,
    max_concurrent_streams: u32, // set by settings frame
    streams: HashMap<u32, StreamStatus>,
    is_server: bool, // todo: use enum instead of boolean?
    // notes: Stream Concurrency
    // Limit the max streams a peer can open using Settings Frame
    // Open/Half Closed count towards max. reserved don't
    // if endpoint recieves HEADERS frame that goes over max, PROTOCOL_ERROR or REFUSED_STREAM.
    // Can reduce max streams. but must either close streams or wait for them to close.
}

// Exposes an API to quickly check/manage the state of the stream
// Note: Perhaps could be used as an API on top of the connection
impl StreamManager {

    // Configures the connection's settings for all streams
    pub fn new (max_streams: u32, is_server: bool) -> StreamManager {
        StreamManager {
            last_client_id: 0, //TODO: Figure out proper defaults
            last_server_id: 0,
            max_concurrent_streams: max_streams,
            streams: HashMap::new(),
            is_server: is_server
            //enable push?
        }
    }

    // helper method to set the state of a stream.
    fn set_state (&mut self, stream_id: &u32, state: StreamStates) {
        match self.get_stream_status(&stream_id) {
            Some(stream) => {
                stream.set_state(state);
            },
            None => {
                () // note: should error
            },
        };
    }

    // Convenience wrapper for getting a stream's status using an id.
    pub fn get_stream_status (&mut self, stream_id: &u32) -> Option<&mut StreamStatus> {
        self.streams.get_mut(&stream_id)
    }

    // registers a stream as idle
    pub fn open_idle (&mut self, receiving: bool, stream_id: u32) {
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
    pub fn open (&mut self, stream_id: u32, receiving: bool) {

        //TODO: Check for Max Concurrency

        let not_set = match self.get_stream_status(&stream_id) {
            None => true,
            Some(status) => false,
        };

        // Exhaustively ensures the stream has not already been set
        // Checks if stream_id provided is valid
        if not_set {
            self.open_idle(receiving, stream_id);
        };

        // And only opens the stream if it was originally set to idle
        if self.streams[&stream_id].state == StreamStates::Idle {
            self.set_state(&stream_id, StreamStates::Open);
        };
        // else err, tried to manually open a stream that has transitioned
    }
    
    // Tests if the direction of the inbound/outbound frame is consistent with the id of the stream
    // Note: Leaving this method in stream manager keeps many of the stream manager's fields private
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

    // Recieves a RawFrame and does validation checks between the frame and the state of the associated stream.
    // If validated, returns true. Else, returns the error thrown during validation.
    // Also updates the state of the stream implied by the recieved frames.
    pub fn handle_frame (&mut self, receiving: bool, frame: &RawFrame) -> bool {
        // note: handle frame could be moved to handlers, but is more ergonomic here
        // Also, what happens if the raw frame to be processed errors? Connection Error and Close Stream?

        let frame_type = frame.header.1;
        let stream_id = frame.header.3;

        // If the frame is valid (is it a new stream?, a continuation?, is the frame allowed in this state?)
        let is_valid = utils::check_valid_frame(self, &frame, receiving);

        if !is_valid {
            //return an error
            //close stream due to connection error?
            debug!("failed initial checks, terminating... stream id: {:?}, frame type: {:?}, ", stream_id, frame_type);
            false
        } else {
            // Hands frame to appropriate handler which will trigger state changes,
            // note: could be used directly to parse frames as well in a deeper integration
            match frame_type {
                //Data
                0x0 => {
                    handlers::handle_data(self, receiving, &frame);
                },
                //Header
                0x1 => {
                    handlers::handle_header(self, receiving, &frame);
                },
                //Priority
                0x20 => {
                    handlers::handle_priority(self, receiving, &frame);
                },
                //RST
                0x3 => {
                    handlers::handle_rst_stream(self, receiving, &frame);
                },
                //Setting
                0x4 => {
                    //TODO: Implement/Integrate Handlers
                },
                //PushPromise
                0x5 => {
                    handlers::handle_push_promise(self, receiving, &frame);
                },
                //Ping
                0x6 => {
                },
                //Goaway
                0x7 => {
                },
                //WindowUpdate
                0x8 => {
                    handlers::handle_window_update(self, receiving, &frame);
                },
                //Continuation
                0x9 => {
                    handlers::handle_continuation(self, receiving, &frame);
                },
                _ => {
                    // should not enter here
                }
            };
            true
        }
    }
}

#[cfg(test)]
mod tests {
    // todo: refactor tests
    // would love to move tests into own module, naive export requires exposure of multiple struct fields.

    use super::super::super::frame::{RawFrame, pack_header};
    use super::super::{StreamStates, Flags, utils, handlers};
    use super::{
        StreamManager,
    };

    // todo: use method to set flags indea of hardcoding

    /// Builds a test frame of the given type with the given header and
    /// payload, by using the `Frame::from_raw` method.
    pub fn build_test_rawframe (stream_id: u32, frame_type: &str, flags: &str) -> RawFrame {
        let data = b"123";

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

        // (length, frame_type, flags, stream_id)
        let header = (data.len() as u32, _type, _flag, stream_id);
        let buf = {
            let mut buf = Vec::new();
            buf.extend(pack_header(&header).to_vec().into_iter());
            buf.extend(data.to_vec().into_iter());
            buf
        };
        RawFrame::from_buf(&buf).unwrap()
    }

    // Should return true if a given flag has been set
    #[test]
    fn test_bitmask_flag () {
        let check_pass = Flags::EndStream.is_set(0x1);
        assert_eq!(check_pass, true);
    }

    // Tests for the external API of StreamManager
    //
    // Check_valid should allow the appropriate frames through (Handlers are tested individually)
    #[test]
    fn test_handle_frame () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");
        let raw_continue = build_test_rawframe(stream_id, "continuation", "endheaders");

        let check_pass1 = stream_manager.handle_frame(true, &raw_header);
        //should be status: open, expect continue: true, should end: true, but is covered by specific handler
        assert_eq!(check_pass1, true);

        let check_pass2 = stream_manager.handle_frame(true, &raw_continue);
        //should be status: open, expect continue: false, should end: true, but is covered by specific handler
        assert_eq!(check_pass2, true);
    }

    // Tests for Opening and closing stream by sending/receiving frames
    //
    // Calling open should open a new stream
    #[test]
    fn test_open_stream () {
        let stream_id = 1;
        let mut stream_manager = StreamManager::new(4, false);

        //opening a stream locally as a client
        stream_manager.open(stream_id, false);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
    }

    // Handling a recived headers frame should correctly open a new stream
    #[test]
    fn test_implicit_open () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");

        stream_manager.handle_frame(true, &raw_header);
        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Open);
    }

    // Calling close should close a given stream
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

        let check_fail = utils::check_continue(&mut stream_manager, &raw_continue);
        assert_eq!(check_fail, false);
    }

    // Check_valid_frame should reject inappropriate frames for a given stream.
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

    // Tests for handlers
    //
    // Receiving a header 'opens' a new stream. The stream now expects continuation frames if the end header flag is not set
    // Aslo, the tracker for created stream ids should update
    #[test]
    fn test_handle_header_continue () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "none");

        handlers::handle_header(&mut stream_manager, true, &raw_header);

        let updated_server_id = stream_manager.last_server_id;
        let stream_status = stream_manager.get_stream_status(&stream_id).unwrap();

        // handle header(recv) should create a streamstatus with status: open and expect_continue to be true
        assert_eq!(stream_status.state, StreamStates::Open);
        assert_eq!(stream_status.expects_continuation, true);
        // the newly created stream should update the id
        assert_eq!(updated_server_id, 2);
    }

    // Receiving a header with the end stream flag should transition the stream to HalfClosedRemote
    #[test]
    fn test_handle_header_end_stream () {
        let stream_id = 2;
        let mut stream_manager = StreamManager::new(4, false);
        let raw_header = build_test_rawframe(stream_id, "headers", "endstream");

        handlers::handle_header(&mut stream_manager, true, &raw_header);

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
        handlers::handle_rst_stream(&mut stream_manager, false, &raw_rst_stream);

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

        handlers::handle_continuation(&mut stream_manager, true, &raw_continue_end);
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

        handlers::handle_push_promise(&mut stream_manager, true, &raw_ppromise);

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
        handlers::handle_data(&mut stream_manager, true, &raw_data);

        assert_eq!(stream_manager.streams[&stream_id].state, StreamStates::Closed);
    }

    //Tests for integrated transitions from Open to Closed
    //todo: refactor tests to be less redudant while maintaining integration garuntees
    //
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

    // Idle -> ResRemote -> HalfClosed Local -> Closed (without continuation)
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

    // Idle -> ResRemote -> HalfClosed Local -> Closed (with continuation)
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

    // Mirored test for Sending frames
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

    // Connection Errors should close a stream
}
