    use crate::app::{StreamState, TransfelApp};
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
                // proporcion: calculamos esa zona y mostramos solo esa.
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

                let _respuesta = ui.interact(
                    marco, 
                    ui.id().with("pantalla_android"),
                    egui::Sense::click_and_drag(),
                );
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
            "La app del celular finalizo la transmision",
            "",
            theme::DANGER,
        )
    } else {
        (
            "\u{25a3}",
            "Pantalla Android",
            "Inicia la transmision desde la app movil",
            "Conectado por USB la transmision es mas fluida",
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
