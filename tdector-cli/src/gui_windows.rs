//! Limit the detached host to its own standard streams on Windows.
//!
//! Stable Rust's Windows process launcher inherits every inheritable handle. A
//! shell or .NET caller may supply inheritable stdout/stderr pipes; leaving them
//! inherited keeps those callers waiting for EOF until the GUI closes. The stable
//! standard library has no handle-inheritance setter, so this small borrowed-
//! handle guard uses the documented Win32 API without taking handle ownership.

use std::io;
use std::process::{Child, Command};

use windows_sys::Win32::Foundation::{
    GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

pub(super) fn spawn(command: &mut Command) -> io::Result<Child> {
    // Called before the launcher starts any threads. The child gets freshly
    // created pipes/null streams from Command, unaffected by this guard.
    let mut guard = StandardHandleInheritance::clear()?;
    let spawned = command.spawn();
    let restored = guard.restore();
    match (spawned, restored) {
        (Ok(child), Ok(())) => Ok(child),
        (Err(error), _) => Err(error),
        (Ok(mut child), Err(error)) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
    }
}

struct StandardHandleInheritance {
    handles: Vec<(HANDLE, u32)>,
}
impl StandardHandleInheritance {
    fn clear() -> io::Result<Self> {
        let mut guard = Self {
            handles: Vec::new(),
        };
        for standard in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: standard is one of the three documented selector constants.
            // The returned handle is borrowed, checked below, and never closed.
            #[expect(
                unsafe_code,
                reason = "stable Rust cannot control Windows handle inheritance"
            )]
            let handle = unsafe { GetStdHandle(standard) };
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            // A detached process may legitimately have no standard stream.
            if handle.is_null() || guard.handles.iter().any(|(saved, _)| *saved == handle) {
                continue;
            }
            let mut flags = 0;
            // SAFETY: the OS validates the opaque handle; flags is a live writable
            // u32 for this synchronous call. No borrowed handle is dereferenced.
            #[expect(
                unsafe_code,
                reason = "stable Rust cannot inspect Windows handle inheritance"
            )]
            if unsafe { GetHandleInformation(handle, &mut flags) } == 0 {
                return Err(io::Error::last_os_error());
            }
            if flags & HANDLE_FLAG_INHERIT == 0 {
                continue;
            }
            // Record before changing so every error path restores earlier changes.
            guard.handles.push((handle, flags));
            Self::set_inheritance(handle, 0)?;
        }
        Ok(guard)
    }
    fn set_inheritance(handle: HANDLE, flags: u32) -> io::Result<()> {
        // SAFETY: this is a borrowed standard handle kept alive by the process.
        // Win32 validates it and changes only HANDLE_FLAG_INHERIT; no pointer is
        // dereferenced, closed, or transferred to Rust ownership.
        #[expect(
            unsafe_code,
            reason = "stable Rust cannot set Windows handle inheritance"
        )]
        if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, flags) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    fn restore(&mut self) -> io::Result<()> {
        let mut first_error = None;
        self.handles.retain(|&(handle, flags)| {
            if let Err(error) = Self::set_inheritance(handle, flags) {
                first_error.get_or_insert(error);
                true // Keep failed entries for the Drop retry.
            } else {
                false
            }
        });
        first_error.map_or(Ok(()), Err)
    }
}
impl Drop for StandardHandleInheritance {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
