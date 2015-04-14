use super::super::StreamId;
use super::frames::{
    Frame,
    Flag,
    parse_padded_payload,
    pack_header,
    RawFrame,
    FrameHeader
};

/// An enum representing the flags that a `ContinuationFrame` can have.
/// The integer representation associated to each variant is that flag's
/// bitmask.
///
/// HTTP/2 spec, section 6.10.
#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Debug)]
#[derive(Copy)]
pub enum ContinuationFlag {
    EndHeaders = 0x4,
}

impl Flag for ContinuationFlag {
    #[inline]
    fn bitmask(&self) -> u8 {
        *self as u8
    }
}

/// A struct representing the CONTINUATION frame for HTTP/2, as defined in the
/// HTTP/2 spec, section 6.10.
pub struct ContinuationFrame {
    /// The header block fragment bytes stored within the frame.
    pub header_fragment: Vec<u8>,
    /// The ID of the stream with which this frame is associated
    pub stream_id: StreamId,
    /// The set of flags for the frame, packed into a single byte.
    flags: u8,
}

impl ContinuationFrame {
    /// Creates a new `ContinuationFrame` with the given header fragment and stream
    /// ID. No flags are set.
    pub fn new(fragment: Vec<u8>, stream_id: StreamId) -> ContinuationFrame {
        ContinuationFrame {
            header_fragment: fragment,
            stream_id: stream_id,
            flags: 0,
        }
    }

    /// Returns whether this frame ends the headers. If not, there MUST be a
    /// number of follow up CONTINUATION frames that send the rest of the
    /// header data.
    pub fn is_headers_end(&self) -> bool {
        self.is_set(ContinuationFlag::EndHeaders)
    }

    /// Returns the length of the payload of the current frame, including any
    /// possible padding in the number of bytes.
    fn payload_len(&self) -> u32 {
        self.header_fragment.len() as u32
    }
}

impl Frame for ContinuationFrame {
    /// The type that represents the flags that the particular `Frame` can take.
    /// This makes sure that only valid `Flag`s are used with each `Frame`.
    type FlagType = ContinuationFlag;

    /// Creates a new `ContinuationFrame` with the given `RawFrame` (i.e. header and
    /// payload), if possible.
    ///
    /// # Returns
    ///
    /// `None` if a valid `ContinuationFrame` cannot be constructed from the given
    /// `RawFrame`. The stream ID *must not* be 0.
    ///
    /// Otherwise, returns a newly constructed `ContinuationFrame`.
    fn from_raw(raw_frame: RawFrame) -> Option<ContinuationFrame> {
        // Unpack the header
        let (len, frame_type, flags, stream_id) = raw_frame.header;
        // Check that the frame type is correct for this frame implementation
        if frame_type != 0x9 {
            return None;
        }
        // Check that the length given in the header matches the payload
        // length; if not, something went wrong and we do not consider this a
        // valid frame.
        if (len as usize) != raw_frame.payload.len() {
            return None;
        }
        // Check that the CONTINUATION frame is not associated to stream 0
        if stream_id == 0 {
            return None;
        }

        let payload = &raw_frame.payload[..];

        Some(ContinuationFrame {
            header_fragment: &raw_frame.payload[..],
            stream_id: stream_id,
            flags: flags
        })
    }

    /// Tests if the given flag is set for the frame.
    fn is_set(&self, flag: ContinuationFlag) -> bool {
        (self.flags & flag.bitmask()) != 0
    }

    // Returns the `StreamId` of the stream to which the frame is associated.
    ///
    /// A `SettingsFrame` always has to be associated to stream `0`.
    fn get_stream_id(&self) -> StreamId {
        self.stream_id
    }

    /// Returns a `FrameHeader` based on the current state of the `Frame`.
    fn get_header(&self) -> FrameHeader {
        (self.payload_len(), 0x9, self.flags, self.stream_id)
    }

    /// Sets the given flag for the frame.
    fn set_flag(&mut self, flag: ContinuationFlag) {
        self.flags |= flag.bitmask();
    }

    /// Returns a `Vec` with the serialized representation of the frame.
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.payload_len() as usize);
        // First the header...
        buf.extend(pack_header(&self.get_header()).to_vec().into_iter());
        // Now the actual headers fragment
        buf.extend(self.header_fragment.clone().into_iter());

        buf
    }
}
