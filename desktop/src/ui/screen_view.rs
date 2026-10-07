    use crate::app::{StreamState, TransfelApp};
    use crate::stream;
    use crate::theme;
    use eframe::egui;

    pub fn show(app: &mut TransfelApp, ui: &mut egui::Ui) {
        //encabezado

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Pantalla del dispositivo").size(20.0).strong());
            
            if app.texture.is_some() {
                let (w, h) = app.stream.content_size();
                ui.add_space(10.0);
                ui.colored_label(
                    theme::MUTED,
                    format!(
                        "{}x{} | {}",
                        w,
                        h,
                        if w > h { "horizontal" } else { "vertical" }
                    ),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match app.stream_state {
                    StreamState::EnVivo => { ui.colored_label(theme::SUCCESS, "● EN VIVO"); }
                    StreamState::Cerrada => { ui.colored_label(theme::DANGER, "● DESCONECTADO"); }
                    StreamState::Inactiva => {}
                }

                if app.stream_state == StreamState::EnVivo {
                    ui.add_space(10.0);

                    // Botones de navegacion, con los simbolos de Android.
                    if app.control_activo {
                        if boton_nav(ui, Nav::Recientes).on_hover_text("Recientes").clicked() {
                            app.stream.enviar_accion(stream::ACCION_RECIENTES);
                        }
                        if boton_nav(ui, Nav::Inicio).on_hover_text("Inicio").clicked() {
                            app.stream.enviar_accion(stream::ACCION_INICIO);
                        }
                        if boton_nav(ui, Nav::Atras).on_hover_text("Atras").clicked() {
                            app.stream.enviar_accion(stream::ACCION_ATRAS);
                        }
                        ui.add_space(6.0);
                    }

                    ui.checkbox(&mut app.control_activo, "\u{1f5b1} Control")
                        .on_hover_text(
                            "Controlar el telefono con el raton. Requiere activar TransFEL \
                             en Ajustes de Accesibilidad del telefono.",
                        );
                }
            });
        });

        ui.add_space(12.0);

        let area = ui.available_rect_before_wrap();
        if area.width() < 40.0 || area.height() < 40.0 {
            return;
        }

        // El video llega cuadrado; la pantalla real ocupa solo una zona.
        let (cw, ch) = app.stream.content_size();
        let (cw, ch) = (cw.max(1) as f32, ch.max(1) as f32);

        let (tw, th) = if app.texture.is_some() {
            (cw, ch)
        } else {
            (
                app.device.width.max(1) as f32,
                app.device.height.max(1) as f32,
            )
        };


        let escala = (area.width() / tw).min(area.height()/th);
        let tamano = egui::vec2(tw * escala, th * escala);
        let marco = egui::Rect::from_center_size(area.center(), tamano);


        let painter = ui.painter_at(area);
        let radio = 12.0;

        painter.rect_filled(marco, radio, theme::VIDEO_BG);
        painter.rect_stroke(
            marco,
            radio,
            egui::Stroke::new(
                1.0,
                if app.stream_state == StreamState::EnVivo { theme::ACCENT} else {theme::BORDER},
            ),
            egui::StrokeKind::Inside,
        );

        match &app.texture {
            Some(tex) => {
                // AUTO_MIRROR centra la pantalla dentro del cuadrado conservando
                let lado = tex.size_vec2();
                let encaje = (lado.x / cw).min(lado.y / ch);
                let visible = egui::vec2(cw * encaje, ch * encaje);

                let margen_x = ((lado.x - visible.x) / 2.0 / lado.x).clamp(0.0, 0.5);
                let margen_y = ((lado.y - visible.y) / 2.0 / lado.y).clamp(0.0, 0.5);

                let uv = egui::Rect::from_min_max(
                    egui::pos2(margen_x, margen_y),
                    egui::pos2(1.0 - margen_x, 1.0 - margen_y),
                );
                painter.image(tex.id(), marco.shrink(1.0), uv, egui::Color32::WHITE);

                if app.control_activo {
                    manejar_raton(app, ui, marco, cw, ch);
                }
            }
            None => {
                placeholder(app, ui, &painter, marco);
            }
        }
        ui.allocate_rect(area, egui::Sense::hover());
    }

    pub fn a_coordenadas_dispositivo(
    punto: egui::Pos2,
    marco: egui::Rect,
    ancho: usize,
    alto: usize,
) -> Option<(i32, i32)> {
    if !marco.contains(punto) {
        return None;
    }
    let rel_x = (punto.x - marco.min.x) / marco.width();
    let rel_y = (punto.y - marco.min.y) / marco.height();
    Some((
        (rel_x * ancho as f32).round() as i32,
        (rel_y * alto as f32).round() as i32,
    ))
}

