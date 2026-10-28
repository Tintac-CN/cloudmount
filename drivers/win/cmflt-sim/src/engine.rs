//! Deterministic user-mode engine simulator.
//!
//! It consumes the kernel→engine part of `cmflt-core::protocol`
//! (`HydrateRequest`, `Dehydrate`, `PinSet`, `Invalidate`) and produces the
//! `HydrateReply` the real engine would send. The simulator does not touch the
//! filesystem; it models which ranges are available and returns the byte count
//! the engine would have written.

use std::collections::BTreeMap;

use cmflt_core::protocol::{
    Dehydrate, Header, HydrateReply, HydrateRequest, Invalidate, MessageKind, PinSet,
    ProtocolError, STATUS_SUCCESS,
};

pub const STATUS_OBJECT_NAME_NOT_FOUND: i32 = 0xC000_0034u32 as i32;
pub const STATUS_INVALID_PARAMETER: i32 = 0xC000_000Du32 as i32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineError {
    Protocol(ProtocolError),
    Unsupported,
    OutputTooSmall,
}

impl From<ProtocolError> for EngineError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

#[derive(Debug, Default)]
pub struct MockEngine {
    files: BTreeMap<u64, Vec<u8>>,
    pinned: BTreeMap<u64, bool>,
}

impl MockEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, file_id: u64, data: Vec<u8>) {
        self.files.insert(file_id, data);
    }

    pub fn remove_file(&mut self, file_id: u64) {
        self.files.remove(&file_id);
    }

    pub fn file_len(&self, file_id: u64) -> Option<usize> {
        self.files.get(&file_id).map(Vec::len)
    }

    pub fn is_pinned(&self, file_id: u64) -> bool {
        self.pinned.get(&file_id).copied().unwrap_or(false)
    }

    /// Handle one kernel→engine message. Returns the number of reply bytes.
    pub fn handle(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, EngineError> {
        let header = Header::decode(input)?;
        match header.kind {
            MessageKind::HydrateRequest => {
                let request = HydrateRequest::decode(input)?;
                let (status, bytes_written) = match self.files.get(&request.file_id) {
                    None => (STATUS_OBJECT_NAME_NOT_FOUND, 0),
                    Some(data) => {
                        let start = request.offset as usize;
                        let length = request.length as usize;
                        match start.checked_add(length) {
                            Some(end) if end <= data.len() => (STATUS_SUCCESS, request.length),
                            _ => (STATUS_INVALID_PARAMETER, 0),
                        }
                    }
                };
                let reply = HydrateReply {
                    tx_id: request.tx_id,
                    status,
                    bytes_written,
                    sha1: [0u8; 20],
                    flags: 0,
                };
                reply.encode(output).map_err(EngineError::from)
            }
            MessageKind::Dehydrate => {
                let request = Dehydrate::decode(input)?;
                if let Some(data) = self.files.get_mut(&request.file_id) {
                    data.clear();
                }
                Ok(0)
            }
            MessageKind::PinSet => {
                let request = PinSet::decode(input)?;
                self.pinned.insert(request.file_id, request.pin != 0);
                Ok(0)
            }
            MessageKind::Invalidate => {
                let request = Invalidate::decode(input)?;
                self.files.remove(&request.file_id);
                self.pinned.remove(&request.file_id);
                Ok(0)
            }
            _ => Err(EngineError::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydrate_request_writes_a_reply() {
        let mut engine = MockEngine::new();
        engine.add_file(7, vec![0xAB; 4096]);
        let request = HydrateRequest {
            tx_id: 11,
            file_id: 7,
            offset: 0,
            length: 1024,
            requestor_pid: 123,
            deadline_ms: 15_000,
            flags: 0,
        };
        let mut input = [0u8; 128];
        let input_len = request.encode(&mut input).expect("request");
        let mut output = [0u8; 128];
        let output_len = engine
            .handle(&input[..input_len], &mut output)
            .expect("reply");
        let reply = HydrateReply::decode(&output[..output_len]).expect("decode");
        assert_eq!(reply.tx_id, 11);
        assert_eq!(reply.status, STATUS_SUCCESS);
        assert_eq!(reply.bytes_written, 1024);
    }

    #[test]
    fn missing_file_and_bad_range_are_errors() {
        let mut engine = MockEngine::new();
        engine.add_file(7, vec![0u8; 16]);
        let mut input = [0u8; 128];
        let mut output = [0u8; 128];

        let missing = HydrateRequest {
            tx_id: 1,
            file_id: 8,
            offset: 0,
            length: 1,
            requestor_pid: 1,
            deadline_ms: 1,
            flags: 0,
        };
        let len = missing.encode(&mut input).expect("encode");
        let out_len = engine.handle(&input[..len], &mut output).expect("reply");
        let reply = HydrateReply::decode(&output[..out_len]).expect("decode");
        assert_eq!(reply.status, STATUS_OBJECT_NAME_NOT_FOUND);

        let bad_range = HydrateRequest {
            tx_id: 2,
            file_id: 7,
            offset: 8,
            length: 16,
            requestor_pid: 1,
            deadline_ms: 1,
            flags: 0,
        };
        let len = bad_range.encode(&mut input).expect("encode");
        let out_len = engine.handle(&input[..len], &mut output).expect("reply");
        let reply = HydrateReply::decode(&output[..out_len]).expect("decode");
        assert_eq!(reply.status, STATUS_INVALID_PARAMETER);
    }

    #[test]
    fn dehydrate_pin_and_invalidate_update_state() {
        let mut engine = MockEngine::new();
        engine.add_file(7, vec![1u8; 8]);
        let mut input = [0u8; 128];

        let pin = PinSet {
            tx_id: 1,
            file_id: 7,
            pin: 1,
        };
        let len = pin.encode(&mut input).expect("pin");
        assert_eq!(engine.handle(&input[..len], &mut []), Ok(0));
        assert!(engine.is_pinned(7));

        let dehydrate = Dehydrate {
            tx_id: 2,
            file_id: 7,
            flags: 0,
        };
        let len = dehydrate.encode(&mut input).expect("dehydrate");
        assert_eq!(engine.handle(&input[..len], &mut []), Ok(0));
        assert_eq!(engine.file_len(7), Some(0));

        engine.add_file(8, vec![1u8; 8]);
        let invalidate = Invalidate {
            file_id: 8,
            flags: 0,
        };
        let len = invalidate.encode(&mut input).expect("invalidate");
        assert_eq!(engine.handle(&input[..len], &mut []), Ok(0));
        assert_eq!(engine.file_len(8), None);
    }

    #[test]
    fn non_kernel_message_is_rejected() {
        let mut engine = MockEngine::new();
        let hello = cmflt_core::protocol::Hello {
            proto_version: cmflt_core::protocol::PROTOCOL_VERSION,
            capabilities: 0,
            engine_version: [0u64; 2],
        };
        let mut input = [0u8; 128];
        let len = hello.encode(&mut input).expect("hello");
        assert_eq!(
            engine.handle(&input[..len], &mut [0u8; 128]),
            Err(EngineError::Unsupported)
        );
    }
}
