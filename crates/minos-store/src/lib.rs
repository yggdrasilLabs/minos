//! Ledger and blob store traits with in-memory and file implementations.

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;

    use minos_core::id::{BlobId, ManifestId, RunId, TreeId};

    use super::{Appended, CorruptKind, StoreError};

    type TestResult = Result<(), Box<dyn Error>>;

    fn run() -> RunId {
        RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000)
    }

    fn json_error() -> Result<serde_json::Error, Box<dyn Error>> {
        Ok(serde_json::from_str::<u8>("x")
            .err()
            .ok_or("parsing `x` as a u8 unexpectedly succeeded")?)
    }

    #[test]
    fn appended_seq_returns_the_carried_seq() {
        assert_eq!(Appended::New { seq: 0 }.seq(), 0);
        assert_eq!(Appended::Duplicate { seq: 7 }.seq(), 7);
        assert_eq!(Appended::Conflict { seq: u64::MAX }.seq(), u64::MAX);
    }

    #[test]
    fn only_io_is_retryable() -> TestResult {
        let id = BlobId::of(b"blob");
        let io_error = StoreError::Io {
            op: "reading the ledger",
            source: io::Error::from(io::ErrorKind::Interrupted),
        };
        assert!(io_error.is_retryable());
        let others = [
            StoreError::Corrupt {
                run: run(),
                line: 1,
                kind: CorruptKind::BadJson,
            },
            StoreError::LedgerTooLarge {
                run: run(),
                bytes: 2,
                max: 1,
            },
            StoreError::Encode(json_error()?),
            StoreError::BlobMissing { id },
            StoreError::BlobTooLarge {
                id,
                size: 2,
                max: 1,
            },
            StoreError::BlobTampered { id },
            StoreError::Unsafe { what: "blob file" },
            StoreError::Poisoned,
        ];
        for error in &others {
            assert!(!error.is_retryable(), "{error} must not be retryable");
        }
        Ok(())
    }

    #[test]
    fn corrupt_display_names_the_run_line_and_kind() {
        let error = StoreError::Corrupt {
            run: run(),
            line: 3,
            kind: CorruptKind::SeqGap {
                expected: 2,
                found: 5,
            },
        };
        let text = error.to_string();
        assert!(text.contains(&run().to_hex()), "{text}");
        assert!(text.contains("line 3"), "{text}");
        assert!(text.contains("expected seq 2, found 5"), "{text}");
    }

    #[test]
    fn blob_too_large_display_names_the_id_size_and_max() {
        let id = BlobId::of(b"blob");
        let text = StoreError::BlobTooLarge {
            id,
            size: 4096,
            max: 1024,
        }
        .to_string();
        assert!(text.contains(&id.to_hex()), "{text}");
        assert!(text.contains("4096"), "{text}");
        assert!(text.contains("1024"), "{text}");
    }

    #[test]
    fn unknown_version_display_says_to_upgrade() {
        let text = CorruptKind::UnknownVersion.to_string();
        assert!(text.contains("upgrade minos"), "{text}");
    }
}
