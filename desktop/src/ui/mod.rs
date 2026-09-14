pub mod files_view;
pub mod screen_view;
pub mod sidebar;
pub mod top_bar;

use crate::theme;
use eframe::egui;

/// Marco de tarjeta reutilizable.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::group(ui.style())
        .fill(theme::CARD)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
}

pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(12.0)
            .strong()
            .color(theme::MUTED),
    );
    ui.add_space(8.0);
}