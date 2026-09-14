use crate::app::TransfelApp;
use crate::files::{human_size, Category, Side, TransferState};
use crate::theme;
use crate::ui::{card, section_label};
use eframe::egui;

const BANDEJA_W: f32 = 310.0;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    header(app, ui);
    ui.add_space(10.0);
    search_bar(app, ui);
    ui.add_space(8.0);
    path_bar(app, ui);
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Cuerpo: lista (izq) + bandeja (der) ──
    let avail = ui.available_size();
    let list_w = (avail.x - BANDEJA_W - 16.0).max(280.0);
    let body_h = avail.y.max(200.0);

    ui.horizontal_top(|ui| {
        // IMPORTANTE: layout top_down explícito, si no hereda el horizontal.
        ui.allocate_ui_with_layout(
            egui::vec2(list_w, body_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(list_w);
                ui.set_height(body_h);
                file_list(app, ui);
            },
        );

        ui.add_space(16.0);

        ui.allocate_ui_with_layout(
            egui::vec2(BANDEJA_W, body_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(BANDEJA_W);
                ui.set_height(body_h);
                staging_panel(app, ui);
            },
        );
    });
}

// ─────────────────────────── Encabezado ───────────────────────────

fn header(app: &mut TransfelApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Archivos").size(20.0).strong());
        ui.add_space(20.0);

        side_tab(ui, app, Side::Pc, "🖥  PC");
        ui.add_space(6.0);
        side_tab(ui, app, Side::Android, "📱  Celular");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("⟳  Actualizar").clicked() {
                let serial = app.device.serial.clone();
                app.files.refresh_current(serial.as_deref());
            }
            if ui.button("➕  Agregar del PC").clicked() {
                app.files.stage_from_dialog();
            }
        });
    });
}

fn side_tab(ui: &mut egui::Ui, app: &mut TransfelApp, side: Side, label: &str) {
    let selected = app.files.side == side;
    let button = egui::Button::new(egui::RichText::new(label).size(14.0))
        .fill(if selected { theme::ACCENT } else { theme::CARD })
        .min_size(egui::vec2(120.0, 32.0));

    if ui.add(button).clicked() && !selected {
        app.files.side = side;
        let serial = app.device.serial.clone();
        app.files.refresh_current(serial.as_deref());
    }
}

fn search_bar(app: &mut TransfelApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label("🔍");
        ui.add(
            egui::TextEdit::singleline(&mut app.files.search)
                .hint_text("Buscar por nombre...")
                .desired_width(260.0),
        );
        if !app.files.search.is_empty() && ui.small_button("✕").clicked() {
            app.files.search.clear();
        }

        ui.add_space(12.0);

        for cat in Category::ALL {
            let selected = app.files.category == cat;
            let button = egui::Button::new(
                egui::RichText::new(format!("{}  {}", cat.icon(), cat.label())).size(13.0),
            )
            .fill(if selected { theme::ACCENT } else { theme::CARD })
            .corner_radius(14.0);

            if ui.add(button).clicked() {
                app.files.category = cat;
            }
        }
    });
}

fn path_bar(app: &mut TransfelApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.small_button("⬆  Subir").clicked() {
            let serial = app.device.serial.clone();
            app.files.go_up(serial.as_deref());
        }

        let home = match app.files.side {
            Side::Pc => "🏠  Inicio",
            Side::Android => "🏠  /sdcard",
        };
        if ui.small_button(home).clicked() {
            app.files.go_home();
            let serial = app.device.serial.clone();
            app.files.refresh_current(serial.as_deref());
        }

        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(app.files.current_path_label())
                    .size(12.0)
                    .color(theme::MUTED),
            )
            .truncate(),
        );
    });

    if let Some(err) = &app.files.error {
        ui.colored_label(theme::DANGER, err);
    }
}

// ─────────────────────────── Lista ───────────────────────────

