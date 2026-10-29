//! Create/scan-suppression policy.
//!
//! KM-2 must never turn a metadata-only create (attribute query, backup
//! scanner, directory enumeration) into a data read. This module classifies a
//! create from the caller's `DesiredAccess` and `CreateOptions` so the same
//! decision can be unit-tested on the host before it is wired into the
//! FltMgr pre-create callback.
//!
//! `DATA_ACCESS` is the only intent that may later trigger a recall. A create
//! itself never hydrates: FltMgr create callbacks are not a data path.

pub const FILE_READ_DATA: u32 = 0x0000_0001;
pub const FILE_WRITE_DATA: u32 = 0x0000_0002;
pub const FILE_APPEND_DATA: u32 = 0x0000_0004;
pub const FILE_READ_EA: u32 = 0x0000_0008;
pub const FILE_WRITE_EA: u32 = 0x0000_0010;
pub const FILE_EXECUTE: u32 = 0x0000_0020;
pub const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
pub const FILE_WRITE_ATTRIBUTES: u32 = 0x0000_0100;
pub const DELETE: u32 = 0x0001_0000;
pub const READ_CONTROL: u32 = 0x0002_0000;
pub const WRITE_DAC: u32 = 0x0004_0000;
pub const WRITE_OWNER: u32 = 0x0008_0000;
pub const SYNCHRONIZE: u32 = 0x0010_0000;

pub const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
pub const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
pub const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

pub const DATA_ACCESS_MASK: u32 =
    FILE_READ_DATA | FILE_WRITE_DATA | FILE_APPEND_DATA | FILE_EXECUTE;

pub const METADATA_ACCESS_MASK: u32 = FILE_READ_EA
    | FILE_WRITE_EA
    | FILE_READ_ATTRIBUTES
    | FILE_WRITE_ATTRIBUTES
    | DELETE
    | READ_CONTROL
    | WRITE_DAC
    | WRITE_OWNER
    | SYNCHRONIZE;

/// Why the caller opened the file. `MetadataOnly` and `DirectoryEnumeration`
/// must pass through without any data I/O.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateIntent {
    MetadataOnly,
    DirectoryEnumeration,
    DataAccess,
}

/// Classify a create. Data access wins over the directory flag because a
/// directory can be opened for data access (rare, but it must not be treated
/// as metadata-only).
pub const fn classify_create(desired_access: u32, create_options: u32) -> CreateIntent {
    if desired_access & DATA_ACCESS_MASK != 0 {
        CreateIntent::DataAccess
    } else if create_options & FILE_DIRECTORY_FILE != 0 {
        CreateIntent::DirectoryEnumeration
    } else {
        CreateIntent::MetadataOnly
    }
}

/// Only a data-access intent may later trigger a recall.
pub const fn needs_data_recall(intent: CreateIntent) -> bool {
    matches!(intent, CreateIntent::DataAccess)
}

/// A create callback never hydrates, regardless of intent. Hydration is
/// triggered from READ or section synchronization, never from CREATE.
pub const fn should_hydrate_on_create(_intent: CreateIntent) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_access_bits_are_recognized() {
        for bit in [
            FILE_READ_DATA,
            FILE_WRITE_DATA,
            FILE_APPEND_DATA,
            FILE_EXECUTE,
        ] {
            assert_eq!(
                classify_create(bit, 0),
                CreateIntent::DataAccess,
                "0x{bit:08X}"
            );
        }
    }

    #[test]
    fn metadata_only_create_is_not_data_access() {
        for bits in [
            FILE_READ_ATTRIBUTES,
            FILE_WRITE_ATTRIBUTES,
            FILE_READ_EA,
            FILE_WRITE_EA,
            DELETE,
            READ_CONTROL,
            WRITE_DAC,
            WRITE_OWNER,
            SYNCHRONIZE,
            FILE_READ_ATTRIBUTES | SYNCHRONIZE,
        ] {
            let intent = classify_create(bits, 0);
            assert_eq!(intent, CreateIntent::MetadataOnly, "0x{bits:08X}");
            assert!(!needs_data_recall(intent));
            assert!(!should_hydrate_on_create(intent));
        }
    }

    #[test]
    fn zero_access_is_metadata_only() {
        assert_eq!(classify_create(0, 0), CreateIntent::MetadataOnly);
    }

    #[test]
    fn directory_create_is_enumeration_without_data_access() {
        let intent = classify_create(FILE_READ_ATTRIBUTES, FILE_DIRECTORY_FILE);
        assert_eq!(intent, CreateIntent::DirectoryEnumeration);
        assert!(!needs_data_recall(intent));
        assert!(!should_hydrate_on_create(intent));
    }

    #[test]
    fn data_access_wins_over_directory_flag() {
        let intent = classify_create(FILE_READ_DATA, FILE_DIRECTORY_FILE);
        assert_eq!(intent, CreateIntent::DataAccess);
        assert!(needs_data_recall(intent));
    }

    #[test]
    fn reparse_point_open_does_not_change_the_intent() {
        let intent = classify_create(FILE_READ_ATTRIBUTES, FILE_OPEN_REPARSE_POINT);
        assert_eq!(intent, CreateIntent::MetadataOnly);
        let data = classify_create(
            FILE_READ_DATA,
            FILE_OPEN_REPARSE_POINT | FILE_NON_DIRECTORY_FILE,
        );
        assert_eq!(data, CreateIntent::DataAccess);
    }

    #[test]
    fn create_never_hydrates() {
        for intent in [
            CreateIntent::MetadataOnly,
            CreateIntent::DirectoryEnumeration,
            CreateIntent::DataAccess,
        ] {
            assert!(!should_hydrate_on_create(intent));
        }
        assert!(needs_data_recall(CreateIntent::DataAccess));
        assert!(!needs_data_recall(CreateIntent::MetadataOnly));
        assert!(!needs_data_recall(CreateIntent::DirectoryEnumeration));
    }
}
