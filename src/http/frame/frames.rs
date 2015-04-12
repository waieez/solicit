use std::mem;
use super::super::StreamId;

/// An alias for the 9-byte buffer that each HTTP/2 frame header must be stored
/// in.
pub type FrameHeaderBuffer = [u8; 9];
/// An alias for the 4-tuple representing the components of each HTTP/2 frame
/// header.
pub type FrameHeader = (u32, u8, u8, u32);

/// Deconstructs a `FrameHeader` into its corresponding 4 components,
/// represented as a 4-tuple: `(length, frame_type, flags, stream_id)`.
///
/// The frame `type` and `flags` components are returned as their original
/// octet representation, rather than reinterpreted.
pub fn unpack_header(header: &FrameHeaderBuffer) -> FrameHeader {
    let length: u32 =
        ((header[0] as u32) << 16) |
        ((header[1] as u32) <<  8) |
        ((header[2] as u32) <<  0);
    let frame_type = header[3];
    let flags = header[4];
    let stream_id: u32 = unpack_octets_4!(header, 5, u32);

    (length, frame_type, flags, stream_id)
}

/// Constructs a buffer of 9 bytes that represents the given `FrameHeader`.
pub fn pack_header(header: &FrameHeader) -> FrameHeaderBuffer {
    let &(length, frame_type, flags, stream_id) = header;

    [
        (((length >> 16) & 0x000000FF) as u8),
        (((length >>  8) & 0x000000FF) as u8),
        (((length >>  0) & 0x000000FF) as u8),
        frame_type,
        flags,
        (((stream_id >> 24) & 0x000000FF) as u8),
        (((stream_id >> 16) & 0x000000FF) as u8),
        (((stream_id >>  8) & 0x000000FF) as u8),
        (((stream_id >>  0) & 0x000000FF) as u8),
    ]
}

/// A helper function that parses the given payload, considering it padded.
///
/// This means that the first byte is the length of the padding with that many
/// 0 bytes expected to follow the actual payload.
///
/// # Returns
///
/// A slice of the given payload where the actual one is found and the length
/// of the padding.
///
/// If the padded payload is invalid (e.g. the length of the padding is equal
/// to the total length), returns `None`.
pub fn parse_padded_payload<'a>(payload: &'a [u8]) -> Option<(&'a [u8], u8)> {
    if payload.len() == 0 {
        // We make sure not to index the payload before we're sure how
        // large the buffer is.
        // If this is the case, the frame is invalid as no padding
        // length can be extracted, even though the frame should be
        // padded.
        return None;
    }
    let pad_len = payload[0] as usize;
    if pad_len >= payload.len() {
        // This is invalid: the padding length MUST be less than the
        // total frame size.
        return None;
    }

    Some((&payload[1..payload.len() - pad_len], pad_len as u8))
}

/// A trait that all HTTP/2 frame header flags need to implement.
pub trait Flag {
    /// Returns a bit mask that represents the flag.
    fn bitmask(&self) -> u8;
}

/// A trait that all HTTP/2 frame structs need to implement.
pub trait Frame {
    /// The type that represents the flags that the particular `Frame` can take.
    /// This makes sure that only valid `Flag`s are used with each `Frame`.
    type FlagType: Flag;

    /// Creates a new `Frame` from the given `RawFrame` (i.e. header and
    /// payload), if possible.
    ///
    /// # Returns
    ///
    /// `None` if a valid `Frame` cannot be constructed from the given
    /// `RawFrame`. Some reasons why this may happen is a wrong frame type in
    /// the header, a body that cannot be decoded according to the particular
    /// frame's rules, etc.
    ///
    /// Otherwise, returns a newly constructed `Frame`.
    fn from_raw(raw_frame: RawFrame) -> Option<Self>;

    /// Tests if the given flag is set for the frame.
    fn is_set(&self, flag: Self::FlagType) -> bool;
    /// Returns the `StreamId` of the stream to which the frame is associated
    fn get_stream_id(&self) -> StreamId;
    /// Returns a `FrameHeader` based on the current state of the `Frame`.
    fn get_header(&self) -> FrameHeader;

