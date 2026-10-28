//! Placeholder metadata for the custom GUID reparse point.
//!
//! The kernel will eventually create placeholders with a non-Microsoft reparse
//! tag, a project GUID and `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`; this module
//! defines the on-disk bytes and the state invariants without depending on
//! FltMgr. It is the host-tested contract for the later `FltTagFile` /
//! `FSCTL_SET_REPARSE_POINT` implementation.
//!
//! `CMFLT_REPARSE_GUID` and the vendor tag below are development values.
//! Production use requires the Microsoft-assigned tag/GUID.

use crate::range::{ByteRange, RangeError, RangeSet};

pub const CMFLT_REPARSE_TAG: u32 = 0x0000_434D;
/// Development GUID bytes (`GUID` memory layout, little-endian fields).
pub const CMFLT_REPARSE_GUID: [u8; 16] = [
    0x0E, 0x0D, 0x5F, 0x5A, 0x5B, 0x2F, 0x6E, 0x4A, 0x9C, 0x1D, 0x7B, 0x3F, 0x2E, 0x1A, 0x0C, 0x44,
];

pub const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
pub const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
pub const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

/// Placeholder attributes: recall on data access plus OFFLINE for older
/// software. `RECALL_ON_OPEN` is intentionally not set on files.
pub const PLACEHOLDER_ATTRIBUTES: u32 =
    FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_OFFLINE;

pub const PLACEHOLDER_META_VERSION: u16 = 1;
pub const GUID_REPARSE_HEADER_LEN: usize = 24;
pub const PLACEHOLDER_META_LEN: usize = 32;
pub const GUID_REPARSE_TOTAL_LEN: usize = GUID_REPARSE_HEADER_LEN + PLACEHOLDER_META_LEN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlaceholderState {
    Placeholder = 0,
    Partial = 1,
    Full = 2,
    Pinned = 3,
}

