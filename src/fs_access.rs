//! Whether the current process can write a regular file, and the text
//! shown when it cannot.
//!
//! `Permissions::readonly` only checks the owner write bit. A root-owned
//! mode 644 file has that bit set and is still not writable by a normal
//! user. Open the file for write, without truncating, and let the kernel answer.

use std::io::ErrorKind;
use std::path::Path;

/// `true` when `path` is a regular file and this process can open it for
/// writing. Does not create or truncate. Missing paths, directories, and
/// permission errors are `false`.
pub fn can_write_regular_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    // write(true) without create/truncate asks for O_WRONLY. Success means
    // this uid can write. The fd is closed immediately.
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

/// Command that launches this exact binary as root on `path`.
/// `current_exe` covers both `~/.local/bin/rust-edit` and a `cargo run` build.
/// A PATH-only `sudo rust-edit` is not used here: sudo's secure_path does
/// not include `~/.local/bin` unless the installer symlink exists.
/// Quote a token for a POSIX shell when it is not a plain path.
fn shell_quote(text: &str) -> String {
    let plain = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | '@'));
    if plain {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

pub fn elevated_launch_line(path: &Path) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "rust-edit".into());
    format!(
        "sudo {} {}",
        shell_quote(&exe),
        shell_quote(&path.display().to_string())
    )
}

pub fn read_only_message(path: &Path) -> String {
    format!(
        "Opened {} read-only.\n\n\
         You can edit this tab, but you do not have permission to save it.\n\
         The modified mark stays until the file is written.\n\n\
         Save a copy with Ctrl+Alt+S.\n\
         To change the original file, quit and run the command below.\n\
         That opens the copy on disk, not this tab.\n\n  {}\n\n\
         Esc or a click closes this notice.",
        path.display(),
        elevated_launch_line(path)
    )
}

pub fn save_error_message(path: &Path, err: &std::io::Error) -> String {
    if err.kind() == ErrorKind::PermissionDenied {
        format!(
            "Cannot save {}.\n\n\
             {err}.\n\n\
             Your edits are still in this tab.\n\
             Save a copy with Ctrl+Alt+S.\n\
             Quitting and running the command below opens the file on disk, not this tab.\n\n  {}",
            path.display(),
            elevated_launch_line(path)
        )
    } else {
        format!(
            "Cannot save {}.\n\n{err}.\n\nYour edits are still in this tab.",
            path.display()
        )
    }
}

pub fn open_denied_message(path: &Path, err: &std::io::Error) -> String {
    if err.kind() == ErrorKind::PermissionDenied {
        format!(
            "Cannot open {}.\n\n{err}.\n\nQuit and run:\n\n  {}",
            path.display(),
            elevated_launch_line(path)
        )
    } else {
        format!("Cannot open {}.\n\n{err}.", path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(label: &str) -> std::path::PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rust-edit-access-{label}-{n}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Root ignores mode bits, so a 0444 file would still be writable.
    fn skip_if_root() -> bool {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find(|line| line.starts_with("Uid:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|n| n.parse::<u32>().ok())
            })
            == Some(0)
    }

    #[test]
    fn writable_regular_file_is_true() {
        let dir = scratch("writable");
        let path = dir.join("notes.txt");
        fs::write(&path, "hi\n").unwrap();
        assert!(can_write_regular_file(&path));
        assert_eq!(fs::read_to_string(&path).unwrap(), "hi\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mode_444_regular_file_is_false_and_unchanged() {
        if skip_if_root() {
            return;
        }
        let dir = scratch("locked");
        let path = dir.join("locked.txt");
        fs::write(&path, "secret\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&path, perms).unwrap();
        assert!(!can_write_regular_file(&path));
        assert_eq!(fs::read_to_string(&path).unwrap(), "secret\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_path_and_directory_are_false() {
        let dir = scratch("missing");
        assert!(!can_write_regular_file(&dir.join("nope.txt")));
        assert!(!can_write_regular_file(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn messages_name_the_path_and_the_sudo_command() {
        let path = Path::new("/etc/hosts");
        let denied = std::io::Error::from(ErrorKind::PermissionDenied);
        let read_only = read_only_message(path);
        let save = save_error_message(path, &denied);
        let open = open_denied_message(path, &denied);
        for body in [&read_only, &save, &open] {
            assert!(body.contains("/etc/hosts"), "{body}");
            assert!(body.contains("sudo "), "{body}");
        }
        assert!(read_only.contains("read-only") || read_only.contains("Read-only"));
        assert!(save.contains("Permission denied") || save.contains("permission"));
        assert!(save.to_lowercase().contains("still"));
        let other = save_error_message(path, &std::io::Error::from(ErrorKind::StorageFull));
        assert!(!other.contains("sudo "), "{other}");
        assert!(other.contains("/etc/hosts"));
    }

    #[test]
    fn shell_quote_wraps_paths_with_spaces() {
        assert_eq!(
            super::shell_quote("/etc/my files/hosts"),
            "'/etc/my files/hosts'"
        );
        assert_eq!(super::shell_quote("/etc/hosts"), "/etc/hosts");
    }
}
