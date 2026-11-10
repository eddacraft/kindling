//! Output-path safety: refuse to overwrite the source database.
//!
//! `export` and `browse` write to a caller-chosen path after reading the
//! database. A destination that resolves to the database itself would
//! truncate the SQLite file, so every write is checked first.

use std::fs::OpenOptions;
use std::path::Path;

use crate::{CliError, CliResult};

/// Reject `output` when it refers to the same file as `db`.
///
/// Covers the direct path, a symlink to the database, and a hard-link alias.
/// An output path that does not exist yet is always allowed.
pub(crate) fn ensure_not_database(output: &Path, db: &Path) -> CliResult {
    if same_file(output, db) {
        return Err(CliError::Invalid(format!(
            "refusing to write to {}: it is the source database ({})",
            output.display(),
            db.display()
        )));
    }
    Ok(())
}

/// Whether `a` and `b` resolve to the same file.
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }

    // Canonical paths resolve symlinks and `..` components.
    if let (Ok(ca), Ok(cb)) = (a.canonicalize(), b.canonicalize()) {
        if ca == cb {
            return true;
        }
    }

    // `metadata` follows symlinks, so a symlink to the database and a hard
    // link to it both report the database's inode. Windows has no stable
    // inode API here; the canonical check above covers symlinks and junctions.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(ma), Ok(mb)) = (a.metadata(), b.metadata()) {
            if ma.dev() == mb.dev() && ma.ino() == mb.ino() {
                return true;
            }
        }
    }

    false
}

/// Open `output` for writing, refusing to write through to the database.
///
/// The file is opened first, without truncation, and the opened descriptor's
/// identity is compared with `db`. A symlink or hard link created after the
/// pre-check is therefore still rejected before any truncation, closing the
/// check-then-act race. The returned file is positioned at the start and
/// truncated only after the check.
pub(crate) fn open_output_verified(output: &Path, db: &Path) -> Result<std::fs::File, CliError> {
    // Deliberately no `truncate` here: the identity check must run before the
    // file is truncated, so an alias to the database is rejected first.
    #[allow(clippy::suspicious_open_options)]
    let file = OpenOptions::new().write(true).create(true).open(output)?;
    if same_file_handle(&file, db) {
        return Err(CliError::Invalid(format!(
            "refusing to write to {}: it is the source database ({})",
            output.display(),
            db.display()
        )));
    }
    // Truncate only regular files: `set_len` fails on `/dev/null`, FIFOs, and
    // other special files that `std::fs::write` accepts. A newly opened
    // non-append file is already positioned at offset 0.
    if file.metadata()?.is_file() {
        file.set_len(0)?;
    }
    Ok(file)
}

/// Whether the open file and `db` are the same file.
///
/// On Unix this compares `dev`/`ino` of the open descriptor and the database,
/// which catches both symlink and hard-link aliases. Windows has no stable
/// inode API here; the canonical-path pre-check covers symlinks and junctions.
fn same_file_handle(file: &std::fs::File, db: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (file.metadata(), db.metadata()) {
            (Ok(fm), Ok(dm)) => fm.dev() == dm.dev() && fm.ino() == dm.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (file, db);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_not_database, open_output_verified, same_file};

    #[test]
    fn identical_paths_are_the_same_file() {
        let p = std::path::Path::new("/tmp/kindling.db");
        assert!(same_file(p, p));
        assert!(ensure_not_database(p, p).is_err());
    }

    #[test]
    fn distinct_paths_are_allowed() {
        let a = std::path::Path::new("/tmp/out.json");
        let b = std::path::Path::new("/tmp/kindling.db");
        assert!(!same_file(a, b));
        assert!(ensure_not_database(a, b).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_hard_link_alias_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kindling.db");
        std::fs::write(&db, b"db").unwrap();

        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&db, &link).unwrap();
        assert!(same_file(&link, &db));
        assert!(ensure_not_database(&link, &db).is_err());

        let hard = dir.path().join("hard.json");
        std::fs::hard_link(&db, &hard).unwrap();
        assert!(same_file(&hard, &db));
        assert!(ensure_not_database(&hard, &db).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn missing_output_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kindling.db");
        std::fs::write(&db, b"db").unwrap();
        let missing = dir.path().join("missing.json");
        assert!(ensure_not_database(&missing, &db).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn open_output_verified_rejects_symlink_and_hard_link() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kindling.db");
        std::fs::write(&db, b"db").unwrap();

        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&db, &link).unwrap();
        assert!(open_output_verified(&link, &db).is_err());
        assert_eq!(std::fs::read(&db).unwrap(), b"db");

        let hard = dir.path().join("hard.json");
        std::fs::hard_link(&db, &hard).unwrap();
        assert!(open_output_verified(&hard, &db).is_err());
        assert_eq!(std::fs::read(&db).unwrap(), b"db");
    }

    #[test]
    fn open_output_verified_allows_a_normal_file() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kindling.db");
        std::fs::write(&db, b"db").unwrap();
        let out = dir.path().join("out.json");

        let mut file = open_output_verified(&out, &db).unwrap();
        file.write_all(b"{}").unwrap();
        drop(file);

        assert_eq!(std::fs::read(&out).unwrap(), b"{}");
        assert_eq!(std::fs::read(&db).unwrap(), b"db");
    }

    #[cfg(unix)]
    #[test]
    fn open_output_verified_allows_special_files() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kindling.db");
        std::fs::write(&db, b"db").unwrap();

        let mut file = open_output_verified(std::path::Path::new("/dev/null"), &db).unwrap();
        file.write_all(b"{}").unwrap();
    }
}
