use crate::app::TransfelApp;
use crate::files::{Category, Side, TransferState};
use crate::theme;
use crate::ui::{card, section_label};
use eframe::egui;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    // ── Encabezado: lados + acciones ────────────────────────────────
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
            if app.files.side == Side::Pc && ui.button("➕  Agregar del PC").clicked() {
                app.files.stage_from_dialog();
            }
        });
    });

    ui.add_space(10.0);

    // ── Buscador + categorías ───────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label("🔍");
        let search = egui::TextEdit::singleline(&mut app.files.search)
            .hint_text("Buscar por nombre...")
            .desired_width(280.0);
        ui.add(search);

        if !app.files.search.is_empty() && ui.small_button("✕").clicked() {
            app.files.search.clear();
        }

        ui.add_space(15.0);

        for cat in Category::ALL {
            let selected = app.files.category == cat;
            let button = egui::Button::new(format!("{}  {}", cat.icon(), cat.label()))
                .fill(if selected { theme::ACCENT } else { theme::CARD })
                .corner_radius(14.0);
            if ui.add(button).clicked() {
                app.files.category = cat;
            }
        }
    });

    ui.add_space(8.0);

    // ── Ruta actual ─────────────────────────────────────────────────
    ui.horizontal(|ui| {
        if ui.small_button("⬆  Subir").clicked() {
            let serial = app.device.serial.clone();
            app.files.go_up(serial.as_deref());
        }
        ui.add_space(8.0);
        ui.colored_label(theme::MUTED, app.files.current_path_label());
    });

    if let Some(err) = &app.files.error {
        ui.colored_label(theme::DANGER, err);
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Cuerpo: lista + bandeja ─────────────────────────────────────
    let bandeja_width = 300.0;

    ui.horizontal_top(|ui| {
        let list_width = (ui.available_width() - bandeja_width - 16.0).max(300.0);

        ui.allocate_ui(egui::vec2(list_width, ui.available_height()), |ui| {
            file_list(app, ui);
        });

        ui.add_space(16.0);

        ui.allocate_ui(egui::vec2(bandeja_width, ui.available_height()), |ui| {
            staging_panel(app, ui);
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

// ─────────────────────────── Lista de archivos ───────────────────────────

fn file_list(app: &mut TransfelApp, ui: &mut egui::Ui) {
    if app.files.side == Side::Android && !app.device.is_connected() {
        card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Sin dispositivo conectado").size(16.0).strong());
                ui.add_space(6.0);
                ui.colored_label(theme::MUTED, "Conecta el celular por USB con depuracion activada");
            });
            ui.add_space(40.0);
        });
        return;
    }

    // Se clonan solo las entradas visibles para no pelear con el borrow checker.
    let visible: Vec<_> = app.files.visible().into_iter().cloned().collect();

    if visible.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.colored_label(theme::MUTED, "No hay archivos que coincidan");
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("file_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in &visible {
                let staged = app.files.is_staged(&entry.path);

                let response = card(ui)
                    .fill(if staged { theme::CARD_HOVER } else { theme::CARD })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(entry.icon()).size(18.0));
                            ui.add_space(6.0);

                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&entry.name).size(14.0).strong());
                                ui.colored_label(theme::MUTED, entry.size_label());
                            });

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
                                },
                            );
                        });
                    })
                    .response;

                // doble clic en carpeta = abrir
                if entry.is_dir && response.interact(egui::Sense::click()).double_clicked() {
                    let serial = app.device.serial.clone();
                    app.files.enter(entry, serial.as_deref());
                }

                ui.add_space(4.0);
            }
        });
}

// ─────────────────────────── Bandeja + transferencias ───────────────────────────

fn staging_panel(app: &mut TransfelApp, ui: &mut egui::Ui) {
    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(ui.available_height());

        section_label(ui, "BANDEJA");

        if app.files.staged.is_empty() {
            ui.colored_label(theme::MUTED, "Carga archivos desde la lista");
        }

        let mut remove: Option<String> = None;

        egui::ScrollArea::vertical()
            .id_salt("staged")
            .max_height(240.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for file in &app.files.staged {
                    ui.horizontal(|ui| {
                        ui.label(file.category.icon());
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&file.name).size(13.0));
                            ui.colored_label(
                                theme::MUTED,
                                format!(
                                    "{} · desde {}",
                                    crate::files::human_size(file.size),
                                    match file.origin {
                                        Side::Pc => "PC",
                                        Side::Android => "Celular",
                                    }
                                ),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                remove = Some(file.path.clone());
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            });

        if let Some(path) = remove {
            app.files.unstage(&path);
        }

        ui.add_space(10.0);

        // Destinos
        section_label(ui, "DESTINO EN EL CELULAR");
        ui.add(
            egui::TextEdit::singleline(&mut app.files.android_path)
                .desired_width(ui.available_width()),
        );

        ui.add_space(10.0);

        let ready = !app.files.staged.is_empty() && app.device.is_connected();
        let send = egui::Button::new(egui::RichText::new("⇅  Transferir todo").size(15.0).strong())
            .fill(if ready { theme::ACCENT } else { theme::CARD })
            .min_size(egui::vec2(ui.available_width(), 40.0));

        if ui.add_enabled(ready, send).clicked() {
            if let Some(serial) = app.device.serial.clone() {
                let android_dest = app.files.android_path.clone();
                let pc_dest = app.files.pc_path.clone();
                app.files.transfer_staged(&serial, &android_dest, &pc_dest);

                // refresca ambos lados al terminar de encolar
                app.files.refresh_pc();
                app.files.refresh_android(&serial);
            }
        }

        ui.add_space(14.0);
        section_label(ui, "HISTORIAL");

        egui::ScrollArea::vertical()
            .id_salt("transfers")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for t in app.files.transfers.iter().rev() {
                    ui.horizontal(|ui| {
                        let (icon, color) = match &t.state {
                            TransferState::Pendiente => ("○", theme::MUTED),
                            TransferState::EnCurso => ("◐", theme::ACCENT),
                            TransferState::Listo => ("✔", theme::SUCCESS),
                            TransferState::Error(_) => ("✕", theme::DANGER),
                        };
                        ui.colored_label(color, icon);
                        ui.label(egui::RichText::new(&t.name).size(12.0));
                        ui.colored_label(
                            theme::MUTED,
                            match t.direction {
                                Side::Android => "→ celular",
                                Side::Pc => "→ PC",
                            },
                        );
                    });
                    if let TransferState::Error(e) = &t.state {
                        ui.colored_label(theme::DANGER, egui::RichText::new(e).size(11.0));
                    }
                }
            });
    });
}