    /// Sets the given flag for the frame.
    fn set_flag(&mut self, flag: Self::FlagType);

    /// Returns a `Vec` with the serialized representation of the frame.
    fn serialize(&self) -> Vec<u8>;
}

/// A struct that defines the format of the raw HTTP/2 frame, i.e. the frame
/// as it is read from the wire.
///
/// This format is defined in section 4.1. of the HTTP/2 spec.
///
/// The `RawFrame` struct simply stores the raw components of an HTTP/2 frame:
/// its header and the payload as a sequence of bytes.
///
/// It does not try to interpret the payload bytes, nor do any validation in
/// terms of its validity based on the frame type given in the header.
/// It is simply a wrapper around the two parts of an HTTP/2 frame.
pub struct RawFrame {
    /// The parsed header of the frame.
    pub header: FrameHeader,
    /// The payload of the frame, as the raw byte sequence, as received on
    /// the wire.
    pub payload: Vec<u8>,
}

impl RawFrame {
    /// Creates a new `RawFrame` with the given `FrameHeader`. The payload is
    /// left empty.
    pub fn new(header: FrameHeader) -> RawFrame {
        RawFrame::with_payload(header, Vec::new())
    }

    /// Creates a new `RawFrame` with the given header and payload.
    pub fn with_payload(header: FrameHeader, payload: Vec<u8>) -> RawFrame {
        RawFrame {
            header: header,
            payload: payload,
        }
    }

