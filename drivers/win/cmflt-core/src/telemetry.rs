//! Fixed-size in-memory telemetry ring.
//!
//! This is the black-box event shape the driver can keep without allocation.
//! It is deliberately small and host-tested; the ETW/event-packet transport is
//! a later wiring step.

pub const TELEMETRY_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TelemetryKind {
    HydrateStart = 1,
    HydrateOk = 2,
    HydrateTimeout = 3,
    HydrateCancel = 4,
    HydrateFail = 5,
    BreakerTrip = 6,
    EngineDown = 7,
    EngineUp = 8,
    PlaceholderCreate = 9,
    Dehydrate = 10,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TelemetryEvent {
    pub seq: u64,
    pub kind: TelemetryKind,
    pub tx_id: u64,
    pub code: i32,
}

#[derive(Clone, Copy)]
pub struct TelemetryRing {
    events: [Option<TelemetryEvent>; TELEMETRY_CAPACITY],
    head: usize,
    len: usize,
    dropped: u64,
    next_seq: u64,
}

impl TelemetryRing {
    pub const fn new() -> Self {
        Self {
            events: [None; TELEMETRY_CAPACITY],
            head: 0,
            len: 0,
            dropped: 0,
            next_seq: 1,
        }
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
        self.dropped = 0;
        self.next_seq = 1;
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Append an event, overwriting the oldest when full. Returns its sequence.
    pub fn push(&mut self, kind: TelemetryKind, tx_id: u64, code: i32) -> u64 {
        let event = TelemetryEvent {
            seq: self.next_seq,
            kind,
            tx_id,
            code,
        };
        self.next_seq = self.next_seq.wrapping_add(1).max(1);

        if self.len < TELEMETRY_CAPACITY {
            let index = (self.head + self.len) % TELEMETRY_CAPACITY;
            if let Some(slot) = self.events.get_mut(index) {
                *slot = Some(event);
            }
            self.len += 1;
        } else {
            if let Some(slot) = self.events.get_mut(self.head) {
                *slot = Some(event);
            }
            self.head = (self.head + 1) % TELEMETRY_CAPACITY;
            self.dropped = self.dropped.saturating_add(1);
        }
        event.seq
    }

    pub fn iter(&self) -> impl Iterator<Item = &TelemetryEvent> {
        (0..self.len).filter_map(move |index| {
            self.events
                .get((self.head + index) % TELEMETRY_CAPACITY)
                .and_then(|event| event.as_ref())
        })
    }

    pub fn latest(&self) -> Option<TelemetryEvent> {
        self.iter().last().copied()
    }
}

impl Default for TelemetryRing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_assign_increasing_sequence_numbers() {
        let mut ring = TelemetryRing::new();
        assert_eq!(ring.push(TelemetryKind::EngineUp, 0, 0), 1);
        assert_eq!(ring.push(TelemetryKind::HydrateStart, 7, 0), 2);
        assert_eq!(ring.push(TelemetryKind::HydrateOk, 7, 0), 3);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.latest().map(|event| event.seq), Some(3));
        assert_eq!(
            ring.latest().map(|event| event.kind),
            Some(TelemetryKind::HydrateOk)
        );
    }

    #[test]
    fn ring_overwrites_oldest_and_counts_drops() {
        let mut ring = TelemetryRing::new();
        for index in 0..(TELEMETRY_CAPACITY as u64 + 2) {
            ring.push(TelemetryKind::PlaceholderCreate, index, 0);
        }
        assert_eq!(ring.len(), TELEMETRY_CAPACITY);
        assert_eq!(ring.dropped(), 2);
        assert_eq!(ring.iter().next().map(|event| event.tx_id), Some(2));
        assert_eq!(
            ring.iter().last().map(|event| event.tx_id),
            Some(TELEMETRY_CAPACITY as u64 + 1)
        );
    }

    #[test]
    fn clear_resets_the_ring_and_sequence() {
        let mut ring = TelemetryRing::new();
        ring.push(TelemetryKind::EngineDown, 1, -1);
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.dropped(), 0);
        assert_eq!(ring.push(TelemetryKind::EngineUp, 0, 0), 1);
    }
}