impl PlaceholderState {
    const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Placeholder),
            1 => Some(Self::Partial),
            2 => Some(Self::Full),
            3 => Some(Self::Pinned),
            _ => None,
        }
    }

    /// State invariants relative to `size` and `hydrated_bytes`.
    const fn is_consistent(self, size: u64, hydrated: u64) -> bool {
        match self {
            Self::Placeholder => hydrated == 0,
            Self::Partial => hydrated > 0 && hydrated < size,
            Self::Full | Self::Pinned => hydrated == size,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceholderMeta {
    pub file_id: u64,
    pub size: u64,
    pub hydrated_bytes: u64,
    pub state: PlaceholderState,
    /// Reserved for future flags (pin/recall policy); must be zero for now.
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceholderError {
    BufferTooSmall,
    BadTag,
    BadLength,
    BadGuid,
    UnsupportedVersion,
    InvalidState,
    InvalidField,
    OutOfBounds,
    Overflow,
    Range(RangeError),
}

impl PlaceholderMeta {
    /// Encode a complete `REPARSE_GUID_DATA_BUFFER`-shaped blob.
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, PlaceholderError> {
        if buf.len() < GUID_REPARSE_TOTAL_LEN {
            return Err(PlaceholderError::BufferTooSmall);
        }
        if !self.state.is_consistent(self.size, self.hydrated_bytes) {
            return Err(PlaceholderError::InvalidState);
        }
        if self.flags != 0 {
            return Err(PlaceholderError::InvalidField);
        }

        write_u32(buf, 0, CMFLT_REPARSE_TAG)?;
        write_u16(buf, 4, PLACEHOLDER_META_LEN as u16)?;
        write_u16(buf, 6, 0)?;
        write_bytes(buf, 8, &CMFLT_REPARSE_GUID)?;
        write_u16(buf, GUID_REPARSE_HEADER_LEN, PLACEHOLDER_META_VERSION)?;
        write_u8(buf, GUID_REPARSE_HEADER_LEN + 2, self.state as u8)?;
        write_u8(buf, GUID_REPARSE_HEADER_LEN + 3, 0)?;
        write_u32(buf, GUID_REPARSE_HEADER_LEN + 4, self.flags)?;
        write_u64(buf, GUID_REPARSE_HEADER_LEN + 8, self.file_id)?;
        write_u64(buf, GUID_REPARSE_HEADER_LEN + 16, self.size)?;
        write_u64(buf, GUID_REPARSE_HEADER_LEN + 24, self.hydrated_bytes)?;
        Ok(GUID_REPARSE_TOTAL_LEN)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, PlaceholderError> {
        if buf.len() < GUID_REPARSE_TOTAL_LEN {
            return Err(PlaceholderError::BufferTooSmall);
        }
        if read_u32(buf, 0)? != CMFLT_REPARSE_TAG {
            return Err(PlaceholderError::BadTag);
        }
        if read_u16(buf, 4)? as usize != PLACEHOLDER_META_LEN {
            return Err(PlaceholderError::BadLength);
        }
        if read_u16(buf, 6)? != 0 {
            return Err(PlaceholderError::InvalidField);
        }
        if read_array::<16>(buf, 8)? != CMFLT_REPARSE_GUID {
            return Err(PlaceholderError::BadGuid);
        }
        if read_u16(buf, GUID_REPARSE_HEADER_LEN)? != PLACEHOLDER_META_VERSION {
            return Err(PlaceholderError::UnsupportedVersion);
        }
        let state = PlaceholderState::from_u8(read_u8(buf, GUID_REPARSE_HEADER_LEN + 2)?)
            .ok_or(PlaceholderError::InvalidState)?;
        if read_u8(buf, GUID_REPARSE_HEADER_LEN + 3)? != 0 {
            return Err(PlaceholderError::InvalidField);
        }
        let flags = read_u32(buf, GUID_REPARSE_HEADER_LEN + 4)?;
        if flags != 0 {
            return Err(PlaceholderError::InvalidField);
        }
        let file_id = read_u64(buf, GUID_REPARSE_HEADER_LEN + 8)?;
        let size = read_u64(buf, GUID_REPARSE_HEADER_LEN + 16)?;
        let hydrated_bytes = read_u64(buf, GUID_REPARSE_HEADER_LEN + 24)?;
        if !state.is_consistent(size, hydrated_bytes) {
            return Err(PlaceholderError::InvalidState);
        }
        Ok(Self {
            file_id,
            size,
            hydrated_bytes,
            state,
            flags,
        })
    }
}

/// A placeholder plus the local hydrated-range set.
///
/// The fields are private so every mutation goes through [`Placeholder::mark_hydrated`],
/// [`Placeholder::dehydrate`] or [`Placeholder::pin`]; this keeps `state` and
/// `hydrated_bytes` consistent with the range set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placeholder {
    meta: PlaceholderMeta,
    ranges: RangeSet,
}

impl Placeholder {
    pub const fn new(file_id: u64, size: u64) -> Self {
        Self {
            meta: PlaceholderMeta {
                file_id,
                size,
                hydrated_bytes: 0,
                state: PlaceholderState::Placeholder,
                flags: 0,
            },
            ranges: RangeSet::new(),
        }
    }

    pub const fn meta(&self) -> &PlaceholderMeta {
        &self.meta
    }

    pub const fn ranges(&self) -> &RangeSet {
        &self.ranges
    }

    /// True when any byte in the requested range is not local. Zero-length
    /// reads never recall; a range past EOF is an explicit error.
    pub fn needs_recall(&self, offset: u64, length: u64) -> Result<bool, PlaceholderError> {
        if length == 0 {
            return Ok(false);
        }
        let end = offset
            .checked_add(length)
            .ok_or(PlaceholderError::Overflow)?;
        if end > self.meta.size {
            return Err(PlaceholderError::OutOfBounds);
        }
        Ok(!self.ranges.covers(offset, length))
    }

    /// Record that `range` is now local. Overlaps merge; ranges past EOF are
    /// rejected before the set is modified.
    pub fn mark_hydrated(&mut self, range: ByteRange) -> Result<(), PlaceholderError> {
        if range.is_empty() {
            return Ok(());
        }
        let end = range.end().ok_or(PlaceholderError::Overflow)?;
        if end > self.meta.size {
            return Err(PlaceholderError::OutOfBounds);
        }
        self.ranges.insert(range).map_err(PlaceholderError::Range)?;
        self.refresh();
        Ok(())
    }

    /// Pin is only valid once the whole file is local.
    pub fn pin(&mut self) -> Result<(), PlaceholderError> {
        if self.meta.hydrated_bytes != self.meta.size {
            return Err(PlaceholderError::InvalidState);
        }
        self.meta.state = PlaceholderState::Pinned;
        Ok(())
    }

    pub fn unpin(&mut self) {
        if self.meta.state == PlaceholderState::Pinned {
            self.meta.state = PlaceholderState::Full;
        }
    }

    pub fn dehydrate(&mut self) {
        self.ranges.clear();
        self.meta.hydrated_bytes = 0;
        self.meta.state = PlaceholderState::Placeholder;
    }

    fn refresh(&mut self) {
        let hydrated = self.ranges.total().min(self.meta.size);
        self.meta.hydrated_bytes = hydrated;
        self.meta.state = if hydrated == 0 {
            PlaceholderState::Placeholder
        } else if hydrated >= self.meta.size {
            if self.meta.state == PlaceholderState::Pinned {
                PlaceholderState::Pinned
            } else {
                PlaceholderState::Full
            }
        } else {
            PlaceholderState::Partial
        };
    }
}

/// True for Microsoft-owned reparse tags; our tag must return false.
pub const fn is_microsoft_reparse_tag(tag: u32) -> bool {
    tag & 0x8000_0000 != 0
}

/// True for name-surrogate reparse tags; our tag is not one.
pub const fn is_name_surrogate_reparse_tag(tag: u32) -> bool {
    tag & 0x2000_0000 != 0
}

fn checked_end(offset: usize, length: usize) -> Result<usize, PlaceholderError> {
    offset.checked_add(length).ok_or(PlaceholderError::Overflow)
}

fn write_bytes(buf: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), PlaceholderError> {
    let end = checked_end(offset, bytes.len())?;
    let slot = buf
        .get_mut(offset..end)
        .ok_or(PlaceholderError::BufferTooSmall)?;
    slot.copy_from_slice(bytes);
    Ok(())
}

fn write_u8(buf: &mut [u8], offset: usize, value: u8) -> Result<(), PlaceholderError> {
    write_bytes(buf, offset, &[value])
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) -> Result<(), PlaceholderError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) -> Result<(), PlaceholderError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) -> Result<(), PlaceholderError> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

fn read_array<const N: usize>(buf: &[u8], offset: usize) -> Result<[u8; N], PlaceholderError> {
    let end = checked_end(offset, N)?;
    let slice = buf
        .get(offset..end)
        .ok_or(PlaceholderError::BufferTooSmall)?;
    slice
        .try_into()
        .map_err(|_| PlaceholderError::BufferTooSmall)
}

fn read_u8(buf: &[u8], offset: usize) -> Result<u8, PlaceholderError> {
    Ok(read_array::<1>(buf, offset)?[0])
}

fn read_u16(buf: &[u8], offset: usize) -> Result<u16, PlaceholderError> {
    Ok(u16::from_le_bytes(read_array::<2>(buf, offset)?))
}

fn read_u32(buf: &[u8], offset: usize) -> Result<u32, PlaceholderError> {
    Ok(u32::from_le_bytes(read_array::<4>(buf, offset)?))
}

fn read_u64(buf: &[u8], offset: usize) -> Result<u64, PlaceholderError> {
    Ok(u64::from_le_bytes(read_array::<8>(buf, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(state: PlaceholderState, size: u64, hydrated: u64) -> PlaceholderMeta {
        PlaceholderMeta {
            file_id: 0x1122_3344_5566_7788,
            size,
            hydrated_bytes: hydrated,
            state,
            flags: 0,
        }
    }

    #[test]
    fn tag_is_not_microsoft_owned() {
        assert!(!is_microsoft_reparse_tag(CMFLT_REPARSE_TAG));
        assert!(!is_name_surrogate_reparse_tag(CMFLT_REPARSE_TAG));
        assert!(is_microsoft_reparse_tag(0x9000_001A));
    }

    #[test]
    fn placeholder_attributes_are_recall_and_offline() {
        assert_ne!(
            PLACEHOLDER_ATTRIBUTES & FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
            0
        );
        assert_ne!(PLACEHOLDER_ATTRIBUTES & FILE_ATTRIBUTE_OFFLINE, 0);
        assert_eq!(PLACEHOLDER_ATTRIBUTES & FILE_ATTRIBUTE_RECALL_ON_OPEN, 0);
    }

    #[test]
    fn roundtrips_each_state() {
        let cases = [
            meta(PlaceholderState::Placeholder, 4096, 0),
            meta(PlaceholderState::Partial, 4096, 1024),
            meta(PlaceholderState::Full, 4096, 4096),
            meta(PlaceholderState::Pinned, 4096, 4096),
        ];
        let mut buf = [0u8; GUID_REPARSE_TOTAL_LEN];
        for case in cases {
            let len = case.encode(&mut buf).expect("encode");
            assert_eq!(len, GUID_REPARSE_TOTAL_LEN);
            assert_eq!(PlaceholderMeta::decode(&buf[..len]), Ok(case));
        }
    }

    #[test]
    fn rejects_inconsistent_state() {
        let mut buf = [0u8; GUID_REPARSE_TOTAL_LEN];
        assert_eq!(
            meta(PlaceholderState::Placeholder, 4096, 1).encode(&mut buf),
            Err(PlaceholderError::InvalidState)
        );
        assert_eq!(
            meta(PlaceholderState::Partial, 4096, 0).encode(&mut buf),
            Err(PlaceholderError::InvalidState)
        );
        assert_eq!(
            meta(PlaceholderState::Full, 4096, 2048).encode(&mut buf),
            Err(PlaceholderError::InvalidState)
        );
        assert_eq!(
            meta(PlaceholderState::Partial, 0, 0).encode(&mut buf),
            Err(PlaceholderError::InvalidState)
        );
    }

    #[test]
    fn rejects_bad_tag_guid_and_truncation() {
        let value = meta(PlaceholderState::Placeholder, 4096, 0);
        let mut buf = [0u8; GUID_REPARSE_TOTAL_LEN];
        let len = value.encode(&mut buf).expect("encode");

        for shorter in 0..len {
            assert!(PlaceholderMeta::decode(&buf[..shorter]).is_err());
        }

        let mut bad_tag = buf;
        bad_tag[0] ^= 0xFF;
        assert_eq!(
            PlaceholderMeta::decode(&bad_tag),
            Err(PlaceholderError::BadTag)
        );

        let mut bad_guid = buf;
        bad_guid[8] ^= 0xFF;
        assert_eq!(
            PlaceholderMeta::decode(&bad_guid),
            Err(PlaceholderError::BadGuid)
        );
    }

    #[test]
    fn rejects_bad_version_and_reserved_bytes() {
        let value = meta(PlaceholderState::Placeholder, 4096, 0);
        let mut buf = [0u8; GUID_REPARSE_TOTAL_LEN];
        value.encode(&mut buf).expect("encode");

        let mut bad_version = buf;
        bad_version[GUID_REPARSE_HEADER_LEN] = 2;
        assert_eq!(
            PlaceholderMeta::decode(&bad_version),
            Err(PlaceholderError::UnsupportedVersion)
        );

        let mut bad_reserved = buf;
        bad_reserved[GUID_REPARSE_HEADER_LEN + 3] = 1;
        assert_eq!(
            PlaceholderMeta::decode(&bad_reserved),
            Err(PlaceholderError::InvalidField)
        );
    }

    #[test]
    fn placeholder_transitions_and_recall() {
        let mut value = Placeholder::new(7, 4096);
        assert_eq!(value.meta().state, PlaceholderState::Placeholder);
        assert_eq!(value.meta().hydrated_bytes, 0);
        assert_eq!(value.needs_recall(0, 1), Ok(true));
        assert_eq!(value.needs_recall(0, 0), Ok(false));

        value
            .mark_hydrated(ByteRange::new(0, 1024))
            .expect("partial");
        assert_eq!(value.meta().state, PlaceholderState::Partial);
        assert_eq!(value.meta().hydrated_bytes, 1024);
        assert_eq!(value.needs_recall(0, 1024), Ok(false));
        assert_eq!(value.needs_recall(1024, 1), Ok(true));

        value
            .mark_hydrated(ByteRange::new(1024, 3072))
            .expect("full");
        assert_eq!(value.meta().state, PlaceholderState::Full);
        assert_eq!(value.meta().hydrated_bytes, 4096);
        assert_eq!(value.needs_recall(0, 4096), Ok(false));

        value.pin().expect("pin");
        assert_eq!(value.meta().state, PlaceholderState::Pinned);
        value.unpin();
        assert_eq!(value.meta().state, PlaceholderState::Full);

        value.dehydrate();
        assert_eq!(value.meta().state, PlaceholderState::Placeholder);
        assert_eq!(value.meta().hydrated_bytes, 0);
        assert_eq!(value.needs_recall(0, 1), Ok(true));
    }

    #[test]
    fn placeholder_rejects_out_of_bounds_ranges() {
        let mut value = Placeholder::new(7, 4096);
        assert_eq!(
            value.mark_hydrated(ByteRange::new(4096, 1)),
            Err(PlaceholderError::OutOfBounds)
        );
        assert_eq!(
            value.needs_recall(4096, 1),
            Err(PlaceholderError::OutOfBounds)
        );
        assert_eq!(
            value.mark_hydrated(ByteRange::new(u64::MAX, 1)),
            Err(PlaceholderError::Overflow)
        );
        assert_eq!(value.meta().state, PlaceholderState::Placeholder);
    }

    #[test]
    fn adjacent_hydration_ranges_merge() {
        let mut value = Placeholder::new(7, 4096);
        value.mark_hydrated(ByteRange::new(0, 1024)).expect("first");
        value
            .mark_hydrated(ByteRange::new(1024, 1024))
            .expect("second");
        assert_eq!(value.meta().state, PlaceholderState::Partial);
        assert_eq!(value.meta().hydrated_bytes, 2048);
        assert_eq!(value.ranges().len(), 1);
        assert_eq!(value.needs_recall(0, 2048), Ok(false));
        assert_eq!(value.needs_recall(2048, 1), Ok(true));
    }

    #[test]
    fn pin_requires_a_full_placeholder() {
        let mut value = Placeholder::new(7, 4096);
        assert_eq!(value.pin(), Err(PlaceholderError::InvalidState));
        value.mark_hydrated(ByteRange::new(0, 4096)).expect("full");
        value.pin().expect("pin");
        assert_eq!(value.meta().state, PlaceholderState::Pinned);
    }
}
