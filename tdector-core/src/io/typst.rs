use crate::libs::Project;

pub fn escape_typst(s: &str) -> String {
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

#[must_use]
pub fn generate_typst_content(project: &Project) -> String {
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
