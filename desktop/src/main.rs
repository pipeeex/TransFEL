use eframe::egui;
use std::io::Read;
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> eframe::Result<()> {

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TransFEL")
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "TransFEL",
        options,
        Box::new(|_cc| {
            Ok(Box::new(TransfelApp::new()))
        }),
    )
}

struct TransfelApp {

    device: Option<String>,

    model: String,

    android: String,

    resolution: String,

    density: String,

    status: String,

    streaming: bool,

    frames_received: Arc<Mutex<u64>>,
}

impl TransfelApp {

    fn new() -> Self {

        let frames_received =
            Arc::new(Mutex::new(0));

        let mut app = Self {

            device: None,

            model: "-".to_string(),

            android: "-".to_string(),

            resolution: "-".to_string(),

            density: "-".to_string(),

            status:
                "Buscando dispositivo..."
                    .to_string(),

            streaming: false,

            frames_received:
                Arc::clone(
                    &frames_received
                ),
        };

        app.detect_device();

        app.start_stream_server(
            frames_received
        );

        app
    }

    fn adb_command(
        &self,
        serial: &str,
        command: &str,
    ) -> Option<String> {

        let output =
            Command::new("adb")
                .args([
                    "-s",
                    serial,
                    "shell",
                    command,
                ])
                .output()
                .ok()?;

        if !output.status.success() {
            return None;
        }

        Some(
            String::from_utf8_lossy(
                &output.stdout
            )
            .trim()
            .to_string()
        )
    }

    fn detect_device(&mut self) {

        let output =
            Command::new("adb")
                .args(["devices"])
                .output();

        match output {

            Ok(output) => {

                let text =
                    String::from_utf8_lossy(
                        &output.stdout
                    );

                for line in text
                    .lines()
                    .skip(1)
                {

                    let parts:
                        Vec<&str> =
                        line
                            .split_whitespace()
                            .collect();

                    if parts.len() >= 2
                        && parts[1] == "device"
                    {

                        let serial =
                            parts[0]
                                .to_string();

                        self.device =
                            Some(
                                serial.clone()
                            );

                        self.status =
                            "Dispositivo conectado"
                                .to_string();

                        self.get_device_info(
                            &serial
                        );

                        return;
                    }
                }

                self.clear_device();

                self.status =
                    "No hay dispositivos conectados"
                        .to_string();
            }

            Err(_) => {

                self.clear_device();

                self.status =
                    "No se encontro ADB"
                        .to_string();
            }
        }
    }

    fn clear_device(&mut self) {

        self.device = None;

        self.model =
            "-".to_string();

        self.android =
            "-".to_string();

        self.resolution =
            "-".to_string();

        self.density =
            "-".to_string();
    }

    fn get_device_info(
        &mut self,
        serial: &str,
    ) {

        if let Some(model) =
            self.adb_command(
                serial,
                "getprop ro.product.model",
            )
        {
            self.model = model;
        }

        if let Some(version) =
            self.adb_command(
                serial,
                "getprop ro.build.version.release",
            )
        {
            self.android = version;
        }

        if let Some(size) =
            self.adb_command(
                serial,
                "wm size",
            )
        {
            self.resolution =
                size
                    .replace(
                        "Physical size:",
                        ""
                    )
                    .trim()
                    .to_string();
        }

        if let Some(density) =
            self.adb_command(
                serial,
                "wm density",
            )
        {
            self.density =
                density
                    .replace(
                        "Physical density:",
                        ""
                    )
                    .trim()
                    .to_string();
        }

        self.setup_reverse(serial);
    }

    fn setup_reverse(
        &self,
        serial: &str,
    ) {

        let result =
            Command::new("adb")
                .args([
                    "-s",
                    serial,
                    "reverse",
                    "tcp:5000",
                    "tcp:5000",
                ])
                .output();

        match result {

            Ok(output) => {

                if output.status.success() {

                    println!(
                        "ADB reverse configurado"
                    );

                } else {

                    println!(
                        "No se pudo configurar ADB reverse"
                    );
                }
            }

            Err(error) => {

                println!(
                    "Error ADB reverse: {}",
                    error
                );
            }
        }
    }

