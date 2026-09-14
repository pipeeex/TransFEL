use crate::app::TransfelApp;
use crate::theme;
use eframe::egui;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    egui::Panel::top("top_bar")
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(20, 12)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("TransFEL").size(22.0).strong());
                ui.add_space(15.0);

                let connected = app.device.is_connected();
                ui.colored_label(
                    if connected { theme::SUCCESS } else { theme::MUTED },
                    if connected { "● Dispositivo conectado" } else { "● Sin dispositivo" },
                );

                ui.add_space(12.0);
                ui.colored_label(theme::MUTED, &app.status);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Buscar dispositivo").clicked() {
                        app.detect_device();
                    }
                });
            });
        });
}