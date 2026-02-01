//! Interactive segment and token rendering components.
//!
//! This module provides the UI components for displaying and editing text segments,
//! including tokens with glosses and translations. It handles both view and edit modes,
//! with special support for custom fonts and dictionary-style interaction.

use std::collections::HashMap;

use eframe::egui;

use super::colors;
use super::constants;
use super::types::UiAction;
use crate::models::{Segment, Token};
use crate::ui::highlight::create_highlighted_layout;

/// Renders a horizontal layout of clickable tokens with glosses.
///
/// Each token is displayed vertically with its gloss above and original text below.
/// Tokens are interactive and respond to both left-click (show definition) and
/// right-click (show references) actions.
///
/// # Arguments
///
/// * `ui` - The egui UI context
/// * `tokens` - Slice of tokens to render
/// * `vocabulary` - Vocabulary map for looking up glosses
/// * `vocabulary_comments` - Comments map for looking up word comments
/// * `highlight_token` - Optional token text to highlight
/// * `use_custom_font` - Whether to use the custom "SentenceFont" family
///
/// # Returns
///
/// `Some(UiAction)` if a token was clicked, `None` otherwise.
pub fn render_clickable_tokens(
    ui: &mut egui::Ui,
    tokens: &[Token],
    vocabulary: &HashMap<String, String>,
    vocabulary_comments: &HashMap<String, String>,
    highlight_token: Option<&str>,
    use_custom_font: bool,
) -> Option<UiAction> {
    let mut clicked_action = None;

    let font_family = if use_custom_font {
        egui::FontFamily::Name("SentenceFont".into())
    } else {
        egui::FontFamily::Proportional
    };

    let text_color = if ui.visuals().dark_mode {
        colors::FONT_DARK
    } else {
        colors::FONT_LIGHT
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = constants::TOKEN_SPACING_X;
        ui.spacing_mut().item_spacing.y = constants::TOKEN_SPACING_Y;
        for token in tokens {
            let is_highlighted = highlight_token.map_or(false, |h| h == token.original);
            let text = &token.original;

            let gloss = vocabulary.get(text).map(|s| s.as_str()).unwrap_or("");
            let comment = vocabulary_comments
                .get(text)
                .map(|s| s.as_str())
                .unwrap_or("");

            ui.vertical(|ui| {
                let gloss_richtext = egui::RichText::new(gloss)
                    .family(egui::FontFamily::Proportional)
                    .size(constants::GLOSS_FONT_SIZE)
                    .color(text_color);

                let gloss_resp = ui.add(
                    egui::Label::new(gloss_richtext)
                        .extend(),
                );
                
                if !comment.is_empty() {
                    gloss_resp.on_hover_text(comment);
                }

                let label = if is_highlighted {
                    egui::RichText::new(text)
                        .family(font_family.clone())
                        .size(constants::TOKEN_FONT_SIZE)
                        .strong()
                        .background_color(colors::HIGHLIGHT_BG)
                        .color(colors::HIGHLIGHT_FG)
                } else {
                    egui::RichText::new(text)
                        .family(font_family.clone())
                        .size(constants::TOKEN_FONT_SIZE)
                        .color(text_color)
                };

                let mut resp = ui.add(egui::Label::new(label).extend().sense(egui::Sense::click()));

                if !comment.is_empty() {
                    resp = resp.on_hover_text(comment);
                }

                if resp.clicked() {
                    clicked_action = Some(UiAction::ShowDefinition(text.clone()));
                } else if resp.secondary_clicked() {
                    clicked_action = Some(UiAction::ShowReference(text.clone()));
                }
            });
        }
    });
    clicked_action
}

