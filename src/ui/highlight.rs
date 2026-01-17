//! Text highlighting for search matches.

use eframe::egui;

use super::colors;

/// Creates a `LayoutJob` with highlighted search matches.
#[must_use]
pub fn create_highlighted_layout(
    text: &str,
    query: Option<&str>,
    font_id: egui::FontId,
    text_color: egui::Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();

    let base_format = egui::TextFormat {
        font_id: font_id.clone(),
        color: text_color,
        ..Default::default()
    };

    let query = match query {
        Some(q) if !q.is_empty() => q.to_lowercase(),
        _ => {
            job.append(text, 0.0, base_format);
            return job;
        }
    };

    let text_lower = text.to_lowercase();
    let mut last_end = 0;

    for (start, part) in text_lower.match_indices(&query) {
        let end = start + part.len();

        if start >= last_end
            && end <= text.len()
            && text.is_char_boundary(start)
            && text.is_char_boundary(end)
        {
            if start > last_end {
                job.append(&text[last_end..start], 0.0, base_format.clone());
            }

            job.append(
                &text[start..end],
                0.0,
                egui::TextFormat {
                    background: colors::HIGHLIGHT_BG,
                    color: colors::HIGHLIGHT_FG,
                    font_id: font_id.clone(),
                    ..Default::default()
                },
            );

            last_end = end;
        }
    }

    if last_end < text.len() {
        job.append(&text[last_end..], 0.0, base_format);
    }

    job
}
