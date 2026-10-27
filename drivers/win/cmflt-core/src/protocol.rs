//! Wire protocol shared by the kernel driver and the user-mode engine.
//!
//! The encoding is explicit little-endian, fixed-size and allocation-free.
//! Nothing is `#[repr(C)]` on the wire: the kernel and engine may be compiled
//! with different Rust versions, so bytes are the contract and layout
//! assertions are checked against the constants below.
//!
//! Hardening rules:
//! - magic, version, kind, flags and payload length are validated before any
//!   field is read;
//! - every read is bounds-checked and returns an error instead of panicking;
//! - a hydrate request must carry a non-zero transaction id, a non-zero
//!   requestor pid, a non-zero deadline and a non-empty range;
//! - `MAX_PAYLOAD` caps the amount of memory a peer can make us inspect.

pub const PROTOCOL_MAGIC: u32 = 0x4C46_4D43; // bytes "CMFL", little-endian
pub const PROTOCOL_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 24;
pub const MAX_PAYLOAD: usize = 64 * 1024;
pub const STATUS_SUCCESS: i32 = 0;

/// Header kind values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageKind {
    Hello = 1,
    Health = 2,
    HydrateRequest = 3,
    HydrateReply = 4,
}

impl MessageKind {
    const fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Hello),
            2 => Some(Self::Health),
            3 => Some(Self::HydrateRequest),
            4 => Some(Self::HydrateReply),
            _ => None,
        }
    }
}

/// Fixed 24-byte frame header.
///
/// Wire layout:
/// `0..4 magic | 4..6 version | 6..8 kind | 8..16 tx_id | 16..20 payload_len | 20..24 flags`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Header {
    pub kind: MessageKind,
    pub tx_id: u64,
    pub payload_len: u32,
    pub flags: u32,
}

impl Header {
    pub fn encode(self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        if buf.len() < HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall);
        }
        write_u32(buf, 0, PROTOCOL_MAGIC)?;
        write_u16(buf, 4, PROTOCOL_VERSION)?;
        write_u16(buf, 6, self.kind as u16)?;
        write_u64(buf, 8, self.tx_id)?;
        write_u32(buf, 16, self.payload_len)?;
        write_u32(buf, 20, self.flags)?;
        Ok(HEADER_LEN)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.len() < HEADER_LEN {
            return Err(ProtocolError::Truncated);
        }
        if read_u32(buf, 0)? != PROTOCOL_MAGIC {
            return Err(ProtocolError::BadMagic);
        }
        let version = read_u16(buf, 4)?;
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        let kind = MessageKind::from_u16(read_u16(buf, 6)?).ok_or(ProtocolError::UnknownKind)?;
        let tx_id = read_u64(buf, 8)?;
        let payload_len = read_u32(buf, 16)?;
        if payload_len as usize > MAX_PAYLOAD {
            return Err(ProtocolError::BadLength);
        }
        let flags = read_u32(buf, 20)?;
        if flags != 0 {
            return Err(ProtocolError::BadFlags);
        }
        Ok(Self {
            kind,
            tx_id,
            payload_len,
            flags,
        })
    }

    /// Total frame length including the header.
    pub fn total_len(self) -> Result<usize, ProtocolError> {
        HEADER_LEN
            .checked_add(self.payload_len as usize)
            .filter(|len| *len <= HEADER_LEN + MAX_PAYLOAD)
            .ok_or(ProtocolError::BadLength)
    }
}

/// Errors are deliberately small and allocation-free so the kernel can log
/// them as a code without formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    Truncated,
    BufferTooSmall,
    BadMagic,
    UnsupportedVersion,
    UnknownKind,
    WrongKind,
    BadLength,
    BadFlags,
    InvalidField,
    Overflow,
}

/// Engine handshake. `tx_id` is always zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hello {
    pub proto_version: u16,
    pub capabilities: u32,
    pub engine_version: [u8; 16],
}

pub const HELLO_PAYLOAD_LEN: usize = 24;

