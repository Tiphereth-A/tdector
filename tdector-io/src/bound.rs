//! Launch-bound persistence for a long-lived native project owner.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use same_file::Handle;
use tempfile::NamedTempFile;

use crate::{IoError, decode_utf8};

// Windows MoveFileEx can refuse to replace a file while a read/identity handle remains open, even when it shares deletion. Store the OS identity and creation time there; identity recycling during hostile concurrent changes is outside the documented single-writer contract. Other platforms retain their identity handle.
#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
struct FileIdentity {
    volume: u64,
    index: u64,
    created: u64,
}

#[cfg(not(windows))]
#[derive(Debug, PartialEq, Eq)]
struct FileIdentity(Handle);

impl FileIdentity {
    fn from_path(path: &Path) -> io::Result<Self> {
        Self::from_file(File::open(path)?)
    }

    fn from_file(file: File) -> io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let info = winapi_util::file::information(&file)?;
            Ok(Self {
                volume: info.volume_serial_number(),
                index: info.file_index(),
                created: file.metadata()?.creation_time(),
            })
        }
        #[cfg(not(windows))]
        {
            Handle::from_file(file).map(Self)
        }
    }
}

/// An error before replacement, or an I/O error (which may explicitly indicate a committed-state failure). A rejected checkpoint always leaves disk intact.
#[derive(Debug)]
pub enum CheckedSaveError<E> {
    Io(IoError),
    Checkpoint(E),
}

impl<E: fmt::Display> fmt::Display for CheckedSaveError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Checkpoint(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CheckedSaveError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Checkpoint(error) => Some(error),
        }
    }
}

impl<E> From<IoError> for CheckedSaveError<E> {
    fn from(error: IoError) -> Self {
        Self::Io(error)
    }
}

impl<E> From<io::Error> for CheckedSaveError<E> {
    fn from(error: io::Error) -> Self {
        Self::Io(error.into())
    }
}

