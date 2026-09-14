use crate::device::DeviceInfo;
use crate::files::FileManager;
use crate::stream::StreamServer;
use crate::ui;
use eframe::egui;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Pantalla,
    Control,
    Archivos,
}

pub struct TransfelApp {
    pub device: DeviceInfo,
    pub status: String,
    pub tab: Tab,

    pub stream: StreamServer,
    pub texture: Option<egui::TextureHandle>,

    pub files: FileManager,
}

impl TransfelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut app = Self {
            device: DeviceInfo::disconnected(),
            status: "Buscando dispositivo...".into(),
            tab: Tab::Pantalla,
            stream: StreamServer::start(720, 1280),
            texture: None,
            files: FileManager::default(),
        };

        app.detect_device();
        app
    }

    pub fn detect_device(&mut self) {
        match DeviceInfo::detect() {
            Ok(info) => {
                self.stream.set_resolution(info.width, info.height);
                self.status = "Dispositivo conectado".into();
                self.device = info;

                // refresca el listado del celular si estamos en esa vista
                if let Some(serial) = self.device.serial.clone() {
                    self.files.refresh_android(&serial);
                }
            }
            Err(msg) => {
                self.device = DeviceInfo::disconnected();
                self.texture = None;
                self.status = msg;
            }
        }
    }

    pub fn serial(&self) -> Option<&str> {
        self.device.serial.as_deref()
    }

    fn update_video(&mut self, ctx: &egui::Context) {
        let Some(frame) = self.stream.latest_frame() else {
            return;
        };
        let (w, h) = (self.device.width, self.device.height);
        if frame.len() != w * h * 4 {
            return;
        }

        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &frame);
        match &mut self.texture {
            Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
            none => *none = Some(ctx.load_texture("android_screen", image, egui::TextureOptions::LINEAR)),
        }
    }
}

impl eframe::App for TransfelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.update_video(&ctx);
        let transfers_changed = self.files.poll();

        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, crate::theme::BACKGROUND);

        ui::top_bar::show(self, ui);
        ui::sidebar::show(self, ui);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::BACKGROUND)
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ui, |ui| match self.tab {
                Tab::Pantalla => ui::screen_view::show(self, ui),
                Tab::Control => ui::screen_view::show_control_placeholder(ui),
                Tab::Archivos => ui::files_view::show(self, ui),
            });

        // Solo repinta rapido cuando hace falta (ahorra CPU/bateria).
        let busy = self.texture.is_some() || self.files.active_transfers() > 0 || transfers_changed;
        if busy {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}