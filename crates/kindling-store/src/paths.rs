//! Per-project database path resolution.
//!
//! Each project gets its own database under `~/.kindling/projects/<hash>/`,
//! where `<hash>` is the first 12 hex characters of the SHA-256 of the
//! project root path. This mirrors `getDbPath` in the Claude Code plugin
//! (`plugins/kindling-claude-code/hooks/lib/init.js`) — both implementations
//! MUST derive the same hash for the same project root so they share a DB.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Stable 12-hex-char project identifier derived from the project root path.
pub fn project_id(project_root: &str) -> String {
    let digest = Sha256::digest(project_root.as_bytes());
    let mut hex = String::with_capacity(12);
    for byte in digest.iter().take(6) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Database path for a project under the given kindling home directory:
/// `<kindling_home>/projects/<project_id>/kindling.db`.
pub fn project_db_path(kindling_home: &Path, project_root: &str) -> PathBuf {
    kindling_home
        .join("projects")
        .join(project_id(project_root))
        .join("kindling.db")
}

/// Default kindling home (`~/.kindling`), or `None` if no home directory can
/// be determined.
pub fn default_kindling_home() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))?;
    Some(PathBuf::from(home).join(".kindling"))
}

/// Resolve the database path for a project root, honouring the
/// `KINDLING_DB_PATH` environment override used by the Claude Code plugin.
pub fn resolve_db_path(project_root: &str) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("KINDLING_DB_PATH").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    default_kindling_home().map(|home| project_db_path(&home, project_root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_matches_node_crypto() {
        // node -e "console.log(require('crypto').createHash('sha256')
        //   .update('/tmp/example').digest('hex').slice(0, 12))"
        assert_eq!(project_id("/tmp/example"), NODE_HASH_TMP_EXAMPLE);
    }

    #[test]
    fn project_id_is_stable_and_short() {
        let id = project_id("/home/user/project");
        assert_eq!(id.len(), 12);
        assert_eq!(id, project_id("/home/user/project"));
        assert_ne!(id, project_id("/home/user/other"));
    }

    #[test]
    fn db_path_layout() {
        let path = project_db_path(Path::new("/home/u/.kindling"), "/tmp/example");
        let expected = format!("/home/u/.kindling/projects/{NODE_HASH_TMP_EXAMPLE}/kindling.db");
        assert_eq!(path, PathBuf::from(expected));
    }

    /// Reference value produced by the Node.js implementation.
    const NODE_HASH_TMP_EXAMPLE: &str = "f33aa9244af5";
}