impl Hello {
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        let total = HEADER_LEN + HELLO_PAYLOAD_LEN;
        if buf.len() < total {
            return Err(ProtocolError::BufferTooSmall);
        }
        Header {
            kind: MessageKind::Hello,
            tx_id: 0,
            payload_len: HELLO_PAYLOAD_LEN as u32,
            flags: 0,
        }
        .encode(buf)?;
        write_u16(buf, HEADER_LEN, self.proto_version)?;
        write_u32(buf, HEADER_LEN + 2, self.capabilities)?;
        write_bytes(buf, HEADER_LEN + 6, &self.engine_version)?;
        write_u16(buf, HEADER_LEN + 22, 0)?;
        Ok(total)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        let header = Header::decode(buf)?;
        if header.kind != MessageKind::Hello {
            return Err(ProtocolError::WrongKind);
        }
        if header.tx_id != 0 {
            return Err(ProtocolError::InvalidField);
        }
        if header.payload_len as usize != HELLO_PAYLOAD_LEN {
            return Err(ProtocolError::BadLength);
        }
        if buf.len() < header.total_len()? {
            return Err(ProtocolError::Truncated);
        }
        let proto_version = read_u16(buf, HEADER_LEN)?;
        if proto_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        let capabilities = read_u32(buf, HEADER_LEN + 2)?;
        let engine_version = read_array::<16>(buf, HEADER_LEN + 6)?;
        if read_u16(buf, HEADER_LEN + 22)? != 0 {
            return Err(ProtocolError::BadFlags);
        }
        Ok(Self {
            proto_version,
            capabilities,
            engine_version,
        })
    }
}

/// Kernel-to-engine hydration request. Data never travels in this message;
/// the engine writes the file stream directly and reports the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HydrateRequest {
    pub tx_id: u64,
    pub file_id: u64,
    pub offset: u64,
    pub length: u32,
    pub requestor_pid: u32,
    pub deadline_ms: u32,
    pub flags: u32,
}

pub const HYDRATE_REQUEST_PAYLOAD_LEN: usize = 32;

impl HydrateRequest {
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        let total = HEADER_LEN + HYDRATE_REQUEST_PAYLOAD_LEN;
        if buf.len() < total {
            return Err(ProtocolError::BufferTooSmall);
        }
        Header {
            kind: MessageKind::HydrateRequest,
            tx_id: self.tx_id,
            payload_len: HYDRATE_REQUEST_PAYLOAD_LEN as u32,
            flags: 0,
        }
        .encode(buf)?;
        write_u64(buf, HEADER_LEN, self.file_id)?;
        write_u64(buf, HEADER_LEN + 8, self.offset)?;
        write_u32(buf, HEADER_LEN + 16, self.length)?;
        write_u32(buf, HEADER_LEN + 20, self.requestor_pid)?;
        write_u32(buf, HEADER_LEN + 24, self.deadline_ms)?;
        write_u32(buf, HEADER_LEN + 28, self.flags)?;
        Ok(total)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        let header = Header::decode(buf)?;
        if header.kind != MessageKind::HydrateRequest {
            return Err(ProtocolError::WrongKind);
        }
        if header.tx_id == 0 {
            return Err(ProtocolError::InvalidField);
        }
        if header.payload_len as usize != HYDRATE_REQUEST_PAYLOAD_LEN {
            return Err(ProtocolError::BadLength);
        }
        if buf.len() < header.total_len()? {
            return Err(ProtocolError::Truncated);
        }
        let file_id = read_u64(buf, HEADER_LEN)?;
        let offset = read_u64(buf, HEADER_LEN + 8)?;
        let length = read_u32(buf, HEADER_LEN + 16)?;
        let requestor_pid = read_u32(buf, HEADER_LEN + 20)?;
        let deadline_ms = read_u32(buf, HEADER_LEN + 24)?;
        let flags = read_u32(buf, HEADER_LEN + 28)?;
        if length == 0 || requestor_pid == 0 || deadline_ms == 0 {
            return Err(ProtocolError::InvalidField);
        }
        if offset.checked_add(length as u64).is_none() {
            return Err(ProtocolError::Overflow);
        }
        if flags != 0 {
            return Err(ProtocolError::BadFlags);
        }
        Ok(Self {
            tx_id: header.tx_id,
            file_id,
            offset,
            length,
            requestor_pid,
            deadline_ms,
            flags,
        })
    }
}