fn placeholder(
    app: &mut TransfelApp,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    marco: egui::Rect,
) {
    let cerrada = app.stream_state == StreamState::Cerrada;

    let (icono, titulo, sub, sub2, color) = if cerrada {
        (
            "\u{26cc}",
            "Conexion cerrada",
            "La app del celular finalizó la transmision",
            "",
            theme::DANGER,
        )
    } else {
        (
            "\u{25a3}",
            "Pantalla Android",
            "Inicia la transmisión desde la app movil",
            "Conectado por USB la transmisión es mas fluida",
            theme::MUTED,
        )
    };

    // Nada puede salirse del marco.
    let p = painter.with_clip_rect(marco);
    let centro = marco.center();

    // Ancho util para el texto, con margen a los lados.
    let ancho = (marco.width() - 32.0).max(80.0);
    let compacto = marco.height() < 260.0;

    // Icono (se omite si la tarjeta es muy baja).
    if !compacto {
        p.text(
            centro - egui::vec2(0.0, 62.0),
            egui::Align2::CENTER_CENTER,
            icono,
            egui::FontId::proportional(40.0),
            color,
        );
    }

    let mut y = centro.y - if compacto { 18.0 } else { 14.0 };

    // Titulo, ajustado si no cabe.
    let tam_titulo = if marco.width() < 260.0 { 16.0 } else { 21.0 };
    let g = p.layout(
        titulo.to_string(),
        egui::FontId::proportional(tam_titulo),
        color,
        ancho,
    );
    p.galley(
        egui::pos2(centro.x - g.size().x / 2.0, y),
        g.clone(),
        color,
    );
    y += g.size().y + 8.0;

    // Subtitulos, con salto de linea dentro del marco.
    for texto in [sub, sub2] {
        if texto.is_empty() {
            continue;
        }
        let g = p.layout(
            texto.to_string(),
            egui::FontId::proportional(13.0),
            theme::MUTED,
            ancho,
        );
        if y + g.size().y > marco.max.y - 12.0 {
            break;
        }
        p.galley(
            egui::pos2(centro.x - g.size().x / 2.0, y),
            g.clone(),
            theme::MUTED,
        );
        y += g.size().y + 6.0;
    }

    if cerrada {
        let ancho_boton = 200.0_f32.min(marco.width() - 32.0);
        let boton = egui::Rect::from_center_size(
            egui::pos2(centro.x, y + 22.0),
            egui::vec2(ancho_boton, 32.0),
        );
        if marco.contains_rect(boton)
            && ui.put(boton, egui::Button::new("Esperar nueva conexion")).clicked()
        {
            app.stream_state = StreamState::Inactiva;
        }
    }
}

