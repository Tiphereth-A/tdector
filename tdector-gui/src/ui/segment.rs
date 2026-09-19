use std::sync::Arc;
use tdector_app::{Command, annotate_token};

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
use crate::ui::highlight::create_highlighted_layout;
use tdector_core::libs::{Project, Segment, Token};

pub fn render_clickable_tokens(
    ui: &mut egui::Ui,
    tokens: &[Token],
    project: &Project,
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

            let annotation = annotate_token(project, token);
            let comment = &annotation.display_comment;
            let gloss_owned = if annotation.formation_descriptions.is_empty() {
                annotation.base_gloss.clone()
            } else {
                format!(
                    "{} ({})",
                    annotation.base_gloss,
                    annotation.formation_descriptions.join(" + ")
                )
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
    segment: &Segment,
    project: &Project,
    seg_num: usize,
    highlight: Option<&str>,
    use_custom_font: bool,
    commands: &mut Vec<Command>,
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
                    for (word_idx, token) in segment.tokens.iter().enumerate() {
                        let token_action = render_token_column(
                            ui,
                            token,
                            project,
                            highlight,
                            use_custom_font,
                            word_idx,
                            commands,
                        );

                        match token_action {
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
        if let Some(translation) = render_translation_box(ui, segment, editbox_highlight) {
            commands.push(Command::SetTranslation {
                segment: seg_num - 1,
                translation,
            });
        }
    });

    action
}

fn render_token_column(
    ui: &mut egui::Ui,
    token: &Token,
    project: &Project,
    highlight: Option<&str>,
    use_custom_font: bool,
    word_idx: usize,
    commands: &mut Vec<Command>,
) -> UiAction {
    let annotation = annotate_token(project, token);
    let comment = &annotation.display_comment;
    let has_rule = annotation.is_derived;
    let gloss = if annotation.formation_descriptions.is_empty() {
        annotation.base_gloss.clone()
    } else {
        format!(
            "{} ({})",
            annotation.base_gloss,
            annotation.formation_descriptions.join("; ")
        )
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
            let box_color = if gloss.is_empty() {
                ui.visuals().text_color()
            } else if has_rule {
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
                            label_resp.on_hover_text(comment);
                        }
                    } else {
                        let mut current_gloss = annotation.base_gloss.clone();

                        let edit_resp = ui.add_sized(
                            egui::vec2(width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::TextEdit::singleline(&mut current_gloss)
                                .text_color(text_color)
                                .frame(egui::Frame::NONE),
                        );

                        if edit_resp.changed() {
                            commands.push(Command::SetGloss {
                                word: annotation.base_word.clone(),
                                meaning: current_gloss,
                            });
                        }

                        if !comment.is_empty() {
                            edit_resp.on_hover_text(comment);
                        }
                    }
                });

            let layout_job =
                create_highlighted_layout(&token.original, highlight, token_font_id, text_color);
            let mut label_resp = ui.add(egui::Label::new(layout_job).sense(egui::Sense::click()));

            if !comment.is_empty() {
                label_resp = label_resp.on_hover_text(comment);
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
    segment: &Segment,
    highlight: Option<&str>,
) -> Option<String> {
    let mut translation = segment.translation.clone();
    let changed = egui::Frame::NONE
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
                egui::TextEdit::multiline(&mut translation)
                    .desired_width(f32::INFINITY)
                    .desired_rows(TRANSLATION_BOX_ROWS)
                    .frame(egui::Frame::NONE)
                    .layouter(&mut layouter),
            )
            .changed()
        })
        .inner;
    changed.then_some(translation)
}