/// An existing regular file pinned to its launch-time canonical pathname.
///
/// Unix retains an identity handle; Windows records volume, file index, and creation time so an open handle does not obstruct external atomic replacement. An initial symbolic link may name the project, but later redirection is refused. Atomic saves refresh identity and leave other hard links untouched. The exact loaded bytes (including a BOM) are retained for conflict detection.
#[derive(Debug)]
pub struct BoundProject {
    configured: PathBuf,
    canonical: PathBuf,
    configured_is_symlink: bool,
    identity: FileIdentity,
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl BoundProject {
    /// Resolve a path once against the current launch directory and read at most `max_bytes + 1` bytes. This never interprets `-` as stdin or accepts URLs.
    pub fn open(path: &Path, max_bytes: usize) -> Result<Self, IoError> {
        validate_path(path)?;
        let configured = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        let metadata = fs::symlink_metadata(&configured)?;
        let configured_is_symlink = metadata.file_type().is_symlink();
        let canonical = fs::canonicalize(&configured)?;
        if !fs::symlink_metadata(&canonical)?.file_type().is_file() {
            return Err(IoError::InvalidPath {
                path: configured,
                reason: "project must be an existing regular file",
            });
        }
        let (identity, bytes) = read_regular(&canonical, max_bytes)?;
        let result = Self {
            configured,
            canonical,
            configured_is_symlink,
            identity,
            bytes,
            max_bytes,
        };
        result.check_target()?;
        result.check_identity()?;
        Ok(result)
    }

    /// The pinned canonical destination; tools should not accept replacement paths.
    pub fn path(&self) -> &Path {
        &self.canonical
    }

    /// Original or last committed bytes, without UTF-8/BOM transformation.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Decode the current baseline, allowing exactly one leading UTF-8 BOM.
    pub fn text(&self) -> Result<&str, IoError> {
        decode_utf8(&self.bytes)
    }

    /// Validate both pathname/identity and exact bytes. Memory use is bounded by a small streaming buffer, and at most baseline length plus one byte is read.
    pub fn check_unchanged(&self) -> Result<(), IoError> {
        self.check_target()?;
        let file = File::open(&self.canonical).map_err(|error| self.observed_error(error))?;
        if !file.metadata()?.is_file() {
            return Err(self.changed());
        }
        let identity = FileIdentity::from_file(file.try_clone()?)?;
        if identity != self.identity || !matches_bytes(file, &self.bytes)? {
            return Err(self.changed());
        }
        self.check_target()?;
        self.check_identity()
    }

    /// Read a reload candidate without changing this binding. A different file identity at the same pinned pathname is allowed here. Decode and validate the candidate in application staging, then replace this binding only after the application load commits. Dropping a candidate retains the old baseline.
    pub fn read_reload(&self) -> Result<Self, IoError> {
        self.check_target()?;
        let (identity, bytes) = read_regular(&self.canonical, self.max_bytes)?;
        let candidate = Self {
            configured: self.configured.clone(),
            canonical: self.canonical.clone(),
            configured_is_symlink: self.configured_is_symlink,
            identity,
            bytes,
            max_bytes: self.max_bytes,
        };
        candidate.check_target()?;
        candidate.check_identity()?;
        Ok(candidate)
    }

    /// Stage bytes, check the loaded baseline again, and invoke a cancellation or deadline checkpoint immediately before replacement. The checkpoint must only validate whether to proceed; it must not modify the bound file.
    ///
    /// After replacement starts, finish bookkeeping without further cancellation checks. The caller acknowledges its application save token only on success. A `CommittedState` failure means replacement succeeded and requires stopping further edits; other errors preserve the previous baseline and destination.
    pub fn save_checked<E>(
        &mut self,
        bytes: &[u8],
        before_commit: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), CheckedSaveError<E>> {
        self.check_unchanged()?;
        if bytes.len() > self.max_bytes {
            return Err(IoError::LimitExceeded {
                limit: self.max_bytes,
            }
            .into());
        }
        // Allocate the next baseline before the irreversible replacement.
        let next_bytes = bytes.to_vec();
        let directory = self
            .canonical
            .parent()
            .ok_or_else(|| IoError::InvalidPath {
                path: self.canonical.clone(),
                reason: "project has no parent directory",
            })?;
        let mut staged = NamedTempFile::new_in(directory)?;
        staged.write_all(bytes)?;
        staged.flush()?;
        #[cfg(unix)]
        staged
            .as_file()
            .set_permissions(fs::metadata(&self.canonical)?.permissions())?;
        staged.as_file().sync_all()?;
        self.check_unchanged()?;
        before_commit().map_err(CheckedSaveError::Checkpoint)?;
        let committed = staged
            .persist(&self.canonical)
            .map_err(|error| IoError::Io(error.error))?;
        // Capture the actual committed identity and compare it to the current pathname before accepting the new baseline. Windows captures values without retaining an open handle that could obstruct the next rename.
        let committed_identity = FileIdentity::from_file(committed)
            .map_err(|source| IoError::CommittedState { source })?;
        let identity = FileIdentity::from_path(&self.canonical)
            .map_err(|source| IoError::CommittedState { source })?;
        if identity != committed_identity {
            return Err(IoError::CommittedState {
                source: io::Error::other(
                    "project was replaced again while refreshing saved identity",
                ),
            }
            .into());
        }
        self.identity = identity;
        self.bytes = next_bytes;
        Ok(())
    }

    fn changed(&self) -> IoError {
        IoError::InputChanged {
            path: self.canonical.clone(),
        }
    }

    fn observed_error(&self, error: io::Error) -> IoError {
        if error.kind() == io::ErrorKind::NotFound {
            self.changed()
        } else {
            error.into()
        }
    }

    fn check_identity(&self) -> Result<(), IoError> {
        let identity =
            FileIdentity::from_path(&self.canonical).map_err(|error| self.observed_error(error))?;
        if identity != self.identity {
            return Err(self.changed());
        }
        Ok(())
    }

    fn check_target(&self) -> Result<(), IoError> {
        let metadata =
            fs::symlink_metadata(&self.configured).map_err(|error| self.observed_error(error))?;
        if metadata.file_type().is_symlink() != self.configured_is_symlink
            || fs::canonicalize(&self.configured).map_err(|error| self.observed_error(error))?
                != self.canonical
            || !fs::symlink_metadata(&self.canonical)
                .map_err(|error| self.observed_error(error))?
                .file_type()
                .is_file()
            || fs::canonicalize(&self.canonical).map_err(|error| self.observed_error(error))?
                != self.canonical
        {
            return Err(self.changed());
        }
        Ok(())
    }
}

fn validate_path(path: &Path) -> Result<(), IoError> {
    let text = path.as_os_str().to_string_lossy();
    let is_url = text.contains("://")
        || text.split_once(':').is_some_and(|(scheme, _)| {
            // A single alphabetic character is a Windows drive designator.
            scheme.len() > 1
                && scheme.as_bytes()[0].is_ascii_alphabetic()
                && scheme
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"+.-".contains(&byte))
        });
    if text.is_empty() || path == Path::new("-") || is_url {
        return Err(IoError::InvalidPath {
            path: path.to_owned(),
            reason: "expected a filesystem path, not stdin or a URL",
        });
    }
    Ok(())
}