fn file_list(app: &mut TransfelApp, ui: &mut egui::Ui) {
    if app.files.side == Side::Android && !app.device.is_connected() {
        card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Sin dispositivo conectado").size(16.0).strong());
                ui.add_space(6.0);
                ui.colored_label(theme::MUTED, "Conecta por USB o vincula por WiFi");
            });
            ui.add_space(40.0);
        });
        return;
    }

    let visible: Vec<_> = app.files.visible().into_iter().cloned().collect();

    if visible.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.colored_label(theme::MUTED, "No hay archivos que coincidan");
        });
        return;
    }

    let row_w = ui.available_width();

    egui::ScrollArea::vertical()
        .id_salt("file_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Forzamos layout vertical dentro del scroll.
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(row_w);

                for entry in &visible {
                    let staged = app.files.is_staged(&entry.path);

                    card(ui)
                        .fill(if staged { theme::CARD_HOVER } else { theme::CARD })
                        .show(ui, |ui| {
                            ui.set_width(row_w - 24.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(entry.icon()).size(18.0));
                                ui.add_space(6.0);

                                // Botones a la derecha primero, el nombre ocupa el resto.
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if entry.is_dir {
                                            if ui.button("Abrir").clicked() {
                                                let serial = app.device.serial.clone();
                                                app.files.enter(entry, serial.as_deref());
                                            }
                                        } else if staged {
                                            if ui.button("✕ Quitar").clicked() {
                                                app.files.unstage(&entry.path);
                                            }
                                        } else if ui.button("➕ Cargar").clicked() {
                                            app.files.stage(entry);
                                        }

                                        ui.with_layout(
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&entry.name)
                                                            .size(14.0)
                                                            .strong(),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.colored_label(theme::MUTED, entry.size_label());
                                            },
                                        );
                                    },
                                );
                            });
                        });

                    ui.add_space(4.0);
                }
            });
        });
}

// ─────────────────────────── Bandeja ───────────────────────────

fn staging_panel(app: &mut TransfelApp, ui: &mut egui::Ui) {
    let w = ui.available_width();

    card(ui).show(ui, |ui| {
        ui.set_width(w - 24.0);
        ui.set_min_height(ui.available_height());

        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            section_label(ui, "BANDEJA");

            if app.files.staged.is_empty() {
                ui.colored_label(theme::MUTED, "Carga archivos desde la lista");
            }

            let mut remove: Option<String> = None;

            egui::ScrollArea::vertical()
                .id_salt("staged")
                .max_height(220.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        for file in &app.files.staged {
                            ui.horizontal(|ui| {
                                ui.label(file.category.icon());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("✕").clicked() {
                                            remove = Some(file.path.clone());
                                        }
                                        ui.with_layout(
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&file.name).size(13.0),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.colored_label(
                                                    theme::MUTED,
                                                    format!(
                                                        "{} · desde {}",
                                                        human_size(file.size),
                                                        match file.origin {
                                                            Side::Pc => "PC",
                                                            Side::Android => "Celular",
                                                        }
                                                    ),
                                                );
                                            },
                                        );
                                    },
                                );
                            });
                            ui.add_space(4.0);
                        }
                    });
                });

            if let Some(path) = remove {
                app.files.unstage(&path);
            }

            ui.add_space(10.0);
            section_label(ui, "DESTINO EN EL CELULAR");
            ui.add(
                egui::TextEdit::singleline(&mut app.files.android_path)
                    .desired_width(ui.available_width()),
            );

            ui.add_space(10.0);

            let ready = !app.files.staged.is_empty() && app.device.is_connected();
            let send =
                egui::Button::new(egui::RichText::new("⇅  Transferir todo").size(15.0).strong())
                    .fill(if ready { theme::ACCENT } else { theme::CARD })
                    .min_size(egui::vec2(ui.available_width(), 40.0));

            if ui.add_enabled(ready, send).clicked() {
                if let Some(serial) = app.device.serial.clone() {
                    let android_dest = app.files.android_path.clone();
                    let pc_dest = app.files.pc_path.clone();
                    app.files.transfer_staged(&serial, &android_dest, &pc_dest);
                }
            }

            ui.add_space(14.0);
            section_label(ui, "HISTORIAL");

            egui::ScrollArea::vertical()
                .id_salt("transfers")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        for t in app.files.transfers.iter().rev() {
                            ui.horizontal(|ui| {
                                let (icon, color) = match &t.state {
                                    TransferState::Pendiente => ("○", theme::MUTED),
                                    TransferState::EnCurso => ("◐", theme::ACCENT),
                                    TransferState::Listo => ("✔", theme::SUCCESS),
                                    TransferState::Error(_) => ("✕", theme::DANGER),
                                };
                                ui.colored_label(color, icon);
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&t.name).size(12.0))
                                        .truncate(),
                                );
                            });
                            if let TransferState::Error(e) = &t.state {
                                ui.colored_label(theme::DANGER, egui::RichText::new(e).size(11.0));
                            }
                        }
                    });
                });
        });
    });
}