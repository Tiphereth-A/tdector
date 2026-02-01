//! Interactive segment and token rendering components.
//!
//! This module provides the UI components for displaying and editing text segments,
//! including tokens with glosses and translations. It handles both view and edit modes,
//! with special support for custom fonts and dictionary-style interaction.

use std::collections::HashMap;
use std::sync::Arc;

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
    formation_rules: &[crate::models::FormationRule],
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
            let is_highlighted = highlight_token.is_some_and(|h| h == token.original);
            let text = &token.original;

            let base_word = token.base_word.as_ref().unwrap_or(text);
            let base_gloss = vocabulary.get(base_word).map(|s| s.as_str()).unwrap_or("");
            let comment = vocabulary_comments
                .get(base_word)
                .map(|s| s.as_str())
                .unwrap_or("");

            let gloss_owned = if !token.formation_rule_indices.is_empty() {
                let descriptions: Vec<String> = token
                    .formation_rule_indices
                    .iter()
                    .filter_map(|idx| formation_rules.get(*idx))
                    .map(|rule| rule.description.clone())
                    .collect();

                if descriptions.is_empty() {
                    base_gloss.to_string()
                } else {
                    format!("{base_gloss} ({})", descriptions.join(" + "))
                }
            } else {
                base_gloss.to_string()
            };

            ui.vertical(|ui| {
                let gloss_richtext = egui::RichText::new(gloss_owned)
                    .family(egui::FontFamily::Proportional)
                    .size(constants::GLOSS_FONT_SIZE)
                    .color(text_color);

                let gloss_resp = ui.add(egui::Label::new(gloss_richtext).extend());

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
                    clicked_action = Some(UiAction::Filter(Arc::from(text.as_str())));
                } else if resp.secondary_clicked() {
                    clicked_action = Some(UiAction::ShowWordMenu(Arc::from(text.as_str()), 0));
                }
            });
        }
    });
    clicked_action
}

/// Renders a complete segment with tokens, glosses, and translation.
///
/// Displays a segment as a group containing:
/// - A segment number/title with right-click sentence menu
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
    use_custom_font: bool,
    formation_rules: &[crate::models::FormationRule],
) -> UiAction {
    let mut action = UiAction::None;
    ui.group(|ui| {
        let title = egui::RichText::new(format!("[{seg_num}]")).weak();
        let mut title_resp = ui.add(egui::Label::new(title).sense(egui::Sense::click()));

        if !segment.comment.is_empty() {
            title_resp = title_resp.on_hover_text(&segment.comment);
        }

        if title_resp.secondary_clicked() {
            action = UiAction::ShowSentenceMenu(seg_num - 1);
        }

        egui::ScrollArea::horizontal()
            .id_salt(seg_num)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = constants::SEGMENT_SPACING_X;
                    for (word_idx, token) in segment.tokens.iter_mut().enumerate() {
                        let token_action = render_token_column(
                            ui,
                            token,
                            vocabulary,
                            vocabulary_comments,
                            highlight,
                            use_custom_font,
                            word_idx,
                            formation_rules,
                        );

                        match token_action {
                            UiAction::Changed => action = UiAction::Changed,
                            UiAction::Filter(_) => action = token_action,
                            UiAction::ShowSimilar(_) => action = token_action,
                            UiAction::ShowDefinition(_) => action = token_action,
                            UiAction::ShowReference(_) => action = token_action,
                            UiAction::ShowSentenceMenu(_) => action = token_action,
                            UiAction::ShowWordMenu(word, _) => {
                                action = UiAction::ShowWordMenu(word, word_idx);
                            }
                            UiAction::None => {}
                        }
                    }
                });
            });

        ui.add_space(constants::SEGMENT_VERTICAL_SPACING);

        let editbox_highlight = None;
        if render_translation_box(ui, segment, editbox_highlight) && action == UiAction::None {
            action = UiAction::Changed;
        }
    });

    action
}

/// Renders a single token as a vertical column with gloss and original text.
/// The token is displayed with:
/// - A bordered box containing the editable gloss (top)
/// - The original token text with optional highlighting (bottom)
///
/// In dictionary mode (always enabled), clicking parts shows dictionary popups
/// or applies filters. Right-clicking shows a context menu.
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
    use_custom_font: bool,
    word_idx: usize,
    formation_rules: &[crate::models::FormationRule],
) -> UiAction {
    let base_word = token.base_word.as_ref().unwrap_or(&token.original);
    let base_gloss = vocabulary.get(base_word).cloned().unwrap_or_default();
    let base_comment = vocabulary_comments
        .get(base_word)
        .cloned()
        .unwrap_or_default();

    let (gloss, comment, has_rule) = if !token.formation_rule_indices.is_empty() {
        let descriptions: Vec<String> = token
            .formation_rule_indices
            .iter()
            .filter_map(|idx| formation_rules.get(*idx))
            .map(|rule| rule.description.clone())
            .collect();

        if descriptions.is_empty() {
            (base_gloss, base_comment, false)
        } else {
            let combined_gloss = format!("{base_gloss} ({})", descriptions.join("; "));
            (combined_gloss, base_comment, true)
        }
    } else {
        (base_gloss, base_comment, false)
    };

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

    ui.allocate_ui_with_layout(
        egui::vec2(width + constants::GLOSS_BOX_LAYOUT_EXTRA, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            let box_color = if has_rule {
                colors::GLOSSBOX_BYFORMATION
            } else {
                colors::GLOSSBOX
            };

            egui::Frame::NONE
                .stroke(egui::Stroke::new(constants::BOX_STROKE_WIDTH, box_color))
                .inner_margin(constants::GLOSS_BOX_INNER_MARGIN)
                .corner_radius(constants::GLOSS_BOX_ROUNDING)
                .show(ui, |ui| {
                    if has_rule {
                        // Tokens with formation rules show read-only combined gloss
                        let label_resp = ui.add_sized(
                            egui::vec2(width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::Label::new(egui::RichText::new(&gloss).color(text_color))
                                .truncate(),
                        );

                        if !comment.is_empty() {
                            label_resp.on_hover_text(&comment);
                        }
                    } else {
                        // Regular tokens have editable gloss
                        let lookup_word = base_word.clone();
                        let mut current_gloss = vocabulary
                            .get(&lookup_word)
                            .cloned()
                            .unwrap_or_default();

                        let edit_resp = ui.add_sized(
                            egui::vec2(width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::TextEdit::singleline(&mut current_gloss)
                                .text_color(text_color)
                                .frame(false),
                        );

                        if edit_resp.changed() {
                            vocabulary.insert(lookup_word, current_gloss);
                            action = UiAction::Changed;
                        }

                        if !comment.is_empty() {
                            edit_resp.on_hover_text(&comment);
                        }
                    }
                });

            let layout_job =
                create_highlighted_layout(&token.original, highlight, token_font_id, text_color);
            let mut label_resp = ui.add(egui::Label::new(layout_job).sense(egui::Sense::click()));

            if !comment.is_empty() {
                label_resp = label_resp.on_hover_text(&comment);
            }

            if label_resp.clicked() {
                action = UiAction::Filter(Arc::from(token.original.as_str()));
            } else if label_resp.secondary_clicked() {
                action = UiAction::ShowWordMenu(Arc::from(token.original.as_str()), word_idx);
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
