use crate::app::TransfelApp;
use crate::theme;
use crate::ui::{card, section_label};
use eframe::egui;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Conexion").size(20.0).strong());
    ui.add_space(4.0);
    ui.colored_label(
        theme::MUTED,
        "Conecta por WiFi para usar pantalla y archivos sin cable.",
    );
    ui.add_space(16.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            devices_card(app, ui);
            ui.add_space(14.0);
            enable_card(app, ui);
            ui.add_space(14.0);
            manual_card(app, ui);
            ui.add_space(14.0);
            pair_card(app, ui);
            ui.add_space(14.0);
            log_card(app, ui);
        });
}

// ── Dispositivos detectados ──

fn devices_card(app: &mut TransfelApp, ui: &mut egui::Ui) {
    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            section_label(ui, "DISPOSITIVOS DETECTADOS");

            if app.devices.is_empty() {
                ui.colored_label(theme::MUTED, "Ninguno. Conecta el cable USB para empezar.");
                return;
            }

            let devices = app.devices.clone();
            for device in devices {
                ui.horizontal(|ui| {
                    let active = app.selected.as_deref() == Some(device.serial.as_str());

                    let b = egui::Button::new(
                        egui::RichText::new(device.label()).size(13.0),
                    )
                    .fill(if active { theme::ACCENT } else { theme::CARD_HOVER })
                    .min_size(egui::vec2(250.0, 30.0));

                    if ui.add_enabled(device.ready(), b).clicked() {
                        app.select_device(device.serial.clone());
                    }

                    if !device.ready() {
                        ui.colored_label(theme::DANGER, &device.state);
                    } else if active {
                        ui.colored_label(theme::SUCCESS, "en uso");
                    }

                    if device.Wifi && ui.small_button("Desconectar").clicked() {
                        app.wifi.disconnect(device.serial.clone());
                    }
                });
                ui.add_space(4.0);
            }
        });
    });
}

// ── Activar WiFi desde USB ──

fn enable_card(app: &mut TransfelApp, ui: &mut egui::Ui) {
    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            section_label(ui, "PASO 1 · ACTIVAR WIFI");
            ui.colored_label(
                theme::MUTED,
                "Con el cable conectado, esto abre el puerto y enlaza por red. Solo hace falta una vez.",
            );
            ui.add_space(10.0);

            let usb = app
                .devices
                .iter()
                .find(|d| !d.Wifi && d.ready())
                .map(|d| d.serial.clone());

            let ready = usb.is_some() && !app.wifi.busy;

            let b = egui::Button::new(
                egui::RichText::new("📶  Activar conexion WiFi").size(15.0).strong(),
            )
            .fill(if ready { theme::ACCENT } else { theme::CARD_HOVER })
            .min_size(egui::vec2(260.0, 38.0));

            if ui.add_enabled(ready, b).clicked() {
                if let Some(ref serial) = usb {
                    app.wifi.enable_from_usb(serial.to_string());
                }
            }

            if usb.is_none() {
                ui.add_space(6.0);
                ui.colored_label(theme::MUTED, "Conecta el cable USB para habilitar este boton.");
            }

            if app.wifi.busy {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.colored_label(theme::MUTED, "Trabajando...");
                });
            }
        });
    });
}

// ── Conexion manual ──

fn manual_card(app: &mut TransfelApp, ui: &mut egui::Ui) {
    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            section_label(ui, "CONEXION MANUAL");
            ui.colored_label(theme::MUTED, "Si ya activaste WiFi antes, entra directo con la IP.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.wifi.ip_input)
                        .hint_text("192.168.1.50:5555")
                        .desired_width(230.0),
                );

                let ready = !app.wifi.ip_input.trim().is_empty() && !app.wifi.busy;
                if ui.add_enabled(ready, egui::Button::new("Conectar")).clicked() {
                    let address = app.wifi.ip_input.clone();
                    app.wifi.connect(&address);
                }
            });

            ui.add_space(6.0);
            ui.checkbox(&mut app.wifi.auto_reconnect, "Reconectar automaticamente al abrir");
        });
    });
}

// ── Emparejamiento Android 11+ ──

fn pair_card(app: &mut TransfelApp, ui: &mut egui::Ui) {
    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            section_label(ui, "SIN CABLE · ANDROID 11 O SUPERIOR");
            ui.colored_label(
                theme::MUTED,
                "Opciones de desarrollador → Depuracion inalambrica → Vincular con codigo.",
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Direccion");
                ui.add(
                    egui::TextEdit::singleline(&mut app.wifi.pair_address)
                        .hint_text("192.168.1.50:37251")
                        .desired_width(180.0),
                );
                ui.label("Codigo");
                ui.add(
                    egui::TextEdit::singleline(&mut app.wifi.pair_code)
                        .hint_text("123456")
                        .desired_width(90.0),
                );

                let ready = !app.wifi.pair_address.trim().is_empty()
                    && !app.wifi.pair_code.trim().is_empty()
                    && !app.wifi.busy;

                if ui.add_enabled(ready, egui::Button::new("Vincular")).clicked() {
                    let address = app.wifi.pair_address.clone();
                    let code = app.wifi.pair_code.clone();
                    app.wifi.pair(&address, &code);
                }
            });

            ui.add_space(6.0);
            ui.colored_label(
                theme::MUTED,
                "Ojo: el puerto de vinculacion es distinto al de conexion. Tras vincular, usa la IP y el puerto que muestra la pantalla de Depuracion inalambrica arriba, en Conexion manual.",
            );
        });
    });
}

// ── Registro ──

fn log_card(app: &mut TransfelApp, ui: &mut egui::Ui) {
    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            section_label(ui, "REGISTRO");

            if app.wifi.log.is_empty() {
                ui.colored_label(theme::MUTED, "Sin actividad.");
                return;
            }

            egui::ScrollArea::vertical()
                .id_salt("wifi_log")
                .max_height(160.0)
                .stick_to_bottom(true)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (error, text) in &app.wifi.log {
                        ui.colored_label(
                            if *error { theme::DANGER } else { theme::MUTED },
                            egui::RichText::new(text).size(12.0),
                        );
                    }
                });
        });
    });
}