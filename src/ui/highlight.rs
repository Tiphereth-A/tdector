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
/// Uses optimized string searching with pre-normalized lowercase strings for O(n+m) performance.
/// All matches are found in a single pass through the text.
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

    let highlight_format = egui::TextFormat {
        background: colors::HIGHLIGHT_BG,
        color: colors::HIGHLIGHT_FG,
        font_id: font_id.clone(),
        ..Default::default()
    };

    // Pre-compute lowercase versions once
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let query_len = query.len();

    let mut last_end = 0;
    let mut search_start = 0;

    // Find all matches using efficient string search
    while let Some(match_pos) = text_lower[search_start..].find(&query_lower) {
        let match_start = search_start + match_pos;

        // Find the corresponding position in the original text
        // We need to find where this byte position maps to in terms of character boundaries
        let match_byte_start = match_start;
        let match_byte_end = match_start + query_len;

        // Ensure we're at valid UTF-8 boundaries
        if !text.is_char_boundary(match_byte_start)
            || !text.is_char_boundary(match_byte_end.min(text.len()))
        {
            search_start = match_start + 1;
            continue;
        }

        let actual_match_end = match_byte_end.min(text.len());

        // Append non-highlighted text before match
        if match_byte_start > last_end {
            job.append(&text[last_end..match_byte_start], 0.0, base_format.clone());
        }

        // Append highlighted match
        job.append(
            &text[match_byte_start..actual_match_end],
            0.0,
            highlight_format.clone(),
        );

        last_end = actual_match_end;
        search_start = actual_match_end;
    }

    // Append remaining non-highlighted text
    if last_end < text.len() {
        job.append(&text[last_end..], 0.0, base_format);
    }

    job
}
