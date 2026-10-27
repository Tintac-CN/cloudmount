//! Kernel-side connection and handshake state for the Filter Manager port.
//!
//! The driver owns one [`EngineState`] behind a spinlock. The Filter Manager
//! message callback feeds input bytes into [`EngineState::handle`]; the same
//! code runs in host tests without FltMgr.
//!
//! Protocol rules enforced here:
//! - a message is only accepted after `on_connect`;
//! - `Health` is only accepted after a successful `Hello`;
//! - `Hello` must use the current protocol version;
//! - the `HelloAck` is encoded directly into the caller's output buffer before
//!   any state is changed, so an undersized buffer cannot leave a
//!   half-handshaken connection.
//!
//! Version and health fields are atomics. Besides making the state safe to
//! inspect from callbacks, atomic scalar stores stop LLVM from coalescing the
//! reset/update sequence into 16-byte SSE stores. The driver image is checked
//! for SSE/FP instructions by the PE gate.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::protocol::{
    encode_kernel_ack, Header, Health, Hello, MessageKind, ProtocolError, KERNEL_CAPABILITIES,
    PROTOCOL_VERSION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeError {
    NotConnected,
    NotHandshaken,
    UnsupportedMessage,
    VersionMismatch,
    Malformed,
    OutputTooSmall,
}

/// Connection-scoped handshake state. Plain data: the driver protects it with
/// a spinlock and the host simulator owns it directly.
#[derive(Debug)]
pub struct EngineState {
    connected: bool,
    handshaken: bool,
    engine_version_lo: AtomicU64,
    engine_version_hi: AtomicU64,
    engine_capabilities: AtomicU32,
    health_uptime_ms: AtomicU64,
    health_inflight: AtomicU32,
    health_completed: AtomicU32,
    health_failed: AtomicU32,
    health_breaker_trips: AtomicU32,
    hello_count: AtomicU32,
    health_count: AtomicU32,
}

impl EngineState {
    pub const fn new() -> Self {
        Self {
            connected: false,
            handshaken: false,
            engine_version_lo: AtomicU64::new(0),
            engine_version_hi: AtomicU64::new(0),
            engine_capabilities: AtomicU32::new(0),
            health_uptime_ms: AtomicU64::new(0),
            health_inflight: AtomicU32::new(0),
            health_completed: AtomicU32::new(0),
            health_failed: AtomicU32::new(0),
            health_breaker_trips: AtomicU32::new(0),
            hello_count: AtomicU32::new(0),
            health_count: AtomicU32::new(0),
        }
    }

    /// Called from the Filter Manager connect callback before any message.
    pub fn on_connect(&mut self) {
        self.reset();
        self.connected = true;
    }

    /// Called from the disconnect callback and during unload.
    pub fn on_disconnect(&mut self) {
        self.reset();
    }

    fn reset(&mut self) {
        self.connected = false;
        self.handshaken = false;
        self.engine_version_lo.store(0, Ordering::Relaxed);
        self.engine_version_hi.store(0, Ordering::Relaxed);
        self.engine_capabilities.store(0, Ordering::Relaxed);
        self.health_uptime_ms.store(0, Ordering::Relaxed);
        self.health_inflight.store(0, Ordering::Relaxed);
        self.health_completed.store(0, Ordering::Relaxed);
        self.health_failed.store(0, Ordering::Relaxed);
        self.health_breaker_trips.store(0, Ordering::Relaxed);
        self.hello_count.store(0, Ordering::Relaxed);
        self.health_count.store(0, Ordering::Relaxed);
    }

    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    pub const fn is_handshaken(&self) -> bool {
        self.handshaken
    }

    /// Reconstruct the 16-byte engine version for logs and tests. The driver
    /// does not call this on the hot path.
    pub fn engine_version(&self) -> [u8; 16] {
        let low = self.engine_version_lo.load(Ordering::Relaxed).to_le_bytes();
        let high = self.engine_version_hi.load(Ordering::Relaxed).to_le_bytes();
        let mut bytes = [0u8; 16];
        let mut index = 0usize;
        while index < 8 {
            if let Some(slot) = bytes.get_mut(index) {
                *slot = low.get(index).copied().unwrap_or(0);
            }
            if let Some(slot) = bytes.get_mut(index + 8) {
                *slot = high.get(index).copied().unwrap_or(0);
            }
            index += 1;
        }
        bytes
    }

