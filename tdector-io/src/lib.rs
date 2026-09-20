//! Native UTF-8 inputs, atomic file replacement, and bounded project bindings.
//!
//! Checked bindings detect observed conflicts under a single-writer contract. Comparisons followed by replacement are not interprocess compare-and-swap, and path checks are not a sandbox against hostile concurrent path changes.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::Utf8Error;

use tempfile::NamedTempFile;

/// Typed failures let the adapter distinguish persistence conflicts from I/O.
#[derive(Debug)]
pub enum IoError {
    Io(io::Error),
    InvalidUtf8(Utf8Error),
    OutputExists {
        path: PathBuf,
    },
    InputChanged {
        path: PathBuf,
    },
    InvalidPath {
        path: PathBuf,
        reason: &'static str,
    },
    LimitExceeded {
        limit: usize,
    },
    /// Replacement succeeded, but its identity could not be recovered safely. Callers must stop writes and must not retry this operation automatically.
    CommittedState {
        source: io::Error,
    },
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::InvalidUtf8(error) => write!(f, "Input must be UTF-8: {error}"),
            Self::OutputExists { path } => write!(
                f,
                "Output already exists: {}; use --overwrite to replace it",
                path.display()
            ),
            Self::InputChanged { path } => {
                write!(f, "Input changed since it was loaded: {}", path.display())
            }
            Self::InvalidPath { path, reason } => {
                write!(f, "Invalid project path {}: {reason}", path.display())
            }
            Self::LimitExceeded { limit } => write!(f, "Project exceeds the {limit}-byte limit"),
            Self::CommittedState { source } => {
                write!(
                    f,
                    "File replacement committed, but identity refresh failed: {source}"
                )
            }
        }
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidUtf8(error) => Some(error),
            Self::CommittedState { source } => Some(source),
            Self::OutputExists { .. }
            | Self::InputChanged { .. }
            | Self::InvalidPath { .. }
            | Self::LimitExceeded { .. } => None,
        }
    }
}

impl From<io::Error> for IoError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Read a native file without transforming the original bytes.
pub fn read_file(path: &Path) -> Result<Vec<u8>, IoError> {
    Ok(fs::read(path)?)
}

/// Decode UTF-8, accepting exactly one optional leading UTF-8 BOM.
///
/// All other characters, including trailing newlines and a second BOM, remain.
pub fn decode_utf8(bytes: &[u8]) -> Result<&str, IoError> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(IoError::InvalidUtf8)
}

