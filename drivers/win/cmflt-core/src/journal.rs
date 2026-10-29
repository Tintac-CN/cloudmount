//! Crash-consistency journal and recovery planning.
//!
//! The kernel does not write a persistent journal: all state that must survive
//! a crash lives in the user-mode engine. This module defines the compact
//! record the engine keeps for each hydration transaction and the pure
//! reconciliation decision used after a restart.
//!
//! A record says "this range was being hydrated for this file size". On restart
//! the engine compares the observed file size and local ranges:
//! - size mismatch -> rehydrate all;
//! - range still local -> trust it;
//! - range missing -> rehydrate just that range.

use crate::placeholder::PlaceholderState;
use crate::range::{ByteRange, RangeSet};

pub const JOURNAL_CAPACITY: usize = 64;
pub const JOURNAL_RECORD_LEN: usize = 48;
pub const JOURNAL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalRecord {
    pub file_id: u64,
    pub tx_id: u64,
    pub offset: u64,
    pub length: u64,
    pub file_size: u64,
    pub state: PlaceholderState,
    pub flags: u32,
}

impl JournalRecord {
    pub const fn range(&self) -> ByteRange {
        ByteRange::new(self.offset, self.length)
    }

    /// Encode the fixed 48-byte record.
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, JournalError> {
        if buf.len() < JOURNAL_RECORD_LEN {
            return Err(JournalError::BufferTooSmall);
        }
        if self.length == 0 {
            return Err(JournalError::InvalidField);
        }
        let end = self
            .offset
            .checked_add(self.length)
            .ok_or(JournalError::Overflow)?;
        if end > self.file_size {
            return Err(JournalError::InvalidField);
        }
        if self.flags != 0 {
            return Err(JournalError::InvalidField);
        }
        write_u16(buf, 0, JOURNAL_VERSION)?;
        write_u8(buf, 2, self.state as u8)?;
        write_u8(buf, 3, 0)?;
        write_u32(buf, 4, self.flags)?;
        write_u64(buf, 8, self.file_id)?;
        write_u64(buf, 16, self.tx_id)?;
        write_u64(buf, 24, self.offset)?;
        write_u64(buf, 32, self.length)?;
        write_u64(buf, 40, self.file_size)?;
        Ok(JOURNAL_RECORD_LEN)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, JournalError> {
        if buf.len() < JOURNAL_RECORD_LEN {
            return Err(JournalError::BufferTooSmall);
        }
        if read_u16(buf, 0)? != JOURNAL_VERSION {
            return Err(JournalError::BadVersion);
        }
        let state = PlaceholderState::from_u8(read_u8(buf, 2)?).ok_or(JournalError::BadState)?;
        if read_u8(buf, 3)? != 0 {
            return Err(JournalError::InvalidField);
        }
        let flags = read_u32(buf, 4)?;
        if flags != 0 {
            return Err(JournalError::InvalidField);
        }
        let file_id = read_u64(buf, 8)?;
        let tx_id = read_u64(buf, 16)?;
        let offset = read_u64(buf, 24)?;
        let length = read_u64(buf, 32)?;
        let file_size = read_u64(buf, 40)?;
        let end = offset.checked_add(length).ok_or(JournalError::Overflow)?;
        if length == 0 || end > file_size {
            return Err(JournalError::InvalidField);
        }
        Ok(Self {
            file_id,
            tx_id,
            offset,
            length,
            file_size,
            state,
            flags,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalError {
    BufferTooSmall,
    BadVersion,
    BadState,
    InvalidField,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryAction {
    /// The recorded range is still local and the size matches.
    Trusted,
    /// The size matches but the recorded range is not fully local.
    RehydrateRange(ByteRange),
    /// The file size changed; discard local range bookkeeping.
    RehydrateAll,
}

/// Plan recovery for one journal record against observed state.
pub fn plan_recovery(
    record: &JournalRecord,
    observed_size: u64,
    observed: &RangeSet,
) -> RecoveryAction {
    if record.file_size != observed_size {
        return RecoveryAction::RehydrateAll;
    }
    if observed.covers(record.offset, record.length) {
        RecoveryAction::Trusted
    } else {
        RecoveryAction::RehydrateRange(record.range())
    }
}

/// Fixed-capacity ring. When full, the oldest record is dropped and counted.
#[derive(Clone, Copy)]
pub struct Journal {
    records: [JournalRecord; JOURNAL_CAPACITY],
    head: usize,
    len: usize,
    dropped: u64,
}

impl Journal {
    const EMPTY: JournalRecord = JournalRecord {
        file_id: 0,
        tx_id: 0,
        offset: 0,
        length: 1,
        file_size: 1,
        state: PlaceholderState::Placeholder,
        flags: 0,
    };

    pub const fn new() -> Self {
        Self {
            records: [Self::EMPTY; JOURNAL_CAPACITY],
            head: 0,
            len: 0,
            dropped: 0,
        }
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
        self.dropped = 0;
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

    pub fn push(&mut self, record: JournalRecord) {
        if self.len < JOURNAL_CAPACITY {
            let index = (self.head + self.len) % JOURNAL_CAPACITY;
            if let Some(slot) = self.records.get_mut(index) {
                *slot = record;
            }
            self.len += 1;
        } else {
            if let Some(slot) = self.records.get_mut(self.head) {
                *slot = record;
            }
            self.head = (self.head + 1) % JOURNAL_CAPACITY;
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &JournalRecord> {
        (0..self.len)
            .filter_map(move |index| self.records.get((self.head + index) % JOURNAL_CAPACITY))
    }

    /// Most recent record for `file_id`, if any.
    pub fn find_latest(&self, file_id: u64) -> Option<JournalRecord> {
        self.iter()
            .rev()
            .find(|record| record.file_id == file_id)
            .copied()
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}

fn checked_end(offset: usize, length: usize) -> Result<usize, JournalError> {
    offset.checked_add(length).ok_or(JournalError::Overflow)
}

fn write_bytes(buf: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), JournalError> {
    let end = checked_end(offset, bytes.len())?;
    let slot = buf
        .get_mut(offset..end)
        .ok_or(JournalError::BufferTooSmall)?;
    slot.copy_from_slice(bytes);
    Ok(())
}

fn write_u8(buf: &mut [u8], offset: usize, value: u8) -> Result<(), JournalError> {
    write_bytes(buf, offset, &[value])
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) -> Result<(), JournalError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) -> Result<(), JournalError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) -> Result<(), JournalError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn read_array<const N: usize>(buf: &[u8], offset: usize) -> Result<[u8; N], JournalError> {
    let end = checked_end(offset, N)?;
    let slice = buf.get(offset..end).ok_or(JournalError::BufferTooSmall)?;
    slice.try_into().map_err(|_| JournalError::BufferTooSmall)
}

fn read_u8(buf: &[u8], offset: usize) -> Result<u8, JournalError> {
    Ok(read_array::<1>(buf, offset)?[0])
}

fn read_u16(buf: &[u8], offset: usize) -> Result<u16, JournalError> {
    Ok(u16::from_le_bytes(read_array::<2>(buf, offset)?))
}

fn read_u32(buf: &[u8], offset: usize) -> Result<u32, JournalError> {
    Ok(u32::from_le_bytes(read_array::<4>(buf, offset)?))
}

fn read_u64(buf: &[u8], offset: usize) -> Result<u64, JournalError> {
    Ok(u64::from_le_bytes(read_array::<8>(buf, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(file_id: u64, tx_id: u64) -> JournalRecord {
        JournalRecord {
            file_id,
            tx_id,
            offset: 0,
            length: 1024,
            file_size: 4096,
            state: PlaceholderState::Partial,
            flags: 0,
        }
    }

    #[test]
    fn record_round_trip() {
        let value = record(7, 9);
        let mut buf = [0u8; JOURNAL_RECORD_LEN];
        let len = value.encode(&mut buf).expect("encode");
        assert_eq!(len, JOURNAL_RECORD_LEN);
        assert_eq!(JournalRecord::decode(&buf[..len]), Ok(value));
    }

    #[test]
    fn record_rejects_invalid_fields() {
        let value = record(7, 9);
        let mut buf = [0u8; JOURNAL_RECORD_LEN];
        value.encode(&mut buf).expect("encode");

        let mut bad_version = buf;
        bad_version[0] = 2;
        assert_eq!(
            JournalRecord::decode(&bad_version),
            Err(JournalError::BadVersion)
        );

        let mut bad_state = buf;
        bad_state[2] = 9;
        assert_eq!(
            JournalRecord::decode(&bad_state),
            Err(JournalError::BadState)
        );

        let mut zero_length = value;
        zero_length.length = 0;
        assert_eq!(
            zero_length.encode(&mut buf),
            Err(JournalError::InvalidField)
        );

        let mut past_eof = value;
        past_eof.offset = 4096;
        past_eof.length = 1;
        assert_eq!(past_eof.encode(&mut buf), Err(JournalError::InvalidField));
    }

    #[test]
    fn ring_keeps_newest_records_and_counts_drops() {
        let mut journal = Journal::new();
        for index in 0..(JOURNAL_CAPACITY as u64 + 2) {
            journal.push(record(index, index + 1));
        }
        assert_eq!(journal.len(), JOURNAL_CAPACITY);
        assert_eq!(journal.dropped(), 2);
        assert_eq!(journal.iter().next().map(|record| record.file_id), Some(2));
        assert_eq!(
            journal.iter().last().map(|record| record.file_id),
            Some(JOURNAL_CAPACITY as u64 + 1)
        );
        assert_eq!(journal.find_latest(2).map(|record| record.tx_id), Some(3));
        assert_eq!(journal.find_latest(1), None);
    }

    #[test]
    fn recovery_trusts_covered_range_and_rehydrates_missing_or_resized() {
        let value = record(7, 9);
        let mut ranges = RangeSet::new();
        ranges.insert(ByteRange::new(0, 1024)).expect("range");
        assert_eq!(
            plan_recovery(&value, 4096, &ranges),
            RecoveryAction::Trusted
        );

        let empty = RangeSet::new();
        assert_eq!(
            plan_recovery(&value, 4096, &empty),
            RecoveryAction::RehydrateRange(ByteRange::new(0, 1024))
        );

        assert_eq!(
            plan_recovery(&value, 8192, &ranges),
            RecoveryAction::RehydrateAll
        );
    }

    #[test]
    fn clear_resets_the_ring() {
        let mut journal = Journal::new();
        journal.push(record(1, 1));
        journal.push(record(2, 2));
        assert_eq!(journal.len(), 2);
        journal.clear();
        assert!(journal.is_empty());
        assert_eq!(journal.dropped(), 0);
        assert_eq!(journal.find_latest(1), None);
    }
}