    pub fn engine_capabilities(&self) -> u32 {
        self.engine_capabilities.load(Ordering::Relaxed)
    }

    pub fn health(&self) -> Health {
        Health {
            uptime_ms: self.health_uptime_ms.load(Ordering::Relaxed),
            inflight: self.health_inflight.load(Ordering::Relaxed),
            completed: self.health_completed.load(Ordering::Relaxed),
            failed: self.health_failed.load(Ordering::Relaxed),
            breaker_trips: self.health_breaker_trips.load(Ordering::Relaxed),
        }
    }

    pub fn hello_count(&self) -> u32 {
        self.hello_count.load(Ordering::Relaxed)
    }

    pub fn health_count(&self) -> u32 {
        self.health_count.load(Ordering::Relaxed)
    }

    /// Process one user-mode message.
    ///
    /// On success returns the number of bytes written to `output` (zero for
    /// health messages). The caller must treat `Err` as a protocol failure and
    /// return an NTSTATUS to the user-mode caller.
    pub fn handle(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, HandshakeError> {
        if !self.connected {
            return Err(HandshakeError::NotConnected);
        }
        let header = Header::decode(input).map_err(map_protocol_error)?;
        match header.kind {
            MessageKind::Hello => self.handle_hello(input, output),
            MessageKind::Health => self.handle_health(input),
            _ => Err(HandshakeError::UnsupportedMessage),
        }
    }

    fn handle_hello(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, HandshakeError> {
        let hello = Hello::decode(input).map_err(map_protocol_error)?;
        if hello.proto_version != PROTOCOL_VERSION {
            return Err(HandshakeError::VersionMismatch);
        }

        let len = encode_kernel_ack(output, 0, KERNEL_CAPABILITIES)
            .map_err(|_| HandshakeError::OutputTooSmall)?;

        self.handshaken = true;
        self.engine_version_lo
            .store(hello.engine_version[0], Ordering::Relaxed);
        self.engine_version_hi
            .store(hello.engine_version[1], Ordering::Relaxed);
        self.engine_capabilities
            .store(hello.capabilities, Ordering::Relaxed);
        self.hello_count.fetch_add(1, Ordering::Relaxed);
        Ok(len)
    }

    fn handle_health(&mut self, input: &[u8]) -> Result<usize, HandshakeError> {
        if !self.handshaken {
            return Err(HandshakeError::NotHandshaken);
        }
        let health = Health::decode(input).map_err(map_protocol_error)?;
        self.health_uptime_ms
            .store(health.uptime_ms, Ordering::Relaxed);
        self.health_inflight
            .store(health.inflight, Ordering::Relaxed);
        self.health_completed
            .store(health.completed, Ordering::Relaxed);
        self.health_failed.store(health.failed, Ordering::Relaxed);
        self.health_breaker_trips
            .store(health.breaker_trips, Ordering::Relaxed);
        self.health_count.fetch_add(1, Ordering::Relaxed);
        Ok(0)
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

fn map_protocol_error(error: ProtocolError) -> HandshakeError {
    match error {
        ProtocolError::UnsupportedVersion => HandshakeError::VersionMismatch,
        _ => HandshakeError::Malformed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        HelloAck, CAP_HEALTH, CAP_HELLO, KERNEL_VERSION_WORDS, PROTOCOL_VERSION,
    };
    use core::mem::size_of;

    fn words(bytes: &[u8; 16]) -> [u64; 2] {
        let low: [u8; 8] = bytes[0..8].try_into().expect("low half");
        let high: [u8; 8] = bytes[8..16].try_into().expect("high half");
        [u64::from_le_bytes(low), u64::from_le_bytes(high)]
    }

    fn hello() -> Hello {
        Hello {
            proto_version: PROTOCOL_VERSION,
            capabilities: CAP_HELLO | CAP_HEALTH,
            engine_version: words(b"cloudmount-0.1.0"),
        }
    }

    fn health() -> Health {
        Health {
            uptime_ms: 1_000,
            inflight: 1,
            completed: 2,
            failed: 3,
            breaker_trips: 4,
        }
    }

    fn connected() -> EngineState {
        let mut state = EngineState::new();
        state.on_connect();
        state
    }

    #[test]
    fn handshake_returns_a_decodable_hello_ack() {
        let mut state = connected();
        let mut input = [0u8; 128];
        let input_len = hello().encode(&mut input).expect("encode hello");
        let mut output = [0u8; 128];

        let output_len = state
            .handle(&input[..input_len], &mut output)
            .expect("handshake");

        let ack = HelloAck::decode(&output[..output_len]).expect("decode ack");
        assert_eq!(ack.status, 0);
        assert_eq!(ack.proto_version, PROTOCOL_VERSION);
        assert_eq!(ack.capabilities, KERNEL_CAPABILITIES);
        assert_eq!(ack.kernel_version, KERNEL_VERSION_WORDS);
        assert!(state.is_handshaken());
        assert_eq!(state.hello_count(), 1);
        assert_eq!(state.engine_version(), *b"cloudmount-0.1.0");
        assert_eq!(state.engine_capabilities(), hello().capabilities);
    }

    #[test]
    fn health_is_rejected_before_hello_and_accepted_after() {
        let mut state = connected();
        let mut input = [0u8; 128];
        let mut output = [0u8; 128];
        let input_len = health().encode(&mut input).expect("encode health");
        assert_eq!(
            state.handle(&input[..input_len], &mut output),
            Err(HandshakeError::NotHandshaken)
        );

        let hello_len = hello().encode(&mut input).expect("encode hello");
        state
            .handle(&input[..hello_len], &mut output)
            .expect("hello");

        let health_len = health().encode(&mut input).expect("encode health");
        assert_eq!(state.handle(&input[..health_len], &mut output), Ok(0));
        assert_eq!(state.health(), health());
        assert_eq!(state.health_count(), 1);
    }

    #[test]
    fn version_mismatch_is_not_malformed() {
        let mut state = connected();
        let mut input = [0u8; 128];
        let input_len = hello().encode(&mut input).expect("encode hello");
        input
            .get_mut(4..6)
            .expect("version field")
            .copy_from_slice(&2u16.to_le_bytes());
        let mut output = [0u8; 128];
        assert_eq!(
            state.handle(&input[..input_len], &mut output),
            Err(HandshakeError::VersionMismatch)
        );
        assert!(!state.is_handshaken());
    }

    #[test]
    fn malformed_and_truncated_messages_are_rejected() {
        let mut state = connected();
        let mut output = [0u8; 128];
        assert_eq!(
            state.handle(&[], &mut output),
            Err(HandshakeError::Malformed)
        );
        assert_eq!(
            state.handle(&[0u8; 23], &mut output),
            Err(HandshakeError::Malformed)
        );
    }

    #[test]
    fn undersized_output_does_not_partially_handshake() {
        let mut state = connected();
        let mut input = [0u8; 128];
        let input_len = hello().encode(&mut input).expect("encode hello");
        let mut output = [0u8; 8];
        assert_eq!(
            state.handle(&input[..input_len], &mut output),
            Err(HandshakeError::OutputTooSmall)
        );
        assert!(!state.is_handshaken());
        assert_eq!(state.hello_count(), 0);
    }

    #[test]
    fn disconnect_resets_the_connection() {
        let mut state = connected();
        let mut input = [0u8; 128];
        let input_len = hello().encode(&mut input).expect("encode hello");
        let mut output = [0u8; 128];
        state
            .handle(&input[..input_len], &mut output)
            .expect("hello");
        assert!(state.is_handshaken());

        state.on_disconnect();
        assert!(!state.is_connected());
        assert!(!state.is_handshaken());
        assert_eq!(
            state.handle(&input[..input_len], &mut output),
            Err(HandshakeError::NotConnected)
        );
    }

    #[test]
    fn unsupported_message_kind_is_rejected() {
        let mut state = connected();
        let mut input = [0u8; 128];
        let request = crate::protocol::HydrateRequest {
            tx_id: 1,
            file_id: 1,
            offset: 0,
            length: 1,
            requestor_pid: 1,
            deadline_ms: 1,
            flags: 0,
        };
        let input_len = request.encode(&mut input).expect("encode request");
        let mut output = [0u8; 128];
        assert_eq!(
            state.handle(&input[..input_len], &mut output),
            Err(HandshakeError::UnsupportedMessage)
        );
    }

    #[test]
    fn state_is_plain_data() {
        assert!(size_of::<EngineState>() <= 128);
    }
}
