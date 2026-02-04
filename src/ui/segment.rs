use crate::libs::formation::FormationRule;
use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;

use crate::consts::{
    colors::{
        FONT_DARK, FONT_LIGHT, GLOSSBOX, GLOSSBOX_BYFORMATION, HIGHLIGHT_BG, HIGHLIGHT_FG,
        SENTENCEBOX,
    },
    ui::{
        BOX_STROKE_WIDTH, GLOSS_BOX_EXTRA_WIDTH, GLOSS_BOX_INNER_MARGIN, GLOSS_BOX_LAYOUT_EXTRA,
        GLOSS_BOX_MIN_WIDTH, GLOSS_BOX_ROUNDING, GLOSS_FONT_SIZE, SEGMENT_SPACING_X,
        SEGMENT_VERTICAL_SPACING, TOKEN_FONT_SIZE, TOKEN_SPACING_X, TOKEN_SPACING_Y,
        TRANSLATION_BOX_INNER_MARGIN, TRANSLATION_BOX_ROUNDING, TRANSLATION_BOX_ROWS,
        TRANSLATION_BOX_STROKE_WIDTH,
    },
};
use crate::enums::UiAction;
use crate::libs::{Segment, Token};
use crate::ui::highlight::create_highlighted_layout;

pub fn render_clickable_tokens(
    ui: &mut egui::Ui,
    tokens: &[Token],
    vocabulary: &HashMap<String, String>,
    vocabulary_comments: &HashMap<String, String>,
    formatted_word_comments: &HashMap<String, String>,
    highlight_token: Option<&str>,
    use_custom_font: bool,
    formation_rules: &[FormationRule],
) -> Option<UiAction> {
    let mut clicked_action = None;

    let font_family = if use_custom_font {
        egui::FontFamily::Name("SentenceFont".into())
    } else {
        egui::FontFamily::Proportional
    };

    let text_color = if ui.visuals().dark_mode {
        FONT_DARK
    } else {
        FONT_LIGHT
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = TOKEN_SPACING_X;
        ui.spacing_mut().item_spacing.y = TOKEN_SPACING_Y;
        for (word_idx, token) in tokens.iter().enumerate() {
            let is_highlighted = highlight_token.is_some_and(|h| h == token.original);
            let text = &token.original;

            let base_word = token.base_word.as_ref().unwrap_or(text);
            let base_gloss = vocabulary.get(base_word).map(|s| s.as_str()).unwrap_or("");
            let base_comment = vocabulary_comments
                .get(base_word)
                .map(|s| s.as_str())
                .unwrap_or("");
            let formatted_comment = if !token.formation_rule_indices.is_empty() {
                formatted_word_comments
                    .get(text)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            let comment = if !formatted_comment.is_empty() {
                formatted_comment
            } else {
                base_comment
            };

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
                    .size(GLOSS_FONT_SIZE)
                    .color(text_color);

                let gloss_resp = ui.add(egui::Label::new(gloss_richtext).extend());

                if !comment.is_empty() {
                    gloss_resp.on_hover_text(comment);
                }

                let label = if is_highlighted {
                    egui::RichText::new(text)
                        .family(font_family.clone())
                        .size(TOKEN_FONT_SIZE)
                        .strong()
                        .background_color(HIGHLIGHT_BG)
                        .color(HIGHLIGHT_FG)
                } else {
                    egui::RichText::new(text)
                        .family(font_family.clone())
                        .size(TOKEN_FONT_SIZE)
                        .color(text_color)
                };

                let mut resp = ui.add(egui::Label::new(label).extend().sense(egui::Sense::click()));

                if !comment.is_empty() {
                    resp = resp.on_hover_text(comment);
                }

                if resp.clicked() {
                    clicked_action = Some(UiAction::Filter(Arc::from(text.as_str())));
                } else if resp.secondary_clicked() {
                    clicked_action =
                        Some(UiAction::ShowWordMenu(Arc::from(text.as_str()), word_idx));
                }
            });
        }
    });
    clicked_action
}

pub fn render_segment(
    ui: &mut egui::Ui,
    segment: &mut Segment,
    vocabulary: &mut HashMap<String, String>,
    vocabulary_comments: &HashMap<String, String>,
    formatted_word_comments: &HashMap<String, String>,
    seg_num: usize,
    highlight: Option<&str>,
    use_custom_font: bool,
    formation_rules: &[FormationRule],
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
                    ui.spacing_mut().item_spacing.x = SEGMENT_SPACING_X;
                    for (word_idx, token) in segment.tokens.iter_mut().enumerate() {
                        let token_action = render_token_column(
                            ui,
                            token,
                            vocabulary,
                            vocabulary_comments,
                            formatted_word_comments,
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

        ui.add_space(SEGMENT_VERTICAL_SPACING);

        let editbox_highlight = None;
        if render_translation_box(ui, segment, editbox_highlight) && action == UiAction::None {
            action = UiAction::Changed;
        }
    });

    action
}

