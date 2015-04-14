use super::{Frame, RawFrame, FrameHeader};

/// Builds a test frame of the given type with the given header and
/// payload, by using the `Frame::from_raw` method.
pub fn build_test_frame<F: Frame>(header: &FrameHeader, payload: &[u8]) -> F {
    let raw = RawFrame::with_payload(header.clone(), payload.to_vec());
    Frame::from_raw(raw).unwrap()
}

/// Builds a `Vec` containing the given data as a padded HTTP/2 frame.
///
/// It first places the length of the padding, followed by the data,
/// followed by `pad_len` zero bytes.
pub fn build_padded_frame_payload(data: &[u8], pad_len: u8) -> Vec<u8> {
    let sz = 1 + data.len() + pad_len as usize;
    let mut payload: Vec<u8> = Vec::with_capacity(sz);
    payload.push(pad_len);
    payload.extend(data.to_vec().into_iter());
    for _ in 0..pad_len { payload.push(0); }

    payload
}
