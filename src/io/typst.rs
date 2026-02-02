//! Typst markup export for interlinear glossed text.
//!
//! This module generates Typst documents from projects, creating professionally
//! typeset interlinear glossed text suitable for academic publications.
//!
//! The generated markup includes:
//! - Document setup (page size, fonts)
//! - Project title as a heading
//! - Each segment formatted with aligned glosses above tokens
//! - Translations displayed below each segment

use std::fs;
use std::path::Path;

use crate::models::Project;

/// Exports a project to a Typst markup file.
///
/// Generates Typst markup for the entire project and writes it to the
/// specified file path.
///
/// # Arguments
///
/// * `project` - The project to export
/// * `path` - Destination file path for the `.typ` file
///
/// # Returns
///
/// `Ok(())` on success, or an error message string on failure.
///
/// # Errors
///
/// Returns an error if the file cannot be written (permissions, disk space, etc.).
pub fn save_typst_file(project: &Project, path: &Path) -> Result<(), String> {
    let content = generate_typst_content(project);
    fs::write(path, &content).map_err(|e| format!("Failed to export file: {e}"))?;
    Ok(())
}

/// Escapes text for safe inclusion in Typst markup.
///
/// Typst uses several characters for special purposes in its markup language.
/// This function backslash-escapes all such characters and removes line breaks
/// to ensure text is rendered literally without interpretation.
///
/// # Escaped Characters
///
/// `[`, `]`, `#`, `*`, `_`, `` ` ``, `$`, `\`, `@`, `<`, `>`, `{`, `}`, `"`, `~`, `=`, `&`
///
/// # Removed Characters
///
/// `\r`, `\n` (line breaks are controlled by the markup structure)
///
/// # Arguments
///
/// * `s` - The text to escape
///
/// # Returns
///
/// The escaped string safe for embedding in Typst markup.
fn escape_typst(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '[' | ']' | '#' | '*' | '_' | '`' | '$' | '\\' | '@' | '<' | '>' | '{' | '}' | '"'
            | '~' | '=' | '&' => {
                result.push('\\');
                result.push(c);
            }
            '\r' | '\n' => {}
            _ => result.push(c),
        }
    }
    result
}

/// Generates the complete Typst document markup.
///
/// Creates a full Typst document including:
/// 1. Page setup (A4 paper)
/// 2. Font configuration (using custom font if specified)
/// 3. Project title as level-1 heading
/// 4. All segments with interlinear glossing layout
///
/// Each segment is rendered as a block containing:
/// - Horizontally aligned tokens with glosses above
/// - Translation text below with "trans:" prefix
///
/// # Arguments
///
/// * `project` - The project to convert to Typst markup
///
/// # Returns
///
/// A complete Typst document as a string.
#[must_use]
fn generate_typst_content(project: &Project) -> String {
    let mut content = String::new();
    content.push_str("#set page(paper: \"a4\")\n");
    content.push_str("#set text(size: 12pt)\n");
    content.push_str(&format!("= {}\n\n", escape_typst(&project.project_name)));

    for segment in &project.segments {
        content.push_str("#block(inset: 10pt, stroke: none)[\n  ");

        for token in &segment.tokens {
            let gloss_text = project
                .vocabulary
                .get(&token.original)
                .map(|s| s.as_str())
                .unwrap_or("");

            let gloss = if gloss_text.is_empty() {
                String::new()
            } else {
                escape_typst(gloss_text)
            };
            let original = escape_typst(&token.original);

            content.push_str(&format!(
                r#"#box(stack(dir: ttb, align(center, text(size: 8pt)[{gloss}]), v(0.5em), align(center)[{original}])) #h(5pt) "#
            ));
        }
        content.push_str("\n\n");

        let trans = if segment.translation.is_empty() {
            String::new()
        } else {
            escape_typst(&segment.translation)
        };
        content.push_str(&format!("  *trans:* {trans}\n]\n#v(1em)\n"));
    }

    content
}
