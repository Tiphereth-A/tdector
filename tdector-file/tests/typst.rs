use serde_json::json;
use tdector_file::io::generate_typst_content;
use tdector_file::project::{Project, Segment, Token, load_project_from_json};

fn derived_project() -> Project {
    load_project_from_json(json!({
        "version": 2,
        "project_name": "Script study",
        "formation": [
            {
                "description": "Script [PUA]",
                "type": "nonmorphological",
                "command": "fn transform(word) { \"\u{f0010}\" }"
            },
            {
                "description": "plural_*",
                "type": "inflection",
                "command": "fn transform(word) { word + \"[s]\" }"
            }
        ],
        "vocabulary": {
            "original": [
                {"word": "stem", "meaning": "stone [gift] #power"},
                {"word": "\u{f0010}", "meaning": "unrelated surface gloss"}
            ],
            "formatted": [{"word": [0, 0]}, {"word": [0, 0, 1]}]
        },
        "sentences": [{"words": [0, -1, -2], "meaning": "Three forms."}]
    }))
    .expect("valid script-mapping and suffix chains should load")
}

#[test]
fn typst_exports_derived_base_gloss_and_escaped_formation_chain() {
    let project = derived_project();
    let output = generate_typst_content(&project);

    assert!(output.contains(r"text(size: 8pt)[stone \[gift\] \#power (Script \[PUA\])])"));
    assert!(
        output.contains(r"text(size: 8pt)[stone \[gift\] \#power (Script \[PUA\]; plural\_\*)])")
    );
    assert!(output.contains("align(center)[\u{f0010}]))"));
    assert!(output.contains("align(center)[\u{f0010}\\[s\\]]))"));
    assert!(!output.contains("unrelated surface gloss"));
}

#[test]
fn typst_uses_the_current_shared_gloss_for_every_derived_form() {
    let mut project = derived_project();
    project
        .vocabulary
        .insert("stem".into(), "new meaning".into());
    let output = generate_typst_content(&project);

    assert_eq!(output.matches("text(size: 8pt)[new meaning").count(), 3);
    assert!(!output.contains("stone"));
    assert!(output.contains(r"new meaning (Script \[PUA\]; plural\_\*)"));
}

#[test]
fn typst_preserves_plain_tokens_empty_glosses_and_text_escaping() {
    let project = Project {
        project_name: "Plain [study]".into(),
        vocabulary: [("#word".into(), "literal_*\r\n".into())].into(),
        segments: vec![Segment {
            tokens: vec![
                Token {
                    original: "#word".into(),
                    ..Default::default()
                },
                Token {
                    original: "unknown".into(),
                    ..Default::default()
                },
            ],
            translation: "A [translation] #tag.\n".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let output = generate_typst_content(&project);

    assert_eq!(
        output,
        concat!(
            "#set page(paper: \"a4\")\n",
            "#set text(size: 12pt)\n",
            "= Plain \\[study\\]\n\n",
            "#block(inset: 10pt, stroke: none)[\n  ",
            "#box(stack(dir: ttb, align(center, text(size: 8pt)[literal\\_\\*]), v(0.5em), align(center)[\\#word])) #h(5pt) ",
            "#box(stack(dir: ttb, align(center, text(size: 8pt)[]), v(0.5em), align(center)[unknown])) #h(5pt) ",
            "\n\n  *trans:* A \\[translation\\] \\#tag.\n]\n#v(1em)\n"
        )
    );
}
