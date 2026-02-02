//! Utilities for popup window rendering to reduce duplication.

use eframe::egui;

/// Creates a popup title with optional custom font support.
///
/// Generates consistent title formatting across all popup windows.
/// If `use_custom_font` is true, applies the "`SentenceFont`" to the word portion.
///
/// # Arguments
///
/// * `prefix` - The text prefix (e.g., "Definition: ", "References: ")
/// * `word` - The word to display (potentially in custom font)
/// * `use_custom_font` - Whether to use the custom "`SentenceFont`" family
///
/// # Returns
///
/// A `WidgetText` suitable for window titles
pub fn create_popup_title(prefix: &str, word: &str, use_custom_font: bool) -> egui::WidgetText {
    if use_custom_font {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            prefix,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(14.0, egui::FontFamily::Proportional),
                ..Default::default()
            },
        );
        job.append(
            word,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(14.0, egui::FontFamily::Name("SentenceFont".into())),
                ..Default::default()
            },
        );
        egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
    } else {
        egui::WidgetText::from(format!("{prefix}{word}"))
    }
}

/// Creates a string title for pinned popups with custom font support.
///
/// Similar to `create_popup_title` but returns a plain String for storage
/// in pinned popup state.
pub fn create_pinned_title_string(prefix: &str, word: &str, use_custom_font: bool) -> String {
    if use_custom_font {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            prefix,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(14.0, egui::FontFamily::Proportional),
                ..Default::default()
            },
        );
        job.append(
            word,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(14.0, egui::FontFamily::Name("SentenceFont".into())),
                ..Default::default()
            },
        );
        job.text
    } else {
        format!("{prefix}{word}")
    }
}