/// Renders a complete segment with tokens, glosses, and translation.
///
/// Displays a segment as a group containing:
/// - A clickable segment number/title that triggers similarity search
/// - A horizontal scrollable area with all tokens and their editable glosses
/// - An editable translation text box at the bottom
///
/// # Arguments
///
/// * `ui` - The egui UI context
/// * `segment` - The segment to render (mutable for editing)
/// * `vocabulary` - Vocabulary map (mutable for gloss updates)
/// * `vocabulary_comments` - Comments map for looking up word comments
/// * `seg_num` - Display number for this segment (1-indexed)
/// * `highlight` - Optional text to highlight in the segment
/// * `dictionary_mode` - If true, clicking tokens shows dictionary popups instead of editing
/// * `use_custom_font` - Whether to use custom font for original text
///
/// # Returns
///
/// A `UiAction` indicating what operation was triggered (if any).
pub fn render_segment(
    ui: &mut egui::Ui,
    segment: &mut Segment,
    vocabulary: &mut HashMap<String, String>,
    vocabulary_comments: &HashMap<String, String>,
    seg_num: usize,
    highlight: Option<&str>,
    dictionary_mode: bool,
    use_custom_font: bool,
) -> UiAction {
    let mut action = UiAction::None;
    ui.group(|ui| {
        let title = egui::RichText::new(format!("[{}]", seg_num)).weak();
        let mut title_resp = ui.add(egui::Label::new(title).sense(egui::Sense::click()));

        if !segment.comment.is_empty() {
            title_resp = title_resp.on_hover_text(&segment.comment);
        }

        title_resp = title_resp.on_hover_text("Click to verify similar sentences");

        if title_resp.clicked() {
            action = UiAction::ShowSimilar(seg_num);
        }

        egui::ScrollArea::horizontal()
            .id_salt(seg_num)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = constants::SEGMENT_SPACING_X;
                    for token in &mut segment.tokens {
                        let token_action = render_token_column(
                            ui,
                            token,
                            vocabulary,
                            vocabulary_comments,
                            highlight,
                            dictionary_mode,
                            use_custom_font,
                        );

                        match token_action {
                            UiAction::Changed => action = UiAction::Changed,
                            UiAction::Filter(_) => action = token_action,
                            UiAction::ShowSimilar(_) => action = token_action,
                            UiAction::ShowDefinition(_) => action = token_action,
                            UiAction::ShowReference(_) => action = token_action,
                            UiAction::None => {}
                        }
                    }
                });
            });

        ui.add_space(constants::SEGMENT_VERTICAL_SPACING);

        if render_translation_box(ui, segment, highlight) {
            if action == UiAction::None {
                action = UiAction::Changed;
            }
        }
    });

    action
}

