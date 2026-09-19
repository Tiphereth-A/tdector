//! Buffered machine reports and readable text views of the public DTOs.

use std::fmt::Write as _;
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::error::{Failure, Result};
use crate::io;

pub fn success(data: Value, json_mode: bool) -> Result<()> {
    let bytes = if json_mode {
        json_bytes(&json!({"schema_version": 1, "ok": true, "data": data}))?
    } else {
        let mut text = String::new();
        render(&data, 0, &mut text);
        text.into_bytes()
    };
    io::write_stdout(&bytes)?;
    Ok(())
}

pub fn failure(error: Failure, json_mode: bool) -> ExitCode {
    let exit_code = error.exit_code;
    if json_mode {
        let result = json_bytes(&json!({"schema_version": 1, "ok": false, "error": error}))
            .and_then(|bytes| io::write_stdout(&bytes).map_err(Failure::from));
        if let Err(write_error) = result {
            eprintln!("{}", write_error.message);
            return ExitCode::from(write_error.exit_code);
        }
    } else {
        eprintln!("{}: {}", error.code, error.message);
        if let Some(stage) = error.stage {
            eprintln!("Stage: {stage:?}");
        }
        if let Some(index) = error.command_index {
            eprintln!("Command index: {index}");
        }
    }
    ExitCode::from(exit_code)
}

fn json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn label(key: &str) -> String {
    let text = key.replace('_', " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => text,
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "—".into(),
        _ => value.to_string(),
    }
}

fn render(value: &Value, indent: usize, text: &mut String) {
    let padding = " ".repeat(indent);
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if value.is_object() || value.is_array() {
                    let _ = writeln!(text, "{padding}{}:", label(key));
                    render(value, indent + 2, text);
                } else {
                    let _ = writeln!(text, "{padding}{}: {}", label(key), scalar(value));
                }
            }
        }
        Value::Array(items) if items.is_empty() => {
            let _ = writeln!(text, "{padding}(no items)");
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                if index > 0 && item.is_object() {
                    text.push('\n');
                }
                render(item, indent, text);
            }
        }
        _ => {
            let _ = writeln!(text, "{padding}{}", scalar(value));
        }
    }
}
