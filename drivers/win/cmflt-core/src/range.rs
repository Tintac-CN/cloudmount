//! Fixed-capacity hydrated-range accounting.
//!
//! Placeholders may be partially hydrated. The driver keeps a small, bounded
//! set of covered byte ranges per file; an insertion that would exceed the
//! fixed capacity fails instead of allocating. Callers must treat
//! [`RangeSet::covers`] as an exact predicate: "not covered" means "must
//! recall", never "assume zero data".

/// A half-open byte range `[offset, offset + length)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRange {
    pub offset: u64,
    pub length: u64,
}

impl ByteRange {
    pub const EMPTY: Self = Self {
        offset: 0,
        length: 0,
    };

    pub const fn new(offset: u64, length: u64) -> Self {
        Self { offset, length }
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    /// Exclusive end, or `None` on overflow.
    pub const fn end(self) -> Option<u64> {
        self.offset.checked_add(self.length)
    }

    /// True if the ranges overlap or touch. Overflowing ranges are never
    /// considered mergeable; they are rejected at insertion time.
    pub fn overlaps_or_adjacent(self, other: Self) -> bool {
        match (self.end(), other.end()) {
            (Some(self_end), Some(other_end)) => {
                self.offset <= other_end && other.offset <= self_end
            }
            _ => false,
        }
    }

    fn union(self, other: Self) -> Result<Self, RangeError> {
        let end = match (self.end(), other.end()) {
            (Some(a), Some(b)) => {
                if a >= b {
                    a
                } else {
                    b
                }
            }
            _ => return Err(RangeError::Overflow),
        };
        let offset = if self.offset <= other.offset {
            self.offset
        } else {
            other.offset
        };
        let length = end.checked_sub(offset).ok_or(RangeError::Overflow)?;
        Ok(Self { offset, length })
    }
}

/// Errors from range insertion and gap enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    Overflow,
    Capacity,
    OutputTooSmall,
}

/// Maximum number of disjoint ranges tracked per file.
pub const MAX_RANGES: usize = 32;

/// Sorted, merged, fixed-capacity range set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeSet {
    ranges: [ByteRange; MAX_RANGES],
    len: usize,
}

impl Default for RangeSet {
    fn default() -> Self {
        Self::new()
    }
}

