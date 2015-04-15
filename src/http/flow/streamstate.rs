//! This module implements stream state management to help discover connection errors before an expensive parse is made.

//use super::super::{HttpError, HttpResult, StreamId};
//use super::super::frame::{RawFrame, FrameHeader, unpack_header};

use std::collections::HashMap;
use super::super::frame::{RawFrame};

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

pub struct StreamManager {//is struct necessary? why not put it on a single hashmap
    // Stream Concurrency
    // Limit the max streams a peer can open using Settings Frame
    // Open/Half Closed count towards max. reserved don't
    // if endpoint recieves HEADERS frame that goes over max, PROTOCOL_ERROR or REFUSED_STREAM.
    // Can reduce max streams. but must either close streams or wait for them to close.
    pub max_concurrent_streams: u32, //config?
    pub streams: HashMap<u32, StreamStates>, //streams should perhaps default to idle
    is_client: bool, // client or server?
    // next_stream_id: u32 currently handled by SimpleClient (the next stream to be opened/reserved)
}

// Perhaps could be used as an abstraction on top of the connection
// For now is used to quickly check the state of the stream
impl StreamManager {

    // Configures the stream's settings
    pub fn new (max_streams: u32, is_client: bool) -> StreamManager {
        StreamManager {
            max_concurrent_streams: max_streams,
            streams: HashMap::new(),
            is_client: is_client
        }
        //should perhaps initialize the first stream to be open as well.
    }

    fn set_state (&mut self, stream_id: u32, state: StreamStates) {
        debug!("new state {:?}", &stream_id);
        self.streams.insert(stream_id, state);
    }

    pub fn open (&mut self, id: u32) {
        debug!("opening stream {:?}", &id);
        //clients can only open odd streams, servers even.
        //potentially create streams using this api.

        // STREAM_ID: u31
        // init by Server: Even
        if self.is_client && id % 2 == 1 {
            self.set_state(id, StreamStates::Open);
        // init by Client: Odd
        } else if !self.is_client && id % 2 == 0 {
            self.set_state(id, StreamStates::Open);
        };
    }

    pub fn close (&mut self, id: u32) {
        //potentially close streams using this api.
        debug!("closing stream {:?}", &id);
        self.set_state(id, StreamStates::Closed);
    }

    // send

    // recv

    // Checks if the incoming frame is valid for the particular stream.
    pub fn check_valid_frame(&mut self, frame: &RawFrame) -> bool {
        // differentiate send and recieve?

        let (length, frame_type, flags, stream_id) = frame.header;
        //if you can't get the id, maybe it's a new one
        //check to see if this id is valid and it is the beginning of a stream. set status.

        // Stream id must be increasing, respond to unexpected id's with PROTOCOL_ERROR

        match self.streams[&stream_id] {
            StreamStates::Idle => self.check_idle(&frame), // perhaps pass the actual frame as well
            StreamStates::Open => self.check_open(&frame),
            StreamStates::Closed => self.check_closed(&frame),
            StreamStates::ReservedLocal => self.check_reserved_local(&frame),
            StreamStates::ReservedRemote => self.check_reserved_remote(&frame),
            StreamStates::HalfClosedLocal => self.check_half_closed_local(&frame),
            StreamStates::HalfClosedRemote => self.check_half_closed_remote(&frame),
        }
        // perhaps return an option with error instead of bool
    }

    fn transition_state () { // takes a frame
        //checks the implied state transition
        //applies transition
        //self.set_state(stream_id, state)
    }

    //flow control to handle data frame?

    // helpers for each state, each state only allows a certain type of frames
    // TODO: implement acutal filters
    fn check_idle (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
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

    fn check_open (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
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

    fn check_closed (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
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

    fn check_reserved_local (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
        // Reserved Local
        // Associated with open stream initiated by remote peer
        // Send: HEADERS --> half closed (remote)
        // Send: Either endpoint RST_STREAM --> Closed

        match frame_type {
        // May RECV: PRIORITY/WINDOW_UPDATE
            0x2 | 0x8 => true,
            _ => false
        }
    }

    fn check_reserved_remote (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
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

    fn check_half_closed_local (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
        // Half Closed Local (READING)
        // SEND: WINDOW_UPDATE, PRIORITY, RST Stream
        
        match frame_type {
            0x3 => true, // End_STREAM flag or RST_STREAM --> Close
            _ => true, // Can Recv any frame
        }
    }

    fn check_half_closed_remote (&mut self, frame: &RawFrame) -> bool {
        let (length, frame_type, flags, stream_id) = frame.header;
        // Half Closed Remote (Writing)
        // no longer used by peer to send frames, no longer obligated to maintain reciever flow control window
        // Error w/ STREAM_CLOSED when when recv frames not WINDOW_UPDATE, PRIORITY, RST_STREAM
        false
    }

}

#[cfg(test)]
mod tests {
    use super::{
        StreamManager,
        StreamStates
    };

    use super::super::super::frame::{RawFrame, pack_header};

    /// Builds a test frame of the given type with the given header and
    /// payload, by using the `Frame::from_raw` method.
    pub fn build_test_rawframe (frame_type: &str, stream_id: u32) -> RawFrame {
        let data = b"123";
        // (length, frame_type, flags, stream_id)

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

        let header = (data.len() as u32, _type, 0, stream_id);
        let buf = {
            let mut buf = Vec::new();
            buf.extend(pack_header(&header).to_vec().into_iter());
            buf.extend(data.to_vec().into_iter());
            buf
        };
        RawFrame::from_buf(&buf).unwrap()
    }

    #[test]
    fn test_open_stream () {
        let mut stream_manager = StreamManager::new(4, true);
        stream_manager.open(1);
        assert_eq!(stream_manager.streams[&1], StreamStates::Open);
    }

    #[test]
    fn test_check_valid_frame () {
        let mut stream_manager = StreamManager::new(4, true);
        stream_manager.open(1);

        let raw = build_test_rawframe("continuation", 1);

        let check = stream_manager.check_valid_frame(&raw);
        assert_eq!(check, true);
    }
}
