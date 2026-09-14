    use crate::app::{StreamState, TransfelApp};
    use crate::theme;
    use eframe::egui;

    pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Pantalla del dispositivo").size(20.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match app.stream_state {
                    StreamState::EnVivo => { ui.colored_label(theme::SUCCESS, "● EN VIVO"); }
                    StreamState::Cerrada => { ui.colored_label(theme::DANGER, "● DESCONECTADO"); }
                    StreamState::Inactiva => {}
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
                
                    let(titulo, sub, color) = match app.stream_state {
                        StreamState::Cerrada => (
                            "Conexion cerrada",
                            "Se finalizo la trasmicion desde la app",
                            theme::DANGER,
                        ),
                        _ => (
                            "¡Mira aqui la pantalla de tu celular!",
                            "Inicia la transmisión desde la app Movil",
                            theme:: MUTED,
                        ),
                    };

                    ui.vertical_centered(|ui|{
                        ui.add_space((area.y / 2.0 - 80.0).max(0.0));
                        ui.label(egui::RichText::new(if app.stream_state == StreamState::Cerrada { "⛌" } else { "▣" }).size(42.0).color(color));
                        ui.label(egui::RichText::new(titulo).size(24.0).strong().color(color));
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(sub).size(15.0).color(theme::MUTED));

                        if app.stream_state == StreamState::Cerrada {
                            ui.add_space(16.0);
                            if ui.button("Esperar nueva conexion").clicked(){
                                app.stream_state = StreamState::Inactiva;
                            }
                        }
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
