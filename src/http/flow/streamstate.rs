use super::super::StreamId;
use super::super::frame::*;

pub enum StreamStates {
  Idle,
  Open,
  Closed,
  ReservedLocal,
  ReservedRemote,
  HalfClosedLocal,
  HalfClosedRemote
}

pub fn check_state (state: StreamStates) {
//pub fn check_state<F: Frame>(frame: F) {
  match state {
    _ => println!("totally works dawg")
  }
}

// pub fn handle_frame<F: Frame>(frame: F) {

// }

#[cfg(test)]
mod tests {
  use super::{
    StreamStates,
    check_state
  };

  fn it_should_pass() {
    check_state(StreamStates::Closed);
    assert_eq!(true, true);
  }
}