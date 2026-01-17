//! Bottom pagination controls.

use eframe::egui;

use super::colors;
use super::constants;

/// Renders pagination controls. Returns `Some(page)` if navigation occurred.
pub fn render_pagination(
    ctx: &egui::Context,
    current_page: usize,
    total_pages: usize,
    page_size: &mut usize,
) -> Option<usize> {
    if total_pages <= 1 && *page_size == constants::PAGINATION_DEFAULT_PAGE_SIZE {
        return None;
    }

    let mut new_page = None;

    egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
        let text_color = if ui.visuals().dark_mode {
            colors::FONT_DARK
        } else {
            colors::FONT_LIGHT
        };
        ui.with_layout(
            egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Center),
            |ui| {
                if ui
                    .button(egui::RichText::new("<").color(text_color))
                    .clicked()
                    && current_page > 0
                {
                    new_page = Some(current_page - 1);
                }

                let available = ui.available_width();
                let max_items = ((available - constants::PAGINATION_NAV_WIDTH_DEDUCTION)
                    / constants::PAGINATION_ITEM_WIDTH)
                    .floor() as isize;
                let delta = ((max_items - constants::PAGINATION_BUTTON_ADJACENT_COUNT)
                    / constants::PAGINATION_BUTTON_SIDE_COUNT)
                    .max(1);

                for page in 0..total_pages {
                    let p = page as isize;
                    let c = current_page as isize;
                    let near_current = (p - c).abs() <= delta;
                    let is_boundary = page == 0 || page == total_pages - 1;

                    if is_boundary || near_current {
                        let label = format!("{}", page + 1);
                        if page == current_page {
                            ui.label(egui::RichText::new(label).strong().color(text_color));
                        } else if ui
                            .button(egui::RichText::new(label).color(text_color))
                            .clicked()
                        {
                            new_page = Some(page);
                        }
                    } else {
                        if page == 1 {
                            ui.label(egui::RichText::new("...").color(text_color));
                        }
                        if p == c + delta + 1 {
                            ui.label(egui::RichText::new("...").color(text_color));
                        }
                    }
                }

                if ui
                    .button(egui::RichText::new(">").color(text_color))
                    .clicked()
                    && current_page < total_pages - 1
                {
                    new_page = Some(current_page + 1);
                }

                ui.separator();

                egui::ComboBox::from_id_salt("page_size_selector")
                    .selected_text(format!("Show {} per page", page_size))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(page_size, 10, "Show 10 per page");
                        ui.selectable_value(page_size, 20, "Show 20 per page");
                        ui.selectable_value(page_size, 50, "Show 50 per page");
                        ui.selectable_value(page_size, 100, "Show 100 per page");
                    });

                ui.separator();
                ui.label(egui::RichText::new("Jump to:").color(text_color));

                let jump_id = ui.make_persistent_id("jump_page_val");
                let mut jump_page: usize =
                    ui.data(|d| d.get_temp(jump_id)).unwrap_or(current_page + 1);

                let res = ui.add(
                    egui::DragValue::new(&mut jump_page)
                        .range(1..=total_pages)
                        .speed(0.1),
                );

                if res.changed() {
                    ui.data_mut(|d| d.insert_temp(jump_id, jump_page));
                }

                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

                if (res.has_focus() || res.lost_focus()) && enter_pressed {
                    new_page = Some(jump_page.saturating_sub(1));
                    ui.data_mut(|d| d.remove::<usize>(jump_id));

                    if res.has_focus() {
                        res.surrender_focus();
                    }
                } else if res.lost_focus() {
                    ui.data_mut(|d| d.remove::<usize>(jump_id));
                }
            },
        );
    });

    new_page
}
