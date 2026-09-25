use crate::app::{StreamState, Tab, TransfelApp};
use crate::theme;
use crate::ui::{card, section_label};
use eframe::egui;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    egui::Panel::left("sidebar")
        .resizable(true)
        .default_size(260.0)
        .size_range(220.0..=320.0)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ui, |ui| {
            section_label(ui, "DISPOSITIVO");

            card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new(&app.device.model).size(18.0).strong());
                ui.add_space(8.0);
                ui.label(format!("Android {}", app.device.android));
                ui.label(&app.device.resolution);
                ui.label(format!("Densidad {}", app.device.density));
                ui.add_space(8.0);
                if app.device.is_connected() {
                   if app.is_wifi(){
                       ui.colored_label(theme::ACCENT, "📶 WiFi");
                   }else {
                       ui.colored_label(theme::SUCCESS, "🔌 USB");
                   }
                } else {
                    ui.colored_label(theme::DANGER, "Desconectado");
                }
            });

            ui.add_space(25.0);
            section_label(ui, "FUNCIONES");

            nav_button(ui, app, Tab::Pantalla, "▣   Pantalla");

            ui.add_space(7.0);
            nav_button(ui, app, Tab::Archivos, "▤   Archivos");

            ui.add_space(7.0);
            nav_button(ui, app, Tab::Conexion, "📶   Conexion");

            ui.add_space(25.0);

            card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                section_label(ui, "TRANSMISION");

                match app.stream_state {
                    StreamState::EnVivo => ui.colored_label(theme::SUCCESS, "● Transmision activa"),
                    StreamState::Cerrada => ui.colored_label(theme::DANGER, "● Conexion cerrada"),
                    StreamState::Inactiva => ui.colored_label(theme::MUTED, "● Inactiva"),
                };

                let pending = app.files.active_transfers();
                if pending > 0 {
                    ui.add_space(6.0);
                    ui.colored_label(theme::ACCENT, format!("↕ {pending} transferencia(s)"));
                }
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(10.0);
                ui.label(egui::RichText::new("TransFEL - Developed by Villaquiran").size(12.0).color(theme::MUTED));
                ui.add_space(10.0);
                ui.hyperlink_to(
                    egui::RichText::new("⭐ GitHub")
                    .size(12.0)
                    .color(theme::MUTED),
                    "https://github.com/pipeeex/TransFEL"
                )
            });
        });
}

fn nav_button(ui: &mut egui::Ui, app: &mut TransfelApp, tab: Tab, label: &str) {
    let selected = app.tab == tab;
    let button = egui::Button::new(egui::RichText::new(label).size(15.0))
        .min_size(egui::vec2(ui.available_width(), 42.0))
        .fill(if selected { theme::ACCENT } else { theme::CARD });

    if ui.add(button).clicked() {
        app.tab = tab;
    }
}