fn render_token_column(
    ui: &mut egui::Ui,
    token: &mut Token,
    vocabulary: &mut HashMap<String, String>,
    vocabulary_comments: &HashMap<String, String>,
    formatted_word_comments: &HashMap<String, String>,
    highlight: Option<&str>,
    use_custom_font: bool,
    word_idx: usize,
    formation_rules: &[FormationRule],
) -> UiAction {
    let base_word = token.base_word.as_ref().unwrap_or(&token.original);
    let base_gloss = vocabulary.get(base_word).cloned().unwrap_or_default();
    let base_comment = vocabulary_comments
        .get(base_word)
        .cloned()
        .unwrap_or_default();
    let formatted_comment = if !token.formation_rule_indices.is_empty() {
        formatted_word_comments
            .get(&token.original)
            .cloned()
            .unwrap_or_default()
    } else {
        String::new()
    };
    let active_comment = if !formatted_comment.is_empty() {
        formatted_comment
    } else {
        base_comment
    };

    let (gloss, comment, has_rule) = if !token.formation_rule_indices.is_empty() {
        let descriptions: Vec<String> = token
            .formation_rule_indices
            .iter()
            .filter_map(|idx| formation_rules.get(*idx))
            .map(|rule| rule.description.clone())
            .collect();

        if descriptions.is_empty() {
            (base_gloss, active_comment, false)
        } else {
            let combined_gloss = format!("{base_gloss} ({})", descriptions.join("; "));
            (combined_gloss, active_comment, true)
        }
    } else {
        (base_gloss, active_comment, false)
    };

    let default_font_id = egui::TextStyle::Body.resolve(ui.style());
    let token_font_id = if use_custom_font {
        egui::FontId {
            size: TOKEN_FONT_SIZE,
            family: egui::FontFamily::Name("SentenceFont".into()),
        }
    } else {
        egui::FontId {
            size: TOKEN_FONT_SIZE,
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

    let width = (original_width.max(gloss_width) + GLOSS_BOX_EXTRA_WIDTH).max(GLOSS_BOX_MIN_WIDTH);

    let text_color = if ui.visuals().dark_mode {
        FONT_DARK
    } else {
        FONT_LIGHT
    };

    let mut action = UiAction::None;

    ui.allocate_ui_with_layout(
        egui::vec2(width + GLOSS_BOX_LAYOUT_EXTRA, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            let box_color = if has_rule {
                GLOSSBOX_BYFORMATION
            } else {
                GLOSSBOX
            };

            egui::Frame::NONE
                .stroke(egui::Stroke::new(BOX_STROKE_WIDTH, box_color))
                .inner_margin(GLOSS_BOX_INNER_MARGIN)
                .corner_radius(GLOSS_BOX_ROUNDING)
                .show(ui, |ui| {
                    if has_rule {
                        let label_resp = ui.add_sized(
                            egui::vec2(width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::Label::new(egui::RichText::new(&gloss).color(text_color))
                                .truncate(),
                        );

                        if !comment.is_empty() {
                            label_resp.on_hover_text(&comment);
                        }
                    } else {
                        let lookup_word = base_word.clone();
                        let mut current_gloss =
                            vocabulary.get(&lookup_word).cloned().unwrap_or_default();

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

fn render_translation_box(
    ui: &mut egui::Ui,
    segment: &mut Segment,
    highlight: Option<&str>,
) -> bool {
    egui::Frame::NONE
        .stroke(egui::Stroke::new(TRANSLATION_BOX_STROKE_WIDTH, SENTENCEBOX))
        .inner_margin(TRANSLATION_BOX_INNER_MARGIN)
        .corner_radius(TRANSLATION_BOX_ROUNDING)
        .show(ui, |ui| {
            let text_color = if ui.visuals().dark_mode {
                FONT_DARK
            } else {
                FONT_LIGHT
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
                    .desired_rows(TRANSLATION_BOX_ROWS)
                    .frame(false)
                    .layouter(&mut layouter),
            )
            .changed()
        })
        .inner
}
