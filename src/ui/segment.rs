//! Segment and token rendering components.

use std::collections::HashMap;

use eframe::egui;

use super::colors;
use super::constants;
use super::types::UiAction;
use crate::models::{Segment, Token};
use crate::ui::highlight::create_highlighted_layout;

/// Renders clickable tokens in a horizontal layout.
///
/// Returns `Some(UiAction)` if a token was clicked.
pub fn render_clickable_tokens(
    ui: &mut egui::Ui,
    tokens: &[Token],
    vocabulary: &HashMap<String, String>,
    highlight_token: Option<&str>,
    use_custom_font: bool,
) -> Option<UiAction> {
    let mut clicked_action = None;
    let font_family = if use_custom_font {
        egui::FontFamily::Name("SentenceFont".into())
    } else {
        egui::FontFamily::Proportional
    };

    ui.horizontal(|ui| {
        let text_color = if ui.visuals().dark_mode {
            colors::FONT_DARK
        } else {
            colors::FONT_LIGHT
        };
        ui.spacing_mut().item_spacing.x = constants::TOKEN_SPACING_X;
        ui.spacing_mut().item_spacing.y = constants::TOKEN_SPACING_Y;
        for token in tokens {
            let is_highlighted = highlight_token.map_or(false, |h| h == token.original);
            let text = &token.original;

            let gloss = vocabulary.get(text).map(|s| s.as_str()).unwrap_or("");

            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(gloss)
                            .family(egui::FontFamily::Proportional)
                            .size(constants::GLOSS_FONT_SIZE)
                            .color(text_color),
                    )
                    .extend(),
                );

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

                let resp = ui.add(egui::Label::new(label).extend().sense(egui::Sense::click()));

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

/// Renders a segment with editable glosses and translation.
pub fn render_segment(
    ui: &mut egui::Ui,
    segment: &mut Segment,
    vocabulary: &mut HashMap<String, String>,
    seg_num: usize,
    highlight: Option<&str>,
    dictionary_mode: bool,
    use_custom_font: bool,
) -> UiAction {
    let mut action = UiAction::None;
    ui.group(|ui| {
        let title = egui::RichText::new(format!("[{}]", seg_num)).weak();
        let title_resp = ui.add(egui::Label::new(title).sense(egui::Sense::click()));

        if title_resp
            .on_hover_text("Click to verify similar sentences")
            .clicked()
        {
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

/// Renders a token column: gloss above, original text below.
fn render_token_column(
    ui: &mut egui::Ui,
    token: &mut Token,
    vocabulary: &mut HashMap<String, String>,
    highlight: Option<&str>,
    dictionary_mode: bool,
    use_custom_font: bool,
) -> UiAction {
    let mut gloss = vocabulary.get(&token.original).cloned().unwrap_or_default();

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
            token.original.clone(),
            token_font_id.clone(),
            egui::Color32::PLACEHOLDER,
        )
        .rect
        .width();
    let gloss_width = ui
        .painter()
        .layout_no_wrap(
            gloss.clone(),
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
            // Gloss Box (Top)
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
                            egui::Label::new(egui::RichText::new(&gloss).color(text_color))
                                .truncate()
                                .sense(egui::Sense::click()),
                        );

                        // Dictionary Mode Interaction
                        if label_resp.clicked() {
                            action = UiAction::ShowDefinition(token.original.clone());
                        } else if label_resp.secondary_clicked() {
                            action = UiAction::ShowReference(token.original.clone());
                        }
                    } else {
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut gloss)
                                    .desired_width(width)
                                    .frame(false)
                                    .text_color(text_color),
                            )
                            .changed()
                        {
                            vocabulary.insert(token.original.clone(), gloss);
                            action = UiAction::Changed;
                        }
                    }
                });

            // Original Word (Bottom)
            let layout_job =
                create_highlighted_layout(&token.original, highlight, token_font_id, text_color);
            let label_resp = ui.add(egui::Label::new(layout_job).sense(egui::Sense::click()));

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

/// Renders the translation text editor. Returns `true` if modified.
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