/// Traduce el raton sobre la card en eventos tactiles para el telefono.
fn manejar_raton(
    app: &mut TransfelApp,
    ui: &mut egui::Ui,
    marco: egui::Rect,
    ancho: f32,
    alto: f32,
) {
    let respuesta = ui.interact(
        marco,
        ui.id().with("pantalla_android"),
        egui::Sense::click_and_drag(),
    );

    if respuesta.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let punto_a_dispositivo = |punto: egui::Pos2| -> (i32, i32) {
        let rel_x = ((punto.x - marco.min.x) / marco.width()).clamp(0.0, 1.0);
        let rel_y = ((punto.y - marco.min.y) / marco.height()).clamp(0.0, 1.0);
        (
            (rel_x * ancho).round() as i32,
            (rel_y * alto).round() as i32,
        )
    };

    // ── Rueda del raton ──
    let mut soltado = false;

    if respuesta.hovered() {
        let desplazamiento = ui.input(|i| i.smooth_scroll_delta.y);

        if desplazamiento.abs() > 0.5 {
            if let Some(punto) = ui.input(|i| i.pointer.hover_pos()) {
                let (x, y) = punto_a_dispositivo(punto);

                // El signo se invierte: rueda hacia arriba mueve el dedo hacia abajo.
                let delta_px = (desplazamiento * 4.0).clamp(-alto * 0.5, alto * 0.5);
                app.stream.enviar_rueda(x, y, delta_px.round() as i32);
            }
        }
    }

    // ── Toque y arrastre ──
    let pulsado = respuesta.is_pointer_button_down_on();

    let punto_actual = respuesta
        .interact_pointer_pos()
        .or_else(|| ui.input(|i| i.pointer.hover_pos()))
        .map(punto_a_dispositivo);

    match (pulsado, app.ultimo_toque) {
        // Se acaba de pulsar.
        (true, None) => {
            if let Some((x, y)) = punto_actual {
                app.ultimo_toque = Some((x, y));
                app.stream.enviar_toque(stream::CTRL_ABAJO, x, y);
            }
        }

        // Sigue pulsado: solo se avisa si de verdad se movio.
        (true, Some(anterior)) => {
            if let Some((x, y)) = punto_actual {
                if (x, y) != anterior {
                    app.ultimo_toque = Some((x, y));
                    app.stream.enviar_toque(stream::CTRL_MOVER, x, y);
                }
            }
        }

        // Se solto.
        (false, Some(anterior)) => {
            let (x, y) = punto_actual.unwrap_or(anterior);
            app.stream.enviar_toque(stream::CTRL_ARRIBA, x, y);
            app.ultimo_toque = None;
        }

        (false, None) => {}
    }
}

/// Los tres botones de navegacion de Android.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Atras,
    Inicio,
    Recientes,
}

/// Dibuja el icono en vez de usar un glifo: las fuentes que trae egui no
/// incluyen todos los simbolos geometricos y algunos salian en blanco.
fn boton_nav(ui: &mut egui::Ui, forma: Nav) -> egui::Response {
    let (rect, respuesta) =
        ui.allocate_exact_size(egui::vec2(30.0, 24.0), egui::Sense::click());

    let encima = respuesta.hovered();
    let trazo = if encima {
        theme::ACCENT
    } else {
        egui::Color32::from_rgb(198, 198, 208)
    };

    let painter = ui.painter();
    painter.rect_filled(
        rect,
        6.0,
        if encima { theme::CARD_HOVER } else { theme::CARD },
    );

    let centro = rect.center();
    let radio = 5.0_f32;
    let grosor = egui::Stroke::new(1.7, trazo);

    match forma {
        Nav::Atras => {
            // Triangulo relleno apuntando a la izquierda.
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(centro.x - radio, centro.y),
                    egui::pos2(centro.x + radio * 0.75, centro.y - radio),
                    egui::pos2(centro.x + radio * 0.75, centro.y + radio),
                ],
                trazo,
                egui::Stroke::NONE,
            ));
        }
        Nav::Inicio => {
            painter.circle_stroke(centro, radio, grosor);
        }
        Nav::Recientes => {
            painter.rect_stroke(
                egui::Rect::from_center_size(centro, egui::vec2(radio * 1.9, radio * 1.9)),
                1.5,
                grosor,
                egui::StrokeKind::Inside,
            );
        }
    }

    respuesta
}