/// Compare existing file identities, including hard links and symlink aliases.
///
/// A missing output cannot alias the existing input. Other errors are preserved instead of treating an unreadable destination as a safely distinct file.
pub fn same_file(input: &Path, output: &Path) -> Result<bool, IoError> {
    let input = same_file::Handle::from_path(input)?;
    match same_file::Handle::from_path(output) {
        Ok(output) => Ok(input == output),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Stage complete contents beside the destination and commit them atomically.
///
/// Existing symlinks are followed when overwriting, so an in-place edit retains the link and replaces its target. Unix file permissions are preserved. The original destination is never explicitly removed. `tempfile::persist` uses a platform replacement operation (rename on Unix, `MoveFileExW` on Windows); a failed commit drops only the temporary file.
///
/// For an in-place edit, `expected_input` supplies the originally loaded bytes. They are compared after staging, immediately before committing. This detects observed changes but does not lock out a concurrent writer after the check.
pub fn atomic_write(
    output: &Path,
    bytes: &[u8],
    overwrite: bool,
    expected_input: Option<(&Path, &[u8])>,
) -> Result<(), IoError> {
    let metadata = match fs::symlink_metadata(output) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if metadata.is_some() && !overwrite {
        return Err(IoError::OutputExists {
            path: output.to_owned(),
        });
    }

    // Canonicalize an existing target to place the staging file on the target's filesystem, including when the final path component is a symlink.
    let destination = if metadata.is_some() {
        fs::canonicalize(output)?
    } else {
        output.to_owned()
    };
    let directory = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut staged = NamedTempFile::new_in(directory)?;
    staged.write_all(bytes)?;
    staged.flush()?;
    // On Windows, tempfile's replacement restores normal file attributes. Do not make the stage read-only before committing: that would prevent its cleanup when the subsequent input comparison refuses the write.
    #[cfg(unix)]
    if metadata.is_some() {
        staged
            .as_file()
            .set_permissions(fs::metadata(&destination)?.permissions())?;
    }
    staged.as_file().sync_all()?;

    if let Some((input, expected)) = expected_input {
        match fs::File::open(input) {
            Ok(observed) => {
                if !bound::matches_bytes(observed, expected)? {
                    return Err(IoError::InputChanged {
                        path: input.to_owned(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IoError::InputChanged {
                    path: input.to_owned(),
                });
            }
            Err(error) => return Err(error.into()),
        }
    }

    let committed = if overwrite {
        staged.persist(&destination)
    } else {
        // Unlike an existence check followed by rename, this also refuses a destination created by another process after the initial check.
        staged.persist_noclobber(&destination)
    };
    match committed {
        Ok(_) => Ok(()),
        Err(error) if !overwrite && error.error.kind() == io::ErrorKind::AlreadyExists => {
            Err(IoError::OutputExists {
                path: output.to_owned(),
            })
        }
        Err(error) => Err(IoError::Io(error.error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_preserves_whitespace_and_strips_only_one_bom() {
        assert_eq!(
            decode_utf8(b"\xef\xbb\xbf\xe7\x8c\xab \r\n").expect("decode valid UTF-8 fixture"),
            "猫 \r\n"
        );
        assert_eq!(
            decode_utf8(b"\xef\xbb\xbf\xef\xbb\xbftext\n").expect("decode valid UTF-8 fixture"),
            "\u{feff}text\n"
        );
        assert!(matches!(decode_utf8(b"\xff"), Err(IoError::InvalidUtf8(_))));
    }

    #[test]
    fn file_reads_retain_the_loaded_bytes() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let input = directory.path().join("Unicode 猫.txt");
        let expected = b"\xef\xbb\xbfhello\r\n";
        fs::write(&input, expected).expect("write fixture bytes");
        assert_eq!(
            read_file(&input).expect("read original input bytes"),
            expected
        );
    }

    #[test]
    fn detects_hard_links_and_normalized_path_aliases() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let input = directory.path().join("input.json");
        let alias = directory.path().join("alias.json");
        fs::write(&input, b"original").expect("write fixture bytes");
        fs::hard_link(&input, &alias).expect("create hard-link alias");
        assert!(same_file(&input, &alias).expect("compare file identities"));
        assert!(
            same_file(&input, &directory.path().join(".").join("input.json"))
                .expect("compare file identities")
        );
        assert!(
            !same_file(&input, &directory.path().join("new.json"))
                .expect("compare file identities")
        );
        fs::write(directory.path().join("other.json"), b"original").expect("write fixture bytes");
        assert!(
            !same_file(&input, &directory.path().join("other.json"))
                .expect("compare file identities")
        );
    }

    #[test]
    fn existing_output_requires_overwrite_and_is_unchanged_on_refusal() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let output = directory.path().join("project.json");
        fs::write(&output, b"original").expect("write fixture bytes");
        assert!(matches!(
            atomic_write(&output, b"replacement", false, None),
            Err(IoError::OutputExists { .. })
        ));
        assert_eq!(
            fs::read(&output).expect("read destination bytes"),
            b"original"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect staging directory cleanup")
                .count(),
            1
        );
    }

    #[test]
    fn commits_a_new_file_and_replaces_an_existing_file() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let output = directory.path().join("project.json");
        atomic_write(&output, b"first", false, None).expect("commit staged fixture");
        atomic_write(&output, b"second", true, Some((&output, b"first")))
            .expect("commit staged fixture");
        assert_eq!(
            fs::read(&output).expect("read destination bytes"),
            b"second"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect staging directory cleanup")
                .count(),
            1
        );
    }

    #[test]
    fn observed_input_changes_abort_after_staging_without_touching_destination() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let output = directory.path().join("project.json");
        fs::write(&output, b"another writer").expect("write fixture bytes");
        assert!(matches!(
            atomic_write(&output, b"replacement", true, Some((&output, b"original"))),
            Err(IoError::InputChanged { .. })
        ));
        assert_eq!(
            fs::read(&output).expect("read destination bytes"),
            b"another writer"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect staging directory cleanup")
                .count(),
            1
        );
    }

    #[test]
    fn a_removed_input_is_an_observed_change() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let output = directory.path().join("removed.json");
        assert!(matches!(
            atomic_write(&output, b"replacement", true, Some((&output, b"original"))),
            Err(IoError::InputChanged { .. })
        ));
        assert!(!output.exists());
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect staging directory cleanup")
                .count(),
            0
        );
    }

    #[test]
    fn failed_commit_leaves_a_destination_directory_untouched() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let output = directory.path().join("destination");
        fs::create_dir(&output).expect("create destination directory");
        fs::write(output.join("keep.json"), b"original").expect("write fixture bytes");
        assert!(matches!(
            atomic_write(&output, b"replacement", true, None),
            Err(IoError::Io(_))
        ));
        assert_eq!(
            fs::read(output.join("keep.json")).expect("read destination bytes"),
            b"original"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect staging directory cleanup")
                .count(),
            1
        );
    }

    #[cfg(windows)]
    #[test]
    fn failed_windows_replacement_preserves_the_existing_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = tempfile::tempdir().expect("create temporary test directory");
        let output = directory.path().join("project.json");
        fs::write(&output, b"original").expect("write fixture bytes");
        // Permit reads and writes, but deny deletion/replacement while open.
        let held_open = fs::OpenOptions::new()
            .read(true)
            .share_mode(0x0000_0001 | 0x0000_0002)
            .open(&output)
            .expect("hold destination open without delete sharing");
        assert!(matches!(
            atomic_write(&output, b"replacement", true, None),
            Err(IoError::Io(_))
        ));
        drop(held_open);
        assert_eq!(
            fs::read(&output).expect("read destination bytes"),
            b"original"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect staging directory cleanup")
                .count(),
            1
        );
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn detects_symlinks_and_preserves_the_link_on_in_place_save() {
        let directory = tempfile::tempdir().expect("create temporary test directory");
        let input = directory.path().join("project.json");
        let alias = directory.path().join("alias.json");
        fs::write(&input, b"original").expect("write fixture bytes");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&input, &alias).expect("create symbolic-link alias");
        #[cfg(windows)]
        if let Err(error) = std::os::windows::fs::symlink_file(&input, &alias) {
            if error.raw_os_error() == Some(1314) {
                eprintln!("Skipping symbolic-link test: Windows denied symlink creation privilege");
                return;
            }
            panic!("Could not create test symlink: {error}");
        }
        assert!(same_file(&input, &alias).expect("compare file identities"));
        atomic_write(&alias, b"replacement", true, Some((&alias, b"original")))
            .expect("commit staged fixture");
        assert!(
            fs::symlink_metadata(&alias)
                .expect("inspect retained symbolic link")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read(&input).expect("read destination bytes"),
            b"replacement"
        );
    }
}

mod bound;
pub use bound::{BoundProject, CheckedSaveError};
