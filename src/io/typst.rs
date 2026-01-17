//! Typst markup export for interlinear glossing.

use std::fs;
use std::path::Path;

use crate::models::Project;

/// Exports the project as Typst markup.
pub fn save_typst_file(project: &Project, path: &Path) -> Result<(), String> {
    let content = generate_typst_content(project);
    fs::write(path, &content).map_err(|e| format!("Failed to export file: {}", e))?;
    Ok(())
}

/// Escapes special Typst characters.
fn escape_typst(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '[' | ']' | '#' | '*' | '_' | '`' | '$' | '\\' | '@' | '<' | '>' => {
                result.push('\\');
                result.push(c);
            }
            '\r' | '\n' => {}
            _ => result.push(c),
        }
    }
    result
}

/// Generates the full Typst document content.
#[must_use]
fn generate_typst_content(project: &Project) -> String {
    let mut content = String::new();
    content.push_str("#set page(paper: \"a4\")\n");

    if let Some(font_path) = &project.font_path {
        let font_name = std::path::Path::new(font_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        if !font_name.is_empty() {
            content.push_str(&format!(
                "#set text(size: 12pt, font: \"{}\")\n",
                escape_typst(font_name)
            ));
        } else {
            content.push_str("#set text(size: 12pt)\n");
        }
    } else {
        content.push_str("#set text(size: 12pt)\n");
    }

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
                r#"#box(stack(dir: ttb, align(center, text(size: 8pt)[{}]), v(0.5em), align(center)[{}])) #h(5pt) "#,
                gloss, original
            ));
        }
        content.push_str("\n\n");

        let trans = if segment.translation.is_empty() {
            String::new()
        } else {
            escape_typst(&segment.translation)
        };
        content.push_str(&format!("  *trans:* {}\n]\n#v(1em)\n", trans));
    }

    content
}
