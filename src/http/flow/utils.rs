use super::super::frame::{RawFrame};
use super::streammanager::{StreamManager};
use super::StreamStates;

// This helper function is used to check the validity of a inbound/outbound frame on a given stream
//
// First identifies if the frame is trying to open a stream with a valid stream id using check_valid_open_request (located in StreamManager)
// Else, checks to see if it is a continuation frame using check_continue
// If the first two are valid, does a final check for validity using check_<frame>
pub fn check_valid_frame(stream_manager: &mut StreamManager, frame: &RawFrame, receiving: bool) -> bool {

    let frame_type = frame.header.1;
    let stream_id = frame.header.3;

    let valid_so_far = match frame_type {
        0x1 => {
            //todo: refactor uncessary step?
            true // handled by header handler
        },
        0x5 => {
            // Push Promise, peer is attempting to open/reserve a new stream
            // Check to see if this id is valid and it is the beginning of a stream.
            // Stream id must be increasing, respond to unexpected id's with PROTOCOL_ERROR
            stream_manager.check_valid_open_request(stream_id, receiving)
        },
        _ => {
            check_continue(stream_manager, &frame)
        }
    };

    // returns a state that is either Idle if this is a new stream, or the actual state of the stream.
    let state = match stream_manager.get_stream_status(&stream_id) {
        None => StreamStates::Idle,
        Some(status) => status.state.clone(), // note: borrow checker restricts moving the state, refactor?
    };

    // by the time check state is called, it is either an open valid open request or matches continuation expectations
    if valid_so_far {
        check_state(receiving, state, &frame)
    } else {
        false
    }
}

// Checks if the frame sent/recieved is a continuation frame if continuation is expected
// Also checks if the current frame is a continuation frame and the stream is not expecting one
// Currently, this check is run against all frames excluding headers and push promise frames
pub fn check_continue (stream_manager: &mut StreamManager, frame: &RawFrame) -> bool {

    let frame_type = frame.header.1;
    let stream_id = frame.header.3;

    match stream_manager.get_stream_status(&stream_id) {
        // If id not in current list of streams, perhaps it's a new one.
        None => {
            // continuation not expected
            false
        },
        Some(status) => {
            // If stream is expecting continuation frame. Check if this is a continuation frame,
            match status.expects_continuation  {
                true if frame_type != 0x9 => false, //should err
                false if frame_type == 0x9 => false, //connection err?
                _ => {
                    true
                }
            }
        }
    }
}

// Checks if the frame sent/recieved is valid for a given state
// This is the last of a series of checks against unexpected stream ids and the continuation status of a stream
fn check_state (receiving: bool, state: StreamStates, frame: &RawFrame) -> bool {
    // by the time check state is called, valid open request or valid continuation
    let stream_id = frame.header.3;

    match state {
        StreamStates::Idle => check_idle(receiving, &frame),
        StreamStates::Open => check_open(receiving, &frame),
        StreamStates::Closed => check_closed(receiving, &frame),
        StreamStates::ReservedLocal => check_reserved_local(receiving, &frame),
        StreamStates::ReservedRemote => check_reserved_remote(receiving, &frame),
        StreamStates::HalfClosedLocal => check_half_closed_local(receiving, &frame),
        StreamStates::HalfClosedRemote => check_half_closed_remote(receiving, &frame),
    }
}


// Helpers for each state, each state only allows a certain type of frames
// This section effectively filters out invalid frames for a given stream state

// All streams start in the **idle** state. In this state, no frames have been exchanged.
//
// * Sending or receiving a HEADERS frame causes the stream to become "open".
//
// When the HEADERS frame contains the END_STREAM flags, then two state transitions happen.
fn check_idle (receiving: bool, frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    match frame_type {
        0x1 => true, // headers -> half closed local | half closed remote if ES flag
        0x3 if !receiving => true, // rst -> closed
        0x5 => true, //push promise
        0x9 => true, //continuation
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
fn check_reserved_local (receiving: bool, frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Reserved Local
    // Associated with open stream initiated by remote peer
    // Send: HEADERS --> half closed (remote)
    // Send: Either endpoint RST_STREAM --> Closed
    match frame_type {
        0x1 if !receiving => true, // send headers -> half closed remote
        0x9 if !receiving => true, // continuation
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
fn check_reserved_remote (receiving: bool, frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Reserved Remote
    // Reserved by remote peer
    match frame_type {
        0x1 if receiving => true, //headers -> half closed local
        0x9 if receiving => true, //continuation
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
fn check_open (receiving: bool, frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    //priority
    //stream dependency
    //weight
    //end_headers?
    //if not next frame must be continuation
    match frame_type {
        0x0 => true, // data
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
fn check_half_closed_local (receiving: bool, frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    let flag = frame.header.2;
    // Half Closed Local (READING)
    // SEND: WINDOW_UPDATE, PRIORITY, RST Stream
    match frame_type { //TODO: clarify allowed frames
        0x0 if receiving => true, // data
        0x9 => true, // continuation with eh?
        0x3 => true, // rst | ES flag -> closed
        0x20 => true,
        0x8 if !receiving => true, // send window update
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
fn check_half_closed_remote (receiving: bool, frame: &RawFrame) -> bool {
    let frame_type = frame.header.1;
    // Half Closed Remote (Writing)
    // no longer used by peer to send frames, no longer obligated to maintain reciever flow control window
    // Error w/ STREAM_CLOSED when when recv frames not WINDOW_UPDATE, PRIORITY, RST_STREAM
    match frame_type {
        0x0 if !receiving => true, //data
        0x9 => true, // continuation with eh?
        0x3 => true, // rst | ES flag -> closed
        0x20 => true,
        0x8 if receiving => true, // recv window update
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
fn check_closed (receiving: bool, frame: &RawFrame) -> bool {
    // let frame_type = frame.header.1;
    // Closed
    // SEND: PRIORITY, else ERR
    // RECV: After RECV RST_STREAM or End_STREAM flag, if RECV anything ERR STREAM_CLOSED
    let frame_type = frame.header.1;

    match frame_type {
        0x20 => true,// priority
        0x3 if !receiving => true, //sending rst
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