/// Engine-to-kernel hydration result. `sha1` is optional evidence; all-zero
/// means "not provided".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HydrateReply {
    pub tx_id: u64,
    pub status: i32,
    pub bytes_written: u32,
    pub sha1: [u8; 20],
    pub flags: u32,
}

pub const HYDRATE_REPLY_PAYLOAD_LEN: usize = 32;

impl HydrateReply {
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        let total = HEADER_LEN + HYDRATE_REPLY_PAYLOAD_LEN;
        if buf.len() < total {
            return Err(ProtocolError::BufferTooSmall);
        }
        Header {
            kind: MessageKind::HydrateReply,
            tx_id: self.tx_id,
            payload_len: HYDRATE_REPLY_PAYLOAD_LEN as u32,
            flags: 0,
        }
        .encode(buf)?;
        write_i32(buf, HEADER_LEN, self.status)?;
        write_u32(buf, HEADER_LEN + 4, self.bytes_written)?;
        write_bytes(buf, HEADER_LEN + 8, &self.sha1)?;
        write_u32(buf, HEADER_LEN + 28, self.flags)?;
        Ok(total)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        let header = Header::decode(buf)?;
        if header.kind != MessageKind::HydrateReply {
            return Err(ProtocolError::WrongKind);
        }
        if header.tx_id == 0 {
            return Err(ProtocolError::InvalidField);
        }
        if header.payload_len as usize != HYDRATE_REPLY_PAYLOAD_LEN {
            return Err(ProtocolError::BadLength);
        }
        if buf.len() < header.total_len()? {
            return Err(ProtocolError::Truncated);
        }
        let status = read_i32(buf, HEADER_LEN)?;
        let bytes_written = read_u32(buf, HEADER_LEN + 4)?;
        let sha1 = read_array::<20>(buf, HEADER_LEN + 8)?;
        let flags = read_u32(buf, HEADER_LEN + 28)?;
        if status == STATUS_SUCCESS && bytes_written == 0 {
            return Err(ProtocolError::InvalidField);
        }
        if flags != 0 {
            return Err(ProtocolError::BadFlags);
        }
        Ok(Self {
            tx_id: header.tx_id,
            status,
            bytes_written,
            sha1,
            flags,
        })
    }
}

/// Engine health/heartbeat. `tx_id` is always zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    pub uptime_ms: u64,
    pub inflight: u32,
    pub completed: u32,
    pub failed: u32,
    pub breaker_trips: u32,
}

pub const HEALTH_PAYLOAD_LEN: usize = 24;

impl Health {
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, ProtocolError> {
        let total = HEADER_LEN + HEALTH_PAYLOAD_LEN;
        if buf.len() < total {
            return Err(ProtocolError::BufferTooSmall);
        }
        Header {
            kind: MessageKind::Health,
            tx_id: 0,
            payload_len: HEALTH_PAYLOAD_LEN as u32,
            flags: 0,
        }
        .encode(buf)?;
        write_u64(buf, HEADER_LEN, self.uptime_ms)?;
        write_u32(buf, HEADER_LEN + 8, self.inflight)?;
        write_u32(buf, HEADER_LEN + 12, self.completed)?;
        write_u32(buf, HEADER_LEN + 16, self.failed)?;
        write_u32(buf, HEADER_LEN + 20, self.breaker_trips)?;
        Ok(total)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        let header = Header::decode(buf)?;
        if header.kind != MessageKind::Health {
            return Err(ProtocolError::WrongKind);
        }
        if header.tx_id != 0 {
            return Err(ProtocolError::InvalidField);
        }
        if header.payload_len as usize != HEALTH_PAYLOAD_LEN {
            return Err(ProtocolError::BadLength);
        }
        if buf.len() < header.total_len()? {
            return Err(ProtocolError::Truncated);
        }
        Ok(Self {
            uptime_ms: read_u64(buf, HEADER_LEN)?,
            inflight: read_u32(buf, HEADER_LEN + 8)?,
            completed: read_u32(buf, HEADER_LEN + 12)?,
            failed: read_u32(buf, HEADER_LEN + 16)?,
            breaker_trips: read_u32(buf, HEADER_LEN + 20)?,
        })
    }
}

