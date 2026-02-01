//! Text highlighting utilities for search query visualization.
//!
//! This module provides case-insensitive substring matching with visual highlighting
//! for search results in the UI. The highlighting is implemented at the character level
//! to properly handle Unicode and multi-byte characters.

use eframe::egui;

use super::colors;

/// Creates a text layout with highlighted search matches.
///
/// Performs case-insensitive substring matching and applies visual highlighting
/// to all matches found in the text. The search is Unicode-aware and handles
/// multi-byte characters correctly.
///
/// # Arguments
///
/// * `text` - The text to search and highlight
/// * `query` - Search query (if `None` or empty, returns unhighlighted text)
/// * `font_id` - Font specification for the text
/// * `text_color` - Base color for non-highlighted text
///
/// # Returns
///
/// An `egui::text::LayoutJob` with highlighted matches ready for rendering.
///
/// # Performance
///
/// The query is normalized to lowercase on each call. For repeated highlighting
/// of the same query, consider pre-normalizing the query string.
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
        Some(q) if !q.is_empty() => q,
        _ => {
            job.append(text, 0.0, base_format);
            return job;
        }
    };

    let query_chars: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut last_end = 0;

    let char_indices: Vec<(usize, char)> = text.char_indices().collect();

    let mut i = 0;
    while i < char_indices.len() {
        let (start_byte, _) = char_indices[i];

        let mut matched = true;
        let mut match_char_count = 0;
        let mut query_iter = query_chars.iter().peekable();
        let mut text_iter = char_indices[i..].iter().peekable();

        while let Some(&query_char) = query_iter.next() {
            if let Some(&(_, text_char)) = text_iter.next() {
                let text_lower: char = text_char.to_lowercase().next().unwrap_or(text_char);
                if text_lower != query_char {
                    matched = false;
                    break;
                }
                match_char_count += 1;
            } else {
                matched = false;
                break;
            }
        }

        if matched && match_char_count > 0 {
            let end_idx = i + match_char_count;
            let end_byte = if end_idx < char_indices.len() {
                char_indices[end_idx].0
            } else {
                text.len()
            };

            if start_byte > last_end {
                job.append(&text[last_end..start_byte], 0.0, base_format.clone());
            }

            job.append(
                &text[start_byte..end_byte],
                0.0,
                egui::TextFormat {
                    background: colors::HIGHLIGHT_BG,
                    color: colors::HIGHLIGHT_FG,
                    font_id: font_id.clone(),
                    ..Default::default()
                },
            );

            last_end = end_byte;
            i = end_idx;
        } else {
            i += 1;
        }
    }

    if last_end < text.len() {
        job.append(&text[last_end..], 0.0, base_format);
    }

    job
}
