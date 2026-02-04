use eframe::egui;

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
