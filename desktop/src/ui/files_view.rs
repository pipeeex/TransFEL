use crate::app::TransfelApp;
use crate::files::{
    human_size, AndroidDest, Category, Scope, Side, TransferState, ANDROID_SHORTCUTS, PC_SHORTCUTS,
};
use crate::theme;
use crate::ui::{card, section_label};
use eframe::egui;

const BANDEJA_W: f32 = 330.0;

pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
    header(app, ui);
    ui.add_space(10.0);
    filters_card(app, ui);
    ui.add_space(10.0);

    let avail = ui.available_size();
    let list_w = (avail.x - BANDEJA_W - 16.0).max(280.0);
    let body_h = avail.y.max(200.0);

    ui.horizontal_top(|ui| {
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
                tray_panel(app, ui);
            },
        );
    });
}

// ═══════════════════════ Encabezado ═══════════════════════

fn header(app: &mut TransfelApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Archivos").size(20.0).strong());
        ui.add_space(20.0);

        ui.colored_label(theme::MUTED, "Explorando:");
        side_tab(ui, app, Side::Pc, "🖥  Mi PC");
        ui.add_space(4.0);
        side_tab(ui, app, Side::Android, "📱  Celular");

        if app.files.loading {
            ui.add_space(10.0);
            ui.spinner();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("⟳").on_hover_text("Actualizar").clicked() {
                let serial = app.device.serial.clone();
                app.files.invalidate();
                app.files.reload(serial.as_deref());
            }
        });
    });
}

fn side_tab(ui: &mut egui::Ui, app: &mut TransfelApp, side: Side, label: &str) {
    let selected = app.files.side == side;
    let button = egui::Button::new(egui::RichText::new(label).size(14.0))
        .fill(if selected { theme::ACCENT } else { theme::CARD })
        .min_size(egui::vec2(110.0, 30.0));

    if ui.add(button).clicked() {
        let serial = app.device.serial.clone();
        app.files.switch_side(side, serial.as_deref());
    }
}

// ═══════════════════════ Filtros agrupados ═══════════════════════

fn filters_card(app: &mut TransfelApp, ui: &mut egui::Ui) {
    let serial = app.device.serial.clone();

    card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            // ── Fila 1: lugares ──
            ui.horizontal_wrapped(|ui| {
                ui.add_sized(
                    [70.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("Ir a").size(12.0).strong().color(theme::MUTED),
                    ),
                );

                if ui.small_button("⬆  Atrás").clicked() {
                    app.files.go_up(serial.as_deref());
                }

                let list: &[crate::files::Shortcut] = match app.files.side {
                    Side::Pc => &PC_SHORTCUTS,
                    Side::Android => &ANDROID_SHORTCUTS,
                };

                for sc in list {
                    let b = egui::Button::new(
                        egui::RichText::new(format!("{} {}", sc.icon, sc.label)).size(12.0),
                    )
                    .fill(theme::CARD_HOVER)
                    .corner_radius(9.0);

                    if ui.add(b).clicked() {
                        app.files.open_shortcut(sc, serial.as_deref());
                    }
                }
            });

            ui.add_space(8.0);

            // ── Fila 2: buscar, con ámbito explícito ──
            ui.horizontal_wrapped(|ui| {
                ui.add_sized(
                    [70.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("Buscar").size(12.0).strong().color(theme::MUTED),
                    ),
                );

                let response = ui.add(
                    egui::TextEdit::singleline(&mut app.files.search)
                        .hint_text("Nombre del archivo...")
                        .desired_width(240.0),
                );
                if response.changed() {
                    app.files.on_search_changed();
                }

                ui.add_space(6.0);
                ui.colored_label(theme::MUTED, "en");

                let device_label = match app.files.side {
                    Side::Pc => "todo el PC",
                    Side::Android => "todo el celular",
                };

                for (scope, label) in [
                    (Scope::Carpeta, "esta carpeta"),
                    (Scope::Dispositivo, device_label),
                ] {
                    let selected = app.files.scope == scope;
                    let b = egui::Button::new(egui::RichText::new(label).size(12.0))
                        .fill(if selected { theme::ACCENT } else { theme::CARD_HOVER })
                        .corner_radius(9.0);
                    if ui.add(b).clicked() {
                        app.files.set_scope(scope, serial.as_deref());
                    }
                }
            });

            ui.add_space(8.0);

            // ── Fila 3: tipo ──
            ui.horizontal_wrapped(|ui| {
                ui.add_sized(
                    [70.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("Mostrar").size(12.0).strong().color(theme::MUTED),
                    ),
                );

                for cat in Category::ALL {
                    let selected = app.files.category == cat;
                    let b = egui::Button::new(
                        egui::RichText::new(format!("{} {}", cat.icon(), cat.label())).size(12.0),
                    )
                    .fill(if selected { theme::ACCENT } else { theme::CARD_HOVER })
                    .corner_radius(9.0);

                    if ui.add(b).clicked() && !selected {
                        app.files.category = cat;
                        if app.files.scope == Scope::Dispositivo {
                            app.files.on_search_changed();
                        }
                    }
                }
            });

            ui.add_space(8.0);

            // ── Resumen de lo que estás viendo ──
            ui.horizontal(|ui| {
                let (shown, total) = app.files.filter_summary();
                let path = app.files.current_path_label();

                let summary = if app.files.filters_active() {
                    format!("{shown} de {total} · {}", short_path(&path))
                } else {
                    format!("{total} elementos · {}", short_path(&path))
                };

                ui.add(
                    egui::Label::new(
                        egui::RichText::new(summary).size(12.0).color(theme::MUTED),
                    )
                    .truncate(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.files.filters_active() && ui.small_button("✕ Quitar filtros").clicked() {
                        app.files.clear_filters(serial.as_deref());
                    }
                });
            });

            if let Some(err) = &app.files.error {
                ui.colored_label(theme::DANGER, err);
            }
        });
    });
}

