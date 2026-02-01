//! Custom JSON formatter for compact array output.
//!
//! Provides a serde_json formatter that keeps arrays on a single line
//! while maintaining readable pretty-printing for objects.

use std::io;

/// Custom JSON formatter that keeps arrays compact on a single line.
///
/// This formatter produces JSON output where:
/// - Arrays are written on a single line: `[1, 2, 3]`
/// - Objects are pretty-printed with proper indentation
/// - 4-space indentation is used for nested structures
pub struct Formatter {
    indent: Vec<u8>,
    current_indent: usize,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            indent: b"  ".to_vec(),
            current_indent: 0,
        }
    }
}

impl serde_json::ser::Formatter for Formatter {
    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b"[")
    }

    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b"]")
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        Ok(())
    }

    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.current_indent += 1;
        writer.write_all(b"{")
    }

    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.current_indent -= 1;
        if self.current_indent > 0 {
            writer.write_all(b"\n")?;
            for _ in 0..self.current_indent {
                writer.write_all(&self.indent)?;
            }
        } else {
            writer.write_all(b"\n")?;
        }
        writer.write_all(b"}")
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if !first {
            writer.write_all(b",")?;
        }
        writer.write_all(b"\n")?;
        for _ in 0..self.current_indent {
            writer.write_all(&self.indent)?;
        }
        Ok(())
    }

    fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        Ok(())
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b": ")
    }

    fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        Ok(())
    }
}