impl RangeSet {
    pub const fn new() -> Self {
        Self {
            ranges: [ByteRange::EMPTY; MAX_RANGES],
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[ByteRange] {
        self.ranges.get(..self.len).unwrap_or(&[])
    }

    /// Insert a range, merging overlaps and adjacency.
    ///
    /// On error the set is unchanged.
    pub fn insert(&mut self, range: ByteRange) -> Result<(), RangeError> {
        if range.is_empty() {
            return Ok(());
        }
        if range.end().is_none() {
            return Err(RangeError::Overflow);
        }

        let mut work = [ByteRange::EMPTY; MAX_RANGES + 1];
        let mut work_len = 0usize;

        for index in 0..self.len {
            let current = self.ranges.get(index).copied().unwrap_or(ByteRange::EMPTY);
            if let Some(slot) = work.get_mut(work_len) {
                *slot = current;
                work_len += 1;
            }
        }
        if let Some(slot) = work.get_mut(work_len) {
            *slot = range;
            work_len += 1;
        } else {
            return Err(RangeError::Capacity);
        }

        if let Some(slice) = work.get_mut(..work_len) {
            slice.sort_unstable_by_key(|item| item.offset);
        } else {
            return Err(RangeError::Capacity);
        }

        let mut merged = [ByteRange::EMPTY; MAX_RANGES];
        let mut merged_len = 0usize;
        let mut index = 0usize;

        while index < work_len {
            let mut current = work.get(index).copied().unwrap_or(ByteRange::EMPTY);
            index += 1;
            while index < work_len {
                let next = work.get(index).copied().unwrap_or(ByteRange::EMPTY);
                if current.overlaps_or_adjacent(next) {
                    current = current.union(next)?;
                    index += 1;
                } else {
                    break;
                }
            }
            if merged_len >= MAX_RANGES {
                return Err(RangeError::Capacity);
            }
            if let Some(slot) = merged.get_mut(merged_len) {
                *slot = current;
                merged_len += 1;
            }
        }

        for index in 0..merged_len {
            if let Some(slot) = self.ranges.get_mut(index) {
                *slot = merged.get(index).copied().unwrap_or(ByteRange::EMPTY);
            }
        }
        self.len = merged_len;
        Ok(())
    }

    /// True if every byte in `[offset, offset + length)` is present.
    pub fn covers(&self, offset: u64, length: u64) -> bool {
        if length == 0 {
            return true;
        }
        let end = match offset.checked_add(length) {
            Some(value) => value,
            None => return false,
        };
        let mut cursor = offset;
        for index in 0..self.len {
            let range = self.ranges.get(index).copied().unwrap_or(ByteRange::EMPTY);
            let range_end = match range.end() {
                Some(value) => value,
                None => return false,
            };
            if range_end <= cursor {
                continue;
            }
            if range.offset > cursor {
                return false;
            }
            cursor = range_end;
            if cursor >= end {
                return true;
            }
        }
        false
    }

    /// Enumerate the gaps in `[offset, offset + length)` into `out`.
    ///
    /// Returns the number of gaps written. If `out` is too small, returns
    /// [`RangeError::OutputTooSmall`] after writing a prefix.
    pub fn missing(
        &self,
        offset: u64,
        length: u64,
        out: &mut [ByteRange],
    ) -> Result<usize, RangeError> {
        if length == 0 {
            return Ok(0);
        }
        let end = offset.checked_add(length).ok_or(RangeError::Overflow)?;
        let mut cursor = offset;
        let mut count = 0usize;

        for index in 0..self.len {
            let range = self.ranges.get(index).copied().unwrap_or(ByteRange::EMPTY);
            let range_end = range.end().ok_or(RangeError::Overflow)?;
            if range_end <= cursor {
                continue;
            }
            if range.offset >= end {
                break;
            }
            if range.offset > cursor {
                let gap_end = if range.offset < end {
                    range.offset
                } else {
                    end
                };
                if let Some(slot) = out.get_mut(count) {
                    *slot = ByteRange::new(cursor, gap_end - cursor);
                    count += 1;
                } else {
                    return Err(RangeError::OutputTooSmall);
                }
                cursor = gap_end;
                if cursor >= end {
                    return Ok(count);
                }
            }
            cursor = if cursor >= range_end {
                cursor
            } else {
                range_end
            };
            if cursor >= end {
                return Ok(count);
            }
        }

        if cursor < end {
            if let Some(slot) = out.get_mut(count) {
                *slot = ByteRange::new(cursor, end - cursor);
                count += 1;
            } else {
                return Err(RangeError::OutputTooSmall);
            }
        }
        Ok(count)
    }

    /// Sum of covered bytes, saturating at `u64::MAX`.
    pub fn total(&self) -> u64 {
        let mut total = 0u64;
        for index in 0..self.len {
            if let Some(range) = self.ranges.get(index) {
                total = total.saturating_add(range.length);
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_and_overlapping_ranges_merge() {
        let mut set = RangeSet::new();
        set.insert(ByteRange::new(10, 10)).expect("insert");
        set.insert(ByteRange::new(20, 5)).expect("merge adjacent");
        set.insert(ByteRange::new(0, 12))
            .expect("merge overlapping");
        assert_eq!(set.as_slice(), &[ByteRange::new(0, 25)]);
        assert_eq!(set.total(), 25);
    }

    #[test]
    fn insertion_is_order_independent() {
        let mut a = RangeSet::new();
        for range in [
            ByteRange::new(40, 10),
            ByteRange::new(0, 10),
            ByteRange::new(20, 10),
        ] {
            a.insert(range).expect("insert");
        }
        let mut b = RangeSet::new();
        for range in [
            ByteRange::new(20, 10),
            ByteRange::new(0, 10),
            ByteRange::new(40, 10),
        ] {
            b.insert(range).expect("insert");
        }
        assert_eq!(a.as_slice(), b.as_slice());
        assert_eq!(
            a.as_slice(),
            &[
                ByteRange::new(0, 10),
                ByteRange::new(20, 10),
                ByteRange::new(40, 10)
            ]
        );
    }

    #[test]
    fn covers_is_exact_at_boundaries() {
        let mut set = RangeSet::new();
        set.insert(ByteRange::new(100, 50)).expect("insert");
        assert!(set.covers(100, 1));
        assert!(set.covers(149, 1));
        assert!(set.covers(100, 50));
        assert!(!set.covers(99, 1));
        assert!(!set.covers(150, 1));
        assert!(!set.covers(100, 51));
        assert!(set.covers(0, 0));
    }

    #[test]
    fn missing_enumerates_gaps() {
        let mut set = RangeSet::new();
        set.insert(ByteRange::new(10, 10)).expect("insert");
        set.insert(ByteRange::new(30, 10)).expect("insert");
        let mut gaps = [ByteRange::EMPTY; 8];
        let count = set.missing(0, 50, &mut gaps).expect("gaps");
        assert_eq!(
            gaps.get(..count).unwrap_or(&[]),
            &[
                ByteRange::new(0, 10),
                ByteRange::new(20, 10),
                ByteRange::new(40, 10)
            ]
        );
    }

    #[test]
    fn capacity_failure_leaves_set_unchanged() {
        let mut set = RangeSet::new();
        for index in 0..MAX_RANGES {
            set.insert(ByteRange::new((index as u64) * 10, 5))
                .expect("insert within capacity");
        }
        let before = set;
        assert_eq!(
            set.insert(ByteRange::new(10_000, 5)),
            Err(RangeError::Capacity)
        );
        assert_eq!(set, before);
    }

    #[test]
    fn overflow_is_rejected() {
        let mut set = RangeSet::new();
        assert_eq!(
            set.insert(ByteRange::new(u64::MAX, 1)),
            Err(RangeError::Overflow)
        );
        assert!(set.is_empty());
        assert!(!set.covers(u64::MAX, 2));
    }

    #[test]
    fn empty_ranges_are_noops() {
        let mut set = RangeSet::new();
        set.insert(ByteRange::EMPTY).expect("empty insert");
        assert!(set.is_empty());
        assert!(set.covers(123, 0));
        assert_eq!(set.missing(123, 0, &mut []), Ok(0));
    }

    #[test]
    fn output_too_small_is_reported() {
        let mut set = RangeSet::new();
        set.insert(ByteRange::new(10, 10)).expect("insert");
        let mut one = [ByteRange::EMPTY; 1];
        assert_eq!(
            set.missing(0, 40, &mut one),
            Err(RangeError::OutputTooSmall)
        );
    }
}
