// Assumes valid header frame for state
//A HEADERS frame carries the END_STREAM flag that signals the end of a stream.
//However, a HEADERS frame with the END_STREAM flag set can be followed by CONTINUATION frames on the same stream.
//Logically, the CONTINUATION frames are part of the HEADERS frame.
use super::super::frame::{RawFrame};
use super::streammanager::StreamManager;
use super::{StreamStates, Flags};

pub fn handle_header (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) { // parse frame here?
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

    // check for Priority Flag

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
pub fn handle_push_promise (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    let flag = frame.header.2;
    let stream_id = frame.header.3;
    
    // first check if stream is in hashmap
    let exists = match stream_manager.get_stream_status(&stream_id) {
        None => false,
        Some(_status) => true,
    };

    // if state is idle (not in hashmap), reserve it
    if !exists {
        stream_manager.open_idle(stream_id, receiving);
        let mut status = stream_manager.get_stream_status(&stream_id).unwrap();
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
pub fn handle_continuation (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    let flag = frame.header.2;
    let stream_id = frame.header.3;
    let status = stream_manager.get_stream_status(&stream_id).unwrap();

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

//Updates the priority status fields of a given stream
pub fn handle_priority (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    //actually requires parsing the frame to extract relevant info
    //the streams it depends on
    //exclusive?
}

//modifes flow control for the stream
pub fn handle_window_update (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    //also requires parsing the frame to extract information
}

// Closes the stream
pub fn handle_rst_stream (stream_manager: &mut StreamManager, receiving: bool, frame: &RawFrame) {
    // first check if stream is in hashmap
    let stream_id = frame.header.3;
    stream_manager.close(stream_id);
}