    /// Creates a new `RawFrame` by parsing the given buffer.
    ///
    /// # Returns
    ///
    /// If the first bytes of the buffer represent a valid frame, a `RawFrame`
    /// that represents it is returned. Obviously, the buffer should contain
    /// both the header and the payload of the frame.
    ///
    /// If the first bytes of the buffer cannot be interpreted as a raw frame,
    /// `None` is returned. This includes the case where the buffer does not
    /// contain enough data to contain the entire payload (whose length was
    /// advertised in the header).
    pub fn from_buf(buf: &[u8]) -> Option<RawFrame> {
        if buf.len() < 9 {
            return None;
        }
        let header = unpack_header(unsafe {
            assert!(buf.len() >= 9);
            // We just asserted that this transmute is safe.
            mem::transmute(buf.as_ptr())
        });
        let payload_len = header.0 as usize;

        if buf[9..].len() < payload_len {
            return None;
        }

        Some(RawFrame {
            header: header,
            payload: buf[9..9 + header.0 as usize].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        unpack_header,
        pack_header,
        RawFrame,
        FrameHeader,
        Frame,
    };
    use super::super::testconfig::*;

    /// Tests that the `unpack_header` function correctly returns the
    /// components of HTTP/2 frame headers.
    #[test]
    fn test_unpack_header() {
        {
            let header = [0; 9];
            assert_eq!((0, 0, 0, 0), unpack_header(&header));
        }
        {
            let header = [0, 0, 1, 2, 3, 0, 0, 0, 4];
            assert_eq!((1, 2, 3, 4), unpack_header(&header));
        }
        {
            let header = [0, 0, 1, 200, 100, 0, 0, 0, 4];
            assert_eq!((1, 200, 100, 4), unpack_header(&header));
        }
        {
            let header = [0, 0, 1, 0, 0, 0, 0, 0, 0];
            assert_eq!((1, 0, 0, 0), unpack_header(&header));
        }
        {
            let header = [0, 1, 0, 0, 0, 0, 0, 0, 0];
            assert_eq!((256, 0, 0, 0), unpack_header(&header));
        }
        {
            let header = [1, 0, 0, 0, 0, 0, 0, 0, 0];
            assert_eq!((256 * 256, 0, 0, 0), unpack_header(&header));
        }
        {
            let header = [0, 0, 0, 0, 0, 0, 0, 0, 1];
            assert_eq!((0, 0, 0, 1), unpack_header(&header));
        }
        {
            let header = [0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 1];
            assert_eq!(((1 << 24) - 1, 0, 0, 1), unpack_header(&header));
        }
        {
            let header = [0xFF, 0xFF, 0xFF, 0, 0, 1, 1, 1, 1];
            assert_eq!(
                ((1 << 24) - 1, 0, 0, 1 + (1 << 8) + (1 << 16) + (1 << 24)),
                unpack_header(&header));
        }
    }

    /// Tests that the `pack_header` function correctly returns the buffer
    /// corresponding to components of HTTP/2 frame headers.
    #[test]
    fn test_pack_header() {
        {
            let header = [0; 9];
            assert_eq!(pack_header(&(0, 0, 0, 0)), header);
        }
        {
            let header = [0, 0, 1, 2, 3, 0, 0, 0, 4];
            assert_eq!(pack_header(&(1, 2, 3, 4)), header);
        }
        {
            let header = [0, 0, 1, 200, 100, 0, 0, 0, 4];
            assert_eq!(pack_header(&(1, 200, 100, 4)), header);
        }
        {
            let header = [0, 0, 1, 0, 0, 0, 0, 0, 0];
            assert_eq!(pack_header(&(1, 0, 0, 0)), header);
        }
        {
            let header = [0, 1, 0, 0, 0, 0, 0, 0, 0];
            assert_eq!(pack_header(&(256, 0, 0, 0)), header);
        }
        {
            let header = [1, 0, 0, 0, 0, 0, 0, 0, 0];
            assert_eq!(pack_header(&(256 * 256, 0, 0, 0)), header);
        }
        {
            let header = [0, 0, 0, 0, 0, 0, 0, 0, 1];
            assert_eq!(pack_header(&(0, 0, 0, 1)), header);
        }
        {
            let header = [0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 1];
            assert_eq!(pack_header(&((1 << 24) - 1, 0, 0, 1)), header);
        }
        {
            let header = [0xFF, 0xFF, 0xFF, 0, 0, 1, 1, 1, 1];
            let header_components = (
                (1 << 24) - 1, 0, 0, 1 + (1 << 8) + (1 << 16) + (1 << 24)
            );
            assert_eq!(pack_header(&header_components), header);
        }
    }

    /// Tests that the `RawFrame::from_buf` method correctly constructs a
    /// `RawFrame` from a given buffer.
    #[test]
    fn test_raw_frame_from_buffer() {
        // Correct frame
        {
            let data = b"123";
            let header = (data.len() as u32, 0x1, 0, 1);
            let buf = {
                let mut buf = Vec::new();
                buf.extend(pack_header(&header).to_vec().into_iter());
                buf.extend(data.to_vec().into_iter());
                buf
            };

            let raw = RawFrame::from_buf(&buf).unwrap();

            assert_eq!(raw.header, header);
            assert_eq!(raw.payload, data)
        }
        // Correct frame with trailing data
        {
            let data = b"123";
            let header = (data.len() as u32, 0x1, 0, 1);
            let buf = {
                let mut buf = Vec::new();
                buf.extend(pack_header(&header).to_vec().into_iter());
                buf.extend(data.to_vec().into_iter());
                buf.extend(vec![1, 2, 3, 4, 5].into_iter());
                buf
            };

            let raw = RawFrame::from_buf(&buf).unwrap();

            assert_eq!(raw.header, header);
            assert_eq!(raw.payload, data)
        }
        // Missing payload chunk
        {
            let data = b"123";
            let header = (data.len() as u32, 0x1, 0, 1);
            let buf = {
                let mut buf = Vec::new();
                buf.extend(pack_header(&header).to_vec().into_iter());
                buf.extend(data[..2].to_vec().into_iter());
                buf
            };

            assert!(RawFrame::from_buf(&buf).is_none());
        }
        // Missing header chunk
        {
            let header = (0, 0x1, 0, 1);
            let buf = {
                let mut buf = Vec::new();
                buf.extend(pack_header(&header)[..5].to_vec().into_iter());
                buf
            };

            assert!(RawFrame::from_buf(&buf).is_none());
        }
        // Completely empty buffer
        {
            assert!(RawFrame::from_buf(&[]).is_none());
        }
    }
}
