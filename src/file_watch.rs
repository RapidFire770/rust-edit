//! Disk identity for open files. Compare last-accepted stamp vs live metadata.

use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub mtime: SystemTime,
    pub len: u64,
}

impl FileStamp {
    pub fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        Some(Self {
            mtime: meta.modified().ok()?,
            len: meta.len(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskEvent {
    Unchanged,
    Modified,
    Deleted,
    Appeared,
}

pub fn classify(last: Option<FileStamp>, now: Option<FileStamp>) -> DiskEvent {
    match (last, now) {
        (Some(a), Some(b)) if a != b => DiskEvent::Modified,
        (Some(_), None) => DiskEvent::Deleted,
        (None, Some(_)) => DiskEvent::Appeared,
        _ => DiskEvent::Unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn stamp(len: u64, secs: u64) -> FileStamp {
        FileStamp {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
            len,
        }
    }

    #[test]
    fn classify_unchanged_when_both_none() {
        assert_eq!(classify(None, None), DiskEvent::Unchanged);
    }

    #[test]
    fn classify_unchanged_when_stamps_equal() {
        let s = stamp(10, 1);
        assert_eq!(classify(Some(s), Some(s)), DiskEvent::Unchanged);
    }

    #[test]
    fn classify_modified_on_len_or_mtime_change() {
        assert_eq!(
            classify(Some(stamp(10, 1)), Some(stamp(11, 1))),
            DiskEvent::Modified
        );
        assert_eq!(
            classify(Some(stamp(10, 1)), Some(stamp(10, 2))),
            DiskEvent::Modified
        );
    }

    #[test]
    fn classify_deleted_and_appeared() {
        assert_eq!(classify(Some(stamp(10, 1)), None), DiskEvent::Deleted);
        assert_eq!(classify(None, Some(stamp(10, 1))), DiskEvent::Appeared);
    }

    #[test]
    fn of_reads_real_file_then_none_after_delete() {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-stamp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, "abc").unwrap();
        let a = FileStamp::of(&p).expect("stamp");
        assert_eq!(a.len, 3);
        std::fs::write(&p, "abcd").unwrap();
        let b = FileStamp::of(&p).expect("stamp after write");
        assert_eq!(classify(Some(a), Some(b)), DiskEvent::Modified);
        std::fs::remove_file(&p).unwrap();
        assert_eq!(FileStamp::of(&p), None);
    }
}
