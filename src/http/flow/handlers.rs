/// This module contains helper methods used by StreamManager to update the state of a given stream.
/// note: could possibly be a good place for integrating logic for handing priority and parsing frames


use super::super::frame::{RawFrame};
use super::streammanager::StreamManager;
use super::{StreamStates, Flags};

/// Handler for Header Frames
///
/// Header frames transition from Idle to Open or Reserved to Half Closed if the stream was initiated by a Push Promise reservation
pub fn handle_header (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    let flag = frame.header.2;
    let stream_id = frame.header.3;

    // first check if stream is in hashmap
    let state = match stream_manager.get_stream_status(&stream_id) {
        None => StreamStates::Idle,
        Some(_status) => _status.state.clone(),
    };

    // if state is idle and not in hashmap, force open
    // collision with push promise should not be an issue, since push promises with EH flag set transitions to reserved states
    // also, header would be rejected by continuation checks if the EH flag is not set
    if state == StreamStates::Idle {
        stream_manager.open(stream_id, receiving);
    };

    // finally, extract the streamstatus
    let mut status = stream_manager.get_stream_status(&stream_id).unwrap();

    // todo: check for Priority Flag

    // If set, Endheader triggers the transitions from Reserved to HalfClosed
    // if not set, the stream should expect continuation frames until a Endheader flag is set on a continuation frames that follow
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
    // todo: increment active stream count

    // If set, EndStream will transition a stream from Open to Half Closed
    // The stream should transition to closed as soon as a EndHeader Flag is set
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

/// Handler for Push Promise
///
/// If allowed, Push Promise frames are used to reserve streams for later use
/// The state transition triggered by a Push Promise depends on wether the frame was sent or recieved
/// and will transition the state of the stream to Reserved when the EndHeader flag is set (either on itself, or a following Continuation Frame)
///
/// TODO: PP has very nuanced implementation details, take care that they are covered (eg ignoring/rejecting pp's)
pub fn handle_push_promise (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    let flag = frame.header.2;
    let stream_id = frame.header.3;
    
    // first check if stream is in hashmap
    let exists = match stream_manager.get_stream_status(&stream_id) {
        None => false,
        Some(_status) => true,
    };

    // if state is idle (aka. not in hashmap), reserve it
    if !exists {
        stream_manager.open_idle(receiving, stream_id);
        let mut status = stream_manager.get_stream_status(&stream_id).unwrap();
        status.set_reserved(true); //note: currently does not play a major role in state transitions

        // The frame can only trigger a state transition if the EndHeader flag is set
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

/// Continuation Frames
///
/// Continuation frames can only follow push promise or headers frames.
/// They may carry a end headers flag which could trigger a transition to Reserved or Halfclosed states
/// depending on the type of frame it was first associated with (Push Promise, or Headers)
pub fn handle_continuation (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    let flag = frame.header.2;
    let stream_id = frame.header.3;
    let status = stream_manager.get_stream_status(&stream_id).unwrap();

    // Transition is only triggered by a set Endheaders flag, otherwise another continuation frame should follow
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

/// Data Frames
///
/// Data frames are subject to flow control, 
/// half-closed streams transition to closed if the End Stream flag is set
pub fn handle_data (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    let flag = frame.header.2;
    let stream_id = frame.header.3;

    // Flow Control

    if Flags::EndStream.is_set(flag) {
        stream_manager.close(stream_id);
    } else {
        // process frame?
    };

}

/// Priority Frames
///
/// Updates the priority status fields of a given stream
/// TODO: Integrate with PriorityManager
pub fn handle_priority (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    //actually requires parsing the frame to extract relevant info
    //the streams it depends on
    //exclusive?
}

/// Window Update Frames
///
/// Modifes the flow control for the stream
pub fn handle_window_update (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    //also requires parsing the frame to extract information
}

/// RST_STREAM
///
/// Closes the stream
pub fn handle_rst_stream (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    // first check if stream is in hashmap
    let stream_id = frame.header.3;
    stream_manager.close(stream_id);
}