fn read_regular(path: &Path, limit: usize) -> Result<(FileIdentity, Vec<u8>), IoError> {
    // Check before opening so a named pipe or device is not consumed as a file.
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(IoError::InvalidPath {
            path: path.to_owned(),
            reason: "project must be a regular file",
        });
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(IoError::InvalidPath {
            path: path.to_owned(),
            reason: "project must be a regular file",
        });
    }
    let read_identity = FileIdentity::from_file(file.try_clone()?)?;
    let bytes = read_bounded(file, limit)?;
    // Verify that the pathname still names the file we read. Windows identity capture closes handles before returning so later replacement is possible.
    let identity = FileIdentity::from_path(path)?;
    if identity != read_identity {
        return Err(IoError::InputChanged {
            path: path.to_owned(),
        });
    }
    Ok((identity, bytes))
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, IoError> {
    let mut bytes = Vec::new();
    reader
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(IoError::LimitExceeded { limit });
    }
    Ok(bytes)
}

pub(crate) fn matches_bytes(reader: impl Read, expected: &[u8]) -> io::Result<bool> {
    let mut reader = reader.take((expected.len() as u64).saturating_add(1));
    let mut buffer = [0_u8; 8192];
    let mut offset = 0;
    loop {
        let count = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(offset == expected.len());
        }
        let Some(chunk) = expected.get(offset..offset + count) else {
            return Ok(false);
        };
        if chunk != &buffer[..count] {
            return Ok(false);
        }
        offset += count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn fixture(bytes: &[u8], limit: usize) -> (tempfile::TempDir, BoundProject) {
        let directory = tempfile::tempdir().expect("create test directory");
        let path = directory.path().join("project.json");
        fs::write(&path, bytes).expect("create bound fixture");
        let bound = BoundProject::open(&path, limit).expect("bind fixture");
        (directory, bound)
    }

    #[test]
    fn bounds_read_consumption_before_allocating_the_input() {
        struct Counted<'a>(&'a Cell<usize>);
        impl Read for Counted<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                buffer.fill(b'x');
                self.0.set(self.0.get() + buffer.len());
                Ok(buffer.len())
            }
        }
        let consumed = Cell::new(0);
        assert!(matches!(
            read_bounded(Counted(&consumed), 10),
            Err(IoError::LimitExceeded { limit: 10 })
        ));
        assert_eq!(consumed.get(), 11);
        consumed.set(0);
        assert!(!matches_bytes(Counted(&consumed), b"xxxxx").expect("bounded comparison"));
        assert_eq!(consumed.get(), 6);
        assert_eq!(
            read_bounded(&b"abc"[..], 3).expect("read exact limit"),
            b"abc"
        );
        assert!(
            read_bounded(&b""[..], 0)
                .expect("read empty file")
                .is_empty()
        );
    }

    #[test]
    fn rejects_stdin_urls_directories_and_oversized_inputs() {
        for path in [
            "-",
            "",
            "https://example.org/project.json",
            "file:///project.json",
        ] {
            assert!(matches!(
                BoundProject::open(Path::new(path), 10),
                Err(IoError::InvalidPath { .. })
            ));
        }
        let directory = tempfile::tempdir().expect("create test directory");
        assert!(matches!(
            BoundProject::open(directory.path(), 10),
            Err(IoError::InvalidPath { .. })
        ));
        let path = directory.path().join("big.json");
        fs::write(&path, b"12345").expect("write oversized input");
        assert!(matches!(
            BoundProject::open(&path, 4),
            Err(IoError::LimitExceeded { limit: 4 })
        ));
    }

    #[test]
    fn bom_bytes_remain_in_the_baseline_while_decoding_strips_one() {
        let (_directory, bound) = fixture(b"\xef\xbb\xbf{}\n", 100);
        assert_eq!(bound.text().expect("decode BOM"), "{}\n");
        assert_eq!(bound.bytes(), b"\xef\xbb\xbf{}\n");
        fs::write(bound.path(), b"{}\n").expect("remove BOM externally");
        assert!(matches!(
            bound.check_unchanged(),
            Err(IoError::InputChanged { .. })
        ));
    }

    #[test]
    fn repeated_saves_refresh_exact_bytes_and_identity() {
        let (directory, mut bound) = fixture(b"first", 100);
        let original_identity =
            FileIdentity::from_path(bound.path()).expect("capture initial identity");
        bound
            .save_checked(b"second", || Ok::<_, ()>(()))
            .expect("first save");
        assert_ne!(original_identity, bound.identity);
        assert_eq!(bound.bytes(), b"second");
        bound.check_unchanged().expect("refreshed baseline matches");
        let second_identity =
            FileIdentity::from_path(bound.path()).expect("capture second identity");
        bound
            .save_checked(b"third", || Ok::<_, ()>(()))
            .expect("second save");
        assert_ne!(second_identity, bound.identity);
        bound
            .check_unchanged()
            .expect("second refreshed baseline matches");
        assert_eq!(fs::read(bound.path()).expect("read saved bytes"), b"third");
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect cleanup")
                .count(),
            1
        );
    }

    #[test]
    fn unchanged_check_preserves_timestamp_and_identity() {
        let (_directory, bound) = fixture(b"original", 100);
        let before = fs::metadata(bound.path())
            .expect("read timestamp")
            .modified()
            .expect("timestamp");
        let identity = FileIdentity::from_path(bound.path()).expect("capture identity");
        bound.check_unchanged().expect("check clean baseline");
        assert_eq!(
            before,
            fs::metadata(bound.path())
                .expect("read timestamp")
                .modified()
                .expect("timestamp")
        );
        assert_eq!(
            identity,
            FileIdentity::from_path(bound.path()).expect("current identity")
        );
    }

    #[test]
    fn cancellation_before_commit_cleans_staging_and_retains_baseline() {
        let (directory, mut bound) = fixture(b"original", 100);
        let error = bound.save_checked(b"candidate", || Err("cancelled"));
        assert!(matches!(
            error,
            Err(CheckedSaveError::Checkpoint("cancelled"))
        ));
        assert_eq!(bound.bytes(), b"original");
        bound
            .check_unchanged()
            .expect("cancelled save retains baseline");
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect cleanup")
                .count(),
            1
        );
        bound
            .save_checked(b"later", || Ok::<_, ()>(()))
            .expect("save after cancellation");
    }

    #[test]
    fn output_size_failure_retains_disk_baseline_and_skips_checkpoint() {
        let (directory, mut bound) = fixture(b"old", 4);
        let called = Cell::new(false);
        assert!(matches!(
            bound.save_checked(b"too large", || {
                called.set(true);
                Ok::<_, ()>(())
            }),
            Err(CheckedSaveError::Io(IoError::LimitExceeded { limit: 4 }))
        ));
        assert!(!called.get());
        bound
            .check_unchanged()
            .expect("oversized output retains baseline");
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect cleanup")
                .count(),
            1
        );
    }

    #[test]
    fn external_same_byte_replacement_conflicts_until_reload_is_accepted() {
        let (_directory, mut bound) = fixture(b"original", 100);
        crate::atomic_write(bound.path(), b"original", true, None)
            .expect("external atomic replacement");
        assert!(matches!(
            bound.save_checked(b"local edits", || Ok::<_, ()>(())),
            Err(CheckedSaveError::Io(IoError::InputChanged { .. }))
        ));
        let candidate = bound.read_reload().expect("read replacement candidate");
        assert_eq!(candidate.bytes(), b"original");
        assert!(matches!(
            bound.check_unchanged(),
            Err(IoError::InputChanged { .. })
        ));
        bound = candidate;
        bound.check_unchanged().expect("accept refreshed identity");
        bound
            .save_checked(b"local edits", || Ok::<_, ()>(()))
            .expect("save after reload");
    }

    #[test]
    fn external_edits_deletion_and_enlargement_do_not_replace_local_baseline() {
        let (_directory, mut bound) = fixture(b"original", 10);
        fs::write(bound.path(), b"external").expect("external write");
        assert!(matches!(
            bound.check_unchanged(),
            Err(IoError::InputChanged { .. })
        ));
        let candidate = bound.read_reload().expect("read changed candidate");
        assert_eq!(candidate.bytes(), b"external");
        drop(candidate);
        assert_eq!(bound.bytes(), b"original");
        fs::write(bound.path(), b"much larger than the permitted size")
            .expect("external enlargement");
        assert!(matches!(
            bound.read_reload(),
            Err(IoError::LimitExceeded { limit: 10 })
        ));
        assert!(matches!(
            bound.save_checked(b"local", || Ok::<_, ()>(())),
            Err(CheckedSaveError::Io(IoError::InputChanged { .. }))
        ));
        fs::remove_file(bound.path()).expect("external removal");
        assert!(matches!(
            bound.check_unchanged(),
            Err(IoError::InputChanged { .. })
        ));
        assert!(matches!(
            bound.read_reload(),
            Err(IoError::InputChanged { .. })
        ));
        assert_eq!(bound.bytes(), b"original");
        assert!(!bound.path().exists());
    }

    #[test]
    fn save_replaces_only_the_bound_hard_link() {
        let (directory, mut bound) = fixture(b"original", 100);
        let alias = directory.path().join("alias.json");
        fs::hard_link(bound.path(), &alias).expect("make hard link");
        bound
            .save_checked(b"saved", || Ok::<_, ()>(()))
            .expect("save bound name");
        assert_eq!(fs::read(alias).expect("read other hard link"), b"original");
        assert_eq!(fs::read(bound.path()).expect("read saved target"), b"saved");
    }

    #[cfg(windows)]
    #[test]
    fn failed_windows_bound_replacement_keeps_baseline_and_cleans_stage() {
        use std::os::windows::fs::OpenOptionsExt;
        let (directory, mut bound) = fixture(b"original", 100);
        let guard = fs::OpenOptions::new()
            .read(true)
            .share_mode(0x0000_0001 | 0x0000_0002)
            .open(bound.path())
            .expect("deny replacement sharing");
        assert!(matches!(
            bound.save_checked(b"replacement", || Ok::<_, ()>(())),
            Err(CheckedSaveError::Io(IoError::Io(_)))
        ));
        drop(guard);
        bound
            .check_unchanged()
            .expect("failed replacement keeps valid baseline");
        assert_eq!(bound.bytes(), b"original");
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect cleanup")
                .count(),
            1
        );
        bound
            .save_checked(b"retry", || Ok::<_, ()>(()))
            .expect("save after sharing lock released");
    }

    #[cfg(any(unix, windows))]
    fn symlink(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(target, link);
        match result {
            Ok(()) => true,
            #[cfg(windows)]
            Err(error) if error.raw_os_error() == Some(1314) => {
                eprintln!("Skipping symlink test: no Windows symlink privilege");
                false
            }
            Err(error) => panic!("create symlink: {error}"),
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn rejects_launch_alias_redirection_and_retains_both_targets() {
        let directory = tempfile::tempdir().expect("create test directory");
        let first = directory.path().join("first.json");
        let second = directory.path().join("second.json");
        let alias = directory.path().join("alias.json");
        fs::write(&first, b"first").expect("create first target");
        fs::write(&second, b"second").expect("create second target");
        if !symlink(&first, &alias) {
            return;
        }
        let mut bound = BoundProject::open(&alias, 100).expect("bind original alias");
        bound
            .save_checked(b"saved", || Ok::<_, ()>(()))
            .expect("save initial alias target");
        assert!(
            fs::symlink_metadata(&alias)
                .expect("inspect alias")
                .file_type()
                .is_symlink()
        );
        fs::remove_file(&alias).expect("remove original alias");
        assert!(symlink(&second, &alias));
        assert!(matches!(
            bound.read_reload(),
            Err(IoError::InputChanged { .. })
        ));
        assert!(matches!(
            bound.save_checked(b"wrong", || Ok::<_, ()>(())),
            Err(CheckedSaveError::Io(IoError::InputChanged { .. }))
        ));
        assert_eq!(fs::read(&first).expect("read first target"), b"saved");
        assert_eq!(fs::read(&second).expect("read second target"), b"second");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn rejects_replacing_the_pinned_file_with_a_symlink() {
        let (directory, bound) = fixture(b"original", 100);
        let other = directory.path().join("other.json");
        fs::write(&other, b"original").expect("create other file");
        // Probe privilege before removing the bound file on Windows.
        let probe = directory.path().join("probe");
        if !symlink(&other, &probe) {
            return;
        }
        fs::remove_file(probe).expect("remove probe");
        fs::remove_file(bound.path()).expect("remove original file");
        assert!(symlink(&other, bound.path()));
        assert!(matches!(
            bound.check_unchanged(),
            Err(IoError::InputChanged { .. })
        ));
        assert!(matches!(
            bound.read_reload(),
            Err(IoError::InputChanged { .. })
        ));
    }
    #[cfg(unix)]
    #[test]
    fn rejects_a_unix_socket_without_opening_it_as_project_input() {
        let directory = tempfile::tempdir().expect("create socket test directory");
        let path = directory.path().join("socket");
        let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bind test socket");
        assert!(matches!(
            BoundProject::open(&path, 100),
            Err(IoError::InvalidPath { .. })
        ));
    }
}
