use crate::app::TransfelApp;
use crate::theme;
use eframe::egui;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Pantalla del dispositivo").size(20.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.texture.is_some() {
                ui.colored_label(theme::SUCCESS, "● EN VIVO");
            }
        });
    });

    ui.add_space(12.0);

    egui::Frame::group(ui.style())
        .fill(theme::VIDEO_BG)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            let area = ui.available_size();

            let Some(texture) = &app.texture else {
                ui.set_min_size(area);
                ui.vertical_centered(|ui| {
                    ui.add_space((area.y / 2.0 - 70.0).max(0.0));
                    ui.label(egui::RichText::new("Pantalla Android").size(24.0).strong());
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Inicia la transmision desde la app movil")
                            .size(15.0)
                            .color(theme::MUTED),
                    );
                });
                return;
            };

            let size = texture.size_vec2();
            let scale = (area.x / size.x).min(area.y / size.y);
            let display = size * scale;

            ui.add_space(((area.y - display.y) / 2.0).max(0.0));
            ui.horizontal(|ui| {
                ui.add_space(((area.x - display.x) / 2.0).max(0.0));
                ui.image((texture.id(), display));
            });
        });
}

pub fn show_control_placeholder(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(120.0);
        ui.label(egui::RichText::new("Control remoto").size(24.0).strong());
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("Proximamente: teclado y toques via adb input")
                .size(15.0)
                .color(theme::MUTED),
        );
    });
}