fn checked_end(offset: usize, length: usize) -> Result<usize, ProtocolError> {
    offset.checked_add(length).ok_or(ProtocolError::Overflow)
}

fn write_bytes(buf: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), ProtocolError> {
    let end = checked_end(offset, bytes.len())?;
    let slot = buf
        .get_mut(offset..end)
        .ok_or(ProtocolError::BufferTooSmall)?;
    slot.copy_from_slice(bytes);
    Ok(())
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) -> Result<(), ProtocolError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) -> Result<(), ProtocolError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_i32(buf: &mut [u8], offset: usize, value: i32) -> Result<(), ProtocolError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) -> Result<(), ProtocolError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn read_array<const N: usize>(buf: &[u8], offset: usize) -> Result<[u8; N], ProtocolError> {
    let end = checked_end(offset, N)?;
    let slice = buf.get(offset..end).ok_or(ProtocolError::Truncated)?;
    slice.try_into().map_err(|_| ProtocolError::Truncated)
}

fn read_u16(buf: &[u8], offset: usize) -> Result<u16, ProtocolError> {
    Ok(u16::from_le_bytes(read_array::<2>(buf, offset)?))
}

fn read_u32(buf: &[u8], offset: usize) -> Result<u32, ProtocolError> {
    Ok(u32::from_le_bytes(read_array::<4>(buf, offset)?))
}

fn read_i32(buf: &[u8], offset: usize) -> Result<i32, ProtocolError> {
    Ok(i32::from_le_bytes(read_array::<4>(buf, offset)?))
}