/// Renders a single token as a vertical column with gloss and original text.
///
/// The token is displayed with:
/// - A bordered box containing the editable gloss (top)
/// - The original token text with optional highlighting (bottom)
///
/// In dictionary mode, clicking either part shows dictionary popups.
/// In edit mode, the gloss is editable and clicking the original sets a filter.
///
/// # Layout
///
/// The column width is calculated to accommodate the wider of the gloss or
/// original text, ensuring visual balance and readability.
fn render_token_column(
    ui: &mut egui::Ui,
    token: &mut Token,
    vocabulary: &mut HashMap<String, String>,
    vocabulary_comments: &HashMap<String, String>,
    highlight: Option<&str>,
    dictionary_mode: bool,
    use_custom_font: bool,
) -> UiAction {
    let gloss = vocabulary.get(&token.original).cloned().unwrap_or_default();
    let comment = vocabulary_comments
        .get(&token.original)
        .cloned()
        .unwrap_or_default();

    let default_font_id = egui::TextStyle::Body.resolve(ui.style());
    let token_font_id = if use_custom_font {
        egui::FontId {
            size: constants::TOKEN_FONT_SIZE,
            family: egui::FontFamily::Name("SentenceFont".into()),
        }
    } else {
        egui::FontId {
            size: constants::TOKEN_FONT_SIZE,
            family: default_font_id.family.clone(),
        }
    };

    let original_width = ui
        .painter()
        .layout_no_wrap(
            token.original.as_str().into(),
            token_font_id.clone(),
            egui::Color32::PLACEHOLDER,
        )
        .rect
        .width();
    let gloss_width = ui
        .painter()
        .layout_no_wrap(
            gloss.as_str().into(),
            default_font_id.clone(),
            egui::Color32::PLACEHOLDER,
        )
        .rect
        .width();

    let width = (original_width.max(gloss_width) + constants::GLOSS_BOX_EXTRA_WIDTH)
        .max(constants::GLOSS_BOX_MIN_WIDTH);

    let text_color = if ui.visuals().dark_mode {
        colors::FONT_DARK
    } else {
        colors::FONT_LIGHT
    };

    let mut action = UiAction::None;
    let mut gloss_edit = gloss;

    ui.allocate_ui_with_layout(
        egui::vec2(width + constants::GLOSS_BOX_LAYOUT_EXTRA, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            egui::Frame::NONE
                .stroke(egui::Stroke::new(
                    constants::BOX_STROKE_WIDTH,
                    colors::GLOSSBOX,
                ))
                .inner_margin(constants::GLOSS_BOX_INNER_MARGIN)
                .corner_radius(constants::GLOSS_BOX_ROUNDING)
                .show(ui, |ui| {
                    if dictionary_mode {
                        let label_resp = ui.add_sized(
                            egui::vec2(width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::Label::new(egui::RichText::new(&gloss_edit).color(text_color))
                                .truncate()
                                .sense(egui::Sense::click()),
                        );

                        let label_resp = if !comment.is_empty() {
                            label_resp.on_hover_text(&comment)
                        } else {
                            label_resp
                        };

                        if label_resp.clicked() {
                            action = UiAction::ShowDefinition(token.original.clone());
                        } else if label_resp.secondary_clicked() {
                            action = UiAction::ShowReference(token.original.clone());
                        }
                    } else {
                        let gloss_edit_resp = ui
                            .add(
                                egui::TextEdit::singleline(&mut gloss_edit)
                                    .desired_width(width)
                                    .frame(false)
                                    .text_color(text_color),
                            );
                        
                        let gloss_edit_resp = if !comment.is_empty() {
                            gloss_edit_resp.on_hover_text(&comment)
                        } else {
                            gloss_edit_resp
                        };

                        if gloss_edit_resp.changed() {
                            vocabulary.insert(token.original.clone(), gloss_edit.clone());
                            action = UiAction::Changed;
                        }
                    }
                });

            let layout_job =
                create_highlighted_layout(&token.original, highlight, token_font_id, text_color);
            let mut label_resp = ui.add(egui::Label::new(layout_job).sense(egui::Sense::click()));

            if !comment.is_empty() {
                label_resp = label_resp.on_hover_text(&comment);
            }

            if dictionary_mode {
                if label_resp.clicked() {
                    action = UiAction::ShowDefinition(token.original.clone());
                } else if label_resp.secondary_clicked() {
                    action = UiAction::ShowReference(token.original.clone());
                }
            } else {
                if label_resp.clicked() {
                    action = UiAction::Filter(token.original.clone());
                } else if label_resp.secondary_clicked() {
                    action = UiAction::ShowReference(token.original.clone());
                }
            }
        },
    );
    action
}

/// Renders the multi-line translation editor for a segment.
///
/// Displays a bordered, multi-line text input with optional search highlighting.
/// The text editor automatically grows to fit content up to the configured
/// number of rows.
///
/// # Returns
///
/// `true` if the translation text was modified, `false` otherwise.
fn render_translation_box(
    ui: &mut egui::Ui,
    segment: &mut Segment,
    highlight: Option<&str>,
) -> bool {
    egui::Frame::NONE
        .stroke(egui::Stroke::new(
            constants::TRANSLATION_BOX_STROKE_WIDTH,
            colors::SENTENCEBOX,
        ))
        .inner_margin(constants::TRANSLATION_BOX_INNER_MARGIN)
        .corner_radius(constants::TRANSLATION_BOX_ROUNDING)
        .show(ui, |ui| {
            let text_color = if ui.visuals().dark_mode {
                colors::FONT_DARK
            } else {
                colors::FONT_LIGHT
            };
            let mut layouter = |ui: &egui::Ui, string: &dyn egui::TextBuffer, wrap_width: f32| {
                let string = string.as_str();
                let font_id = egui::TextStyle::Body.resolve(ui.style());
                let mut layout_job =
                    create_highlighted_layout(string, highlight, font_id, text_color);
                layout_job.wrap.max_width = wrap_width;
                ui.painter().layout_job(layout_job)
            };

            ui.add(
                egui::TextEdit::multiline(&mut segment.translation)
                    .desired_width(f32::INFINITY)
                    .desired_rows(constants::TRANSLATION_BOX_ROWS)
                    .frame(false)
                    .layouter(&mut layouter),
            )
            .changed()
        })
        .inner
}