/// Acorta rutas largas: C:\Users\juanp\...\Camera
fn short_path(path: &str) -> String {
    if path.len() <= 52 {
        return path.to_string();
    }
    let sep = if path.contains('\\') { '\\' } else { '/' };
    let parts: Vec<&str> = path.split(sep).collect();
    if parts.len() < 4 {
        return path.to_string();
    }
    format!(
        "{}{sep}...{sep}{}",
        parts[0],
        parts[parts.len() - 1]
    )
}

// ═══════════════════════ Lista ═══════════════════════

fn file_list(app: &mut TransfelApp, ui: &mut egui::Ui) {
    if app.files.side == Side::Android && !app.device.is_connected() {
        card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Sin dispositivo conectado").size(16.0).strong());
                ui.add_space(6.0);
                ui.colored_label(theme::MUTED, "Conéctalo por USB o WiFi desde la pestaña Conexion");
            });
            ui.add_space(40.0);
        });
        return;
    }

    let visible: Vec<_> = app.files.visible().into_iter().cloned().collect();

    if visible.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            if app.files.loading {
                ui.spinner();
                ui.add_space(6.0);
                ui.colored_label(theme::MUTED, "Buscando...");
            } else {
                ui.label(egui::RichText::new("Nada por aquí").size(15.0).strong());
                ui.add_space(4.0);
                ui.colored_label(theme::MUTED, "Prueba quitando los filtros o cambiando de carpeta");
            }
        });
        return;
    }

    // Barra de acción de la lista
    let blocked = app
        .files
        .tray_origin()
        .is_some_and(|o| o != app.files.side);

    ui.horizontal(|ui| {
        let files_only = visible.iter().filter(|e| !e.is_dir).count();
        ui.colored_label(theme::MUTED, format!("{files_only} archivo(s) seleccionables"));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.files.side == Side::Pc {
                let b = egui::Button::new("📁  Buscar en el explorador");
                if ui.add_enabled(!blocked, b).clicked() {
                    app.files.stage_from_dialog();
                }
                ui.add_space(4.0);
            }

            let b = egui::Button::new("Seleccionar todos");
            if ui.add_enabled(!blocked && files_only > 0, b).clicked() {
                app.files.stage_all_visible();
            }
        });
    });

    ui.add_space(6.0);

    let row_w = ui.available_width();

    egui::ScrollArea::vertical()
        .id_salt("file_list")
        .auto_shrink([false, false])
        .show_rows(ui, 58.0, visible.len(), |ui, range| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(row_w);

                for entry in &visible[range] {
                    let staged = app.files.is_staged(&entry.path);

                    card(ui)
                        .fill(if staged { theme::ACCENT.gamma_multiply(0.25) } else { theme::CARD })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if staged { theme::ACCENT } else { theme::BORDER },
                        ))
                        .show(ui, |ui| {
                            ui.set_width(row_w - 24.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(entry.icon()).size(18.0));
                                ui.add_space(6.0);

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if entry.is_dir {
                                            if ui.button("Abrir  ›").clicked() {
                                                let serial = app.device.serial.clone();
                                                app.files.enter(entry, serial.as_deref());
                                            }
                                        } else if staged {
                                            if ui
                                                .button(egui::RichText::new("✔ Seleccionado"))
                                                .on_hover_text("Clic para quitar de la bandeja")
                                                .clicked()
                                            {
                                                app.files.unstage(&entry.path);
                                            }
                                        } else {
                                            let b = egui::Button::new("＋ Seleccionar");
                                            if ui.add_enabled(!blocked, b).clicked() {
                                                app.files.stage(entry);
                                            }
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
                                                ui.colored_label(
                                                    theme::MUTED,
                                                    entry.size_label(),
                                                );
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

// ═══════════════════════ Bandeja ═══════════════════════

fn tray_panel(app: &mut TransfelApp, ui: &mut egui::Ui) {
    let w = ui.available_width();

    card(ui).show(ui, |ui| {
        ui.set_width(w - 24.0);
        ui.set_min_height(ui.available_height());

        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            // ── Título con la dirección real ──
            let (title, arrow) = match app.files.tray_origin() {
                Some(Side::Android) => ("TRANSFERENCIA", "📱  Celular   ➜   🖥  PC"),
                Some(Side::Pc) => ("TRANSFERENCIA", "🖥  PC   ➜   📱  Celular"),
                None => ("TRANSFERENCIA", "Selecciona archivos para empezar"),
            };

            section_label(ui, title);
            ui.label(
                egui::RichText::new(arrow)
                    .size(13.0)
                    .strong()
                    .color(if app.files.staged.is_empty() {
                        theme::MUTED
                    } else {
                        theme::ACCENT
                    }),
            );

            if let Some(notice) = app.files.notice.clone() {
                ui.add_space(6.0);
                ui.colored_label(theme::DANGER, egui::RichText::new(notice).size(11.0));
            }

            ui.add_space(10.0);

            // ── Lista de seleccionados ──
            if app.files.staged.is_empty() {
                ui.colored_label(
                    theme::MUTED,
                    "Usa ＋ Seleccionar en la lista de la izquierda.",
                );
            } else {
                ui.horizontal(|ui| {
                    let total: u64 = app.files.staged.iter().map(|f| f.size).sum();
                    let count = app.files.staged.len();
                    ui.colored_label(
                        theme::MUTED,
                        if total > 0 {
                            format!("{count} archivo(s) · {}", human_size(total))
                        } else {
                            format!("{count} archivo(s)")
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Vaciar").clicked() {
                            app.files.clear_tray();
                        }
                    });
                });
                ui.add_space(4.0);
            }

            let mut remove: Option<String> = None;

            egui::ScrollArea::vertical()
                .id_salt("staged")
                .max_height(180.0)
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
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&file.name).size(12.5),
                                            )
                                            .truncate(),
                                        );
                                    },
                                );
                            });
                            ui.add_space(3.0);
                        }
                    });
                });

            if let Some(path) = remove {
                app.files.unstage(&path);
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(10.0);

            // ── Destino, según la dirección ──
            destination_section(app, ui);

            ui.add_space(12.0);

            let ready = !app.files.staged.is_empty() && app.device.is_connected();
            let send = egui::Button::new(
                egui::RichText::new(app.files.transfer_label()).size(15.0).strong(),
            )
            .fill(if ready { theme::ACCENT } else { theme::CARD_HOVER })
            .min_size(egui::vec2(ui.available_width(), 42.0));

            if ui.add_enabled(ready, send).clicked() {
                if let Some(serial) = app.device.serial.clone() {
                    app.files.transfer_staged(&serial);
                }
            }

            if !app.device.is_connected() && !app.files.staged.is_empty() {
                ui.add_space(6.0);
                ui.colored_label(theme::DANGER, "Conecta el celular para transferir.");
            }

            ui.add_space(14.0);
            history_section(app, ui);
        });
    });
}