    fn start_stream_server(
        &mut self,
        frames_received:
            Arc<Mutex<u64>>,
    ) {

        if self.streaming {
            return;
        }

        self.streaming = true;

        thread::spawn(move || {

            let listener =
                match TcpListener::bind(
                    "127.0.0.1:5000"
                ) {

                    Ok(listener) => listener,

                    Err(error) => {

                        println!(
                            "ERROR PUERTO 5000: {}",
                            error
                        );

                        return;
                    }
                };

            println!(
                "Servidor TransFEL escuchando en 5000"
            );

            for connection
                in listener.incoming()
            {

                match connection {

                    Ok(mut stream) => {

                        println!(
                            "Android conectado"
                        );

                        loop {

                            let mut size_buffer =
                                [0u8; 4];

                            if stream
                                .read_exact(
                                    &mut size_buffer
                                )
                                .is_err()
                            {
                                println!(
                                    "Android desconectado"
                                );

                                break;
                            }

                            let size =
                                u32::from_be_bytes(
                                    size_buffer
                                ) as usize;

                            if size == 0 {

                                println!(
                                    "Frame vacio recibido"
                                );

                                continue;
                            }

                            if size >
                                20_000_000
                            {

                                println!(
                                    "Frame demasiado grande: {} bytes",
                                    size
                                );

                                break;
                            }

                            let mut frame =
                                vec![0u8; size];

                            if stream
                                .read_exact(
                                    &mut frame
                                )
                                .is_err()
                            {
                                println!(
                                    "Error leyendo frame"
                                );

                                break;
                            }

                            let mut contador =
                                frames_received
                                    .lock()
                                    .unwrap();

                            *contador += 1;

                            let numero =
                                *contador;

                            drop(contador);

                            if numero == 1 {

                                println!(
                                    "PRIMER FRAME H264 RECIBIDO: {} bytes",
                                    size
                                );

                            }

                            if numero % 30 == 0 {

                                println!(
                                    "Frames H264 recibidos: {} | ultimo: {} bytes",
                                    numero,
                                    size
                                );
                            }
                        }
                    }

                    Err(error) => {

                        println!(
                            "Error TCP: {}",
                            error
                        );
                    }
                }
            }
        });
    }
}

impl eframe::App for TransfelApp {

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
    ) {

        ui.heading("TransFEL");

        ui.separator();

        ui.horizontal(|ui| {

            if ui
                .button(
                    "Buscar dispositivo"
                )
                .clicked()
            {

                self.detect_device();
            }

            ui.label(
                &self.status
            );
        });

        ui.add_space(20.0);

        if self.device.is_some() {

            ui.horizontal(|ui| {

                ui.vertical(|ui| {

                    ui.set_width(250.0);

                    ui.group(|ui| {

                        ui.heading(
                            "Dispositivo"
                        );

                        ui.add_space(10.0);

                        ui.label(
                            format!(
                                "Modelo: {}",
                                self.model
                            )
                        );

                        ui.label(
                            format!(
                                "Android: {}",
                                self.android
                            )
                        );

                        ui.label(
                            format!(
                                "Resolucion: {}",
                                self.resolution
                            )
                        );

                        ui.label(
                            format!(
                                "Densidad: {}",
                                self.density
                            )
                        );

                        ui.add_space(10.0);

                        ui.colored_label(
                            egui::Color32::GREEN,
                            "Conectado por USB",
                        );
                    });

                    ui.add_space(20.0);

                    if ui
                        .button("Pantalla")
                        .clicked()
                    {
                    }

                    if ui
                        .button("Control")
                        .clicked()
                    {
                    }

                    if ui
                        .button("Archivos")
                        .clicked()
                    {
                    }
                });

                ui.add_space(20.0);

                ui.vertical(|ui| {

                    ui.heading(
                        "Vista del dispositivo"
                    );

                    ui.add_space(10.0);

                    egui::Frame::group(
                        ui.style()
                    )
                    .show(
                        ui,
                        |ui| {

                            ui.set_min_size(
                                egui::vec2(
                                    600.0,
                                    500.0
                                )
                            );

                            ui.vertical_centered(
                                |ui| {

                                    ui.add_space(
                                        180.0
                                    );

                                    ui.heading(
                                        "Pantalla Android"
                                    );

                                    let frames =
                                        *self
                                            .frames_received
                                            .lock()
                                            .unwrap();

                                    ui.label(
                                        format!(
                                            "Frames H264 recibidos: {}",
                                            frames
                                        )
                                    );

                                    if frames > 0 {

                                        ui.label(
                                            "Transmision funcionando"
                                        );

                                    } else {

                                        ui.label(
                                            "Esperando frames..."
                                        );
                                    }
                                }
                            );
                        }
                    );
                });
            });

        } else {

            ui.vertical_centered(
                |ui| {

                    ui.add_space(
                        180.0
                    );

                    ui.heading(
                        "TransFEL"
                    );

                    ui.add_space(
                        15.0
                    );

                    ui.label(
                        &self.status
                    );

                    ui.add_space(
                        10.0
                    );

                    ui.label(
                        "Conecta un Android con depuracion USB activada."
                    );
                }
            );
        }

        ui.ctx().request_repaint();
    }
}