use crate::device::{self, DeviceInfo};
use crate::watcher::{self, DeviceEvent, DeviceSummary};
use crate::wifi::WifiManager;
use crate::files::FileManager;
use crate::stream::StreamServer;
use crate::ui;
use eframe::egui;
use std::time::Duration;
use std::sync::mpsc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Pantalla,
    Archivos,
    Conexion,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Inactiva,
    EnVivo,
    Cerrada,
}

pub struct TransfelApp {
    pub device: DeviceInfo,
    pub status: String,
    pub tab: Tab,

    pub devices: Vec<DeviceSummary>,
    pub selected: Option<String>,
    pub wifi: WifiManager,

    pub device_events: mpsc::Receiver<DeviceEvent>,
    info_tx: mpsc::Sender<Result<DeviceInfo, String>>,
    info_rx: mpsc::Receiver<Result<DeviceInfo, String>>,

    pub stream: StreamServer,
    pub stream_state: StreamState,
    pub texture: Option<egui::TextureHandle>,

    pub files: FileManager,
}

impl TransfelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let (info_tx, info_rx) = mpsc::channel();

        let app = Self {
            device: DeviceInfo::disconnected(),
            status: "Buscando dispositivo...".to_string(),
            tab: Tab::Pantalla,

            devices: Vec::new(),
            selected: None,
            wifi: WifiManager::default(), 

            stream: StreamServer::start(720, 1280),
            stream_state: StreamState::Inactiva,
            texture: None,

            files: FileManager::default(),

            device_events: watcher::spawn(),
            info_tx,
            info_rx,
        };

        app
    }

    pub fn detect_device(&mut self) {
        self.status = "Buscando dispositivo...".to_string();
        device::detect_async(self.info_tx.clone());
    }

    fn poll_device(&mut self) {
        while let Ok(DeviceEvent::Devices(list)) = self.device_events.try_recv() {
            self.devices = list;

            // ¿Sigue vivo el seleccionado?
            let still_there = self
                .selected
                .as_ref()
                .map(|s| self.devices.iter().any(|d| &d.serial == s && d.ready()))
                .unwrap_or(false);

            if !still_there {
                // Preferimos WiFi si esta disponible: sobrevive a que quiten el cable.
                let next = self
                    .devices
                    .iter()
                    .find(|d| d.ready() && d.Wifi)
                    .or_else(|| self.devices.iter().find(|d| d.ready()))
                    .map(|d| d.serial.clone());

                match next {
                    Some(serial) => self.select_device(serial),
                    None => {
                        self.selected = None;
                        self.device = DeviceInfo::disconnected();
                        self.texture = None;
                        self.status = "Dispositivo desconectado".to_string();
                        self.files.invalidate();
                        if self.files.side == crate::files::Side::Android {
                            self.files.entries.clear();
                        }
                    }
                }
            }
        }

        while let Ok(result) = self.info_rx.try_recv() {
            match result {
                Ok(info) => {
                    self.stream.set_resolution(info.width, info.height);
                    self.status = if info.serial.as_deref().is_some_and(|s| s.contains(':')) {
                        "Conectado por WiFi".to_string()
                    } else {
                        "Conectado por USB".to_string()
                    };
                    let serial = info.serial.clone();
                    self.device = info;
                    self.files.invalidate();
                    if self.files.side == crate::files::Side::Android {
                        self.files.reload(serial.as_deref());
                    }
                }
                Err(msg) => {
                    self.device = DeviceInfo::disconnected();
                    self.status = msg;
                }
            }
        }
    }

    pub fn select_device(&mut self, serial: String) {
        self.selected = Some(serial.clone());
        self.status = "Leyendo dispositivo...".to_string();
        device::info_async(serial, self.info_tx.clone());
    }

    pub fn is_wifi(&self) -> bool {
        self.device.serial.as_deref().is_some_and(|s| s.contains(':'))
    }

    pub fn serial(&self) -> Option<&str> {
        self.device.serial.as_deref()
    }

    fn update_video(&mut self, ctx: &egui::Context) {
        let live = self.stream.is_live();

        if live {
            self.stream_state = StreamState::EnVivo;
        } else if self.stream_state == StreamState::EnVivo {
            // La app del celular cerro la conexion.
            self.stream_state = StreamState::Cerrada;
            self.texture = None;
            self.stream.drain();
            self.status = "Transmision finalizada".to_string();
            return;
        }

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
            none => {
                *none = Some(ctx.load_texture(
                    "android_screen",
                    image,
                    egui::TextureOptions::LINEAR,
                ))
            }
        }
    }
}

impl eframe::App for TransfelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_device();
        self.wifi.poll();
        self.update_video(&ctx);

        let serial = self.device.serial.clone();
        self.files.poll(serial.as_deref());


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
                Tab::Archivos => ui::files_view::show(self, ui),
                Tab::Conexion => ui::wifi_view::show(self, ui),
            });

        // En vivo  60 fps. Si no, tick lento que igual detecta desconexiones.
        if self.stream_state == StreamState::EnVivo || self.files.active_transfers() > 0 || self.files.loading {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}