fn destination_section(app: &mut TransfelApp, ui: &mut egui::Ui) {
    match app.files.tray_destination() {
        Some(Side::Pc) => {
            section_label(ui, "SE GUARDARÁ EN");
            ui.horizontal(|ui| {
                ui.label("🖥");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(short_path(
                            &app.files.pc_dest.to_string_lossy(),
                        ))
                        .size(12.5),
                    )
                    .truncate(),
                );
            });
            ui.add_space(6.0);
            if ui.button("Cambiar carpeta...").clicked() {
                app.files.pick_pc_dest();
            }
        }
        Some(Side::Android) => {
            section_label(ui, "SE GUARDARÁ EN");
            egui::ComboBox::from_id_salt("android_dest")
                .selected_text(app.files.android_dest.label())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for dest in AndroidDest::ALL {
                        ui.selectable_value(&mut app.files.android_dest, dest, dest.label());
                    }
                });

            ui.add_space(4.0);
            let path = match app.files.android_dest {
                AndroidDest::Actual => app.files.android_path.clone(),
                other => other.path().to_string(),
            };
            ui.colored_label(theme::MUTED, egui::RichText::new(path).size(11.0));
        }
        None => {
            ui.colored_label(
                theme::MUTED,
                egui::RichText::new("El destino se elige solo según de dónde vengan los archivos.")
                    .size(11.5),
            );
        }
    }
}

fn history_section(app: &mut TransfelApp, ui: &mut egui::Ui) {
    if app.files.transfers.is_empty() {
        return;
    }

    section_label(ui, "HISTORIAL");

    egui::ScrollArea::vertical()
        .id_salt("transfers")
        .max_height(130.0)
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
                            egui::Label::new(egui::RichText::new(&t.name).size(12.0)).truncate(),
                        );
                    });
                    if let TransferState::Error(e) = &t.state {
                        ui.colored_label(theme::DANGER, egui::RichText::new("Ocurrió un error, intente de nuevo").size(11.0));
                    }
                }
            });
        });
}