fn read_u64(buf: &[u8], offset: usize) -> Result<u64, ProtocolError> {
    Ok(u64::from_le_bytes(read_array::<8>(buf, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn header_layout_is_stable() {
        assert_eq!(HEADER_LEN, 24);
        assert_eq!(size_of::<Header>(), HEADER_LEN);
    }

    fn hello() -> Hello {
        Hello {
            proto_version: PROTOCOL_VERSION,
            capabilities: 0x0000_0007,
            engine_version: *b"cloudmount-0.1.0",
        }
    }

    fn request() -> HydrateRequest {
        HydrateRequest {
            tx_id: 42,
            file_id: 0x1122_3344_5566_7788,
            offset: 4096,
            length: 8192,
            requestor_pid: 1234,
            deadline_ms: 15_000,
            flags: 0,
        }
    }

    fn reply() -> HydrateReply {
        HydrateReply {
            tx_id: 42,
            status: STATUS_SUCCESS,
            bytes_written: 8192,
            sha1: [0xAB; 20],
            flags: 0,
        }
    }

    fn health() -> Health {
        Health {
            uptime_ms: 9_000_000,
            inflight: 2,
            completed: 100,
            failed: 1,
            breaker_trips: 0,
        }
    }

    #[test]
    fn roundtrips_each_message() {
        let mut buf = [0u8; 128];

        let len = hello().encode(&mut buf).expect("encode hello");
        assert_eq!(Hello::decode(&buf[..len]).expect("decode hello"), hello());

        let len = request().encode(&mut buf).expect("encode request");
        assert_eq!(
            HydrateRequest::decode(&buf[..len]).expect("decode request"),
            request()
        );

        let len = reply().encode(&mut buf).expect("encode reply");
        assert_eq!(
            HydrateReply::decode(&buf[..len]).expect("decode reply"),
            reply()
        );

        let len = health().encode(&mut buf).expect("encode health");
        assert_eq!(
            Health::decode(&buf[..len]).expect("decode health"),
            health()
        );
    }

    #[test]
    fn truncation_returns_error_and_never_panics() {
        let mut buf = [0u8; 128];
        let len = request().encode(&mut buf).expect("encode request");
        for shorter in 0..len {
            assert!(HydrateRequest::decode(&buf[..shorter]).is_err());
        }
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut buf = [0u8; 64];
        let len = hello().encode(&mut buf).expect("encode");
        if let Some(byte) = buf.get_mut(0) {
            *byte ^= 0xFF;
        }
        assert_eq!(Hello::decode(&buf[..len]), Err(ProtocolError::BadMagic));
    }

    #[test]
    fn bad_version_is_rejected() {
        let mut buf = [0u8; 64];
        let len = hello().encode(&mut buf).expect("encode");
        buf.get_mut(4..6)
            .expect("version field")
            .copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            Hello::decode(&buf[..len]),
            Err(ProtocolError::UnsupportedVersion)
        );
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let mut buf = [0u8; 64];
        let len = hello().encode(&mut buf).expect("encode");
        buf.get_mut(6..8)
            .expect("kind field")
            .copy_from_slice(&999u16.to_le_bytes());
        assert_eq!(Hello::decode(&buf[..len]), Err(ProtocolError::UnknownKind));
    }

    #[test]
    fn oversized_payload_is_rejected() {
        let mut buf = [0u8; HEADER_LEN];
        Header {
            kind: MessageKind::Hello,
            tx_id: 0,
            payload_len: (MAX_PAYLOAD + 1) as u32,
            flags: 0,
        }
        .encode(&mut buf)
        .expect("header encode");
        assert_eq!(Header::decode(&buf), Err(ProtocolError::BadLength));
    }

    #[test]
    fn nonzero_flags_are_rejected() {
        let mut buf = [0u8; HEADER_LEN];
        Header {
            kind: MessageKind::Health,
            tx_id: 0,
            payload_len: HEALTH_PAYLOAD_LEN as u32,
            flags: 1,
        }
        .encode(&mut buf)
        .expect("header encode");
        assert_eq!(Header::decode(&buf), Err(ProtocolError::BadFlags));
    }

    #[test]
    fn hydrate_request_rejects_unsafe_fields() {
        let mut buf = [0u8; 128];

        let mut zero_tx = request();
        zero_tx.tx_id = 0;
        let len = zero_tx.encode(&mut buf).expect("encode");
        assert_eq!(
            HydrateRequest::decode(&buf[..len]),
            Err(ProtocolError::InvalidField)
        );

        let mut zero_length = request();
        zero_length.length = 0;
        let len = zero_length.encode(&mut buf).expect("encode");
        assert_eq!(
            HydrateRequest::decode(&buf[..len]),
            Err(ProtocolError::InvalidField)
        );

        let mut zero_pid = request();
        zero_pid.requestor_pid = 0;
        let len = zero_pid.encode(&mut buf).expect("encode");
        assert_eq!(
            HydrateRequest::decode(&buf[..len]),
            Err(ProtocolError::InvalidField)
        );

        let mut zero_deadline = request();
        zero_deadline.deadline_ms = 0;
        let len = zero_deadline.encode(&mut buf).expect("encode");
        assert_eq!(
            HydrateRequest::decode(&buf[..len]),
            Err(ProtocolError::InvalidField)
        );

        let mut overflow = request();
        overflow.offset = u64::MAX;
        overflow.length = 2;
        let len = overflow.encode(&mut buf).expect("encode");
        assert_eq!(
            HydrateRequest::decode(&buf[..len]),
            Err(ProtocolError::Overflow)
        );
    }

    #[test]
    fn success_reply_requires_bytes_written() {
        let mut buf = [0u8; 128];
        let mut empty = reply();
        empty.bytes_written = 0;
        let len = empty.encode(&mut buf).expect("encode");
        assert_eq!(
            HydrateReply::decode(&buf[..len]),
            Err(ProtocolError::InvalidField)
        );
    }

    #[test]
    fn random_buffers_never_panic_the_decoders() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut buf = [0u8; 256];
        for _ in 0..5000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let len = (state as usize) % (buf.len() + 1);
            for byte in buf.iter_mut().take(len) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            let slice = buf.get(..len).unwrap_or(&[]);
            let _ = Header::decode(slice);
            let _ = Hello::decode(slice);
            let _ = Health::decode(slice);
            let _ = HydrateRequest::decode(slice);
            let _ = HydrateReply::decode(slice);
        }
    }
}
