//! CLI stream handling; native file persistence lives in tdector-io.

use std::io::{self, Read, Write};
use std::path::Path;

pub use tdector_io::{IoError, atomic_write, decode_utf8, same_file};

/// Read a file, or stdin for `-`, retaining the original bytes. The caller validates the number of stdin consumers before calling this.
pub fn read_input(path: &Path) -> Result<Vec<u8>, IoError> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin().lock().read_to_end(&mut bytes)?;
        Ok(bytes)
    } else {
        tdector_io::read_file(path)
    }
}

/// Write an artifact or report to stdout and observe flush failures.
pub fn write_stdout(bytes: &[u8]) -> Result<(), IoError> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()?;
    Ok(())
}
