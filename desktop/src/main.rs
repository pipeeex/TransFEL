use eframe::egui;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TransFEL")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "TransFEL",
        options,
        Box::new(|_cc| Ok(Box::new(TransfelApp::new()))),
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

    frame_receiver: mpsc::Receiver<Vec<u8>>,
    frame_sender: mpsc::SyncSender<Vec<u8>>,

    texture: Option<egui::TextureHandle>,

    video_width: usize,
    video_height: usize,
}

impl TransfelApp {
    fn new() -> Self {
        let frames_received =
            Arc::new(Mutex::new(0));

        let (frame_sender, frame_receiver) =
            mpsc::sync_channel::<Vec<u8>>(1);

        let resolution =
            Arc::new(
                Mutex::new(
                    (720usize, 1280usize)
                )
            );

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

            frame_receiver,

            frame_sender:
                frame_sender.clone(),

            texture: None,

            video_width: 720,
            video_height: 1280,
        };

        app.detect_device(
            &resolution
        );

        app.start_stream_server(
            frames_received,
            frame_sender,
            resolution,
        );

        app
    }

    fn ffmpeg_path() -> PathBuf {
        let exe =
            std::env::current_exe()
                .unwrap_or_else(|_| {
                    PathBuf::from(".")
                });

        let exe_dir =
            exe.parent()
                .unwrap_or_else(|| {
                    std::path::Path::new(".")
                });

        let installed_path =
            exe_dir
                .join("tools")
                .join("ffmpeg.exe");

        if installed_path.exists() {
            return installed_path;
        }

        let development_path =
            exe_dir
                .parent()
                .and_then(|p| p.parent())
                .map(|p| {
                    p.join("tools")
                        .join("ffmpeg.exe")
                });

        if let Some(path) =
            development_path
        {
            if path.exists() {
                return path;
            }
        }

        PathBuf::from(
            "ffmpeg.exe"
        )
    }

    fn adb_path() -> PathBuf {
        let exe =
            std::env::current_exe()
                .unwrap_or_else(|_| {
                    PathBuf::from(".")
                });

        let exe_dir =
            exe.parent()
                .unwrap_or_else(|| {
                    std::path::Path::new(".")
                });

        let installed_path =
            exe_dir
                .join("tools")
                .join("adb.exe");

        if installed_path.exists() {
            return installed_path;
        }

        PathBuf::from(
            "adb.exe"
        )
    }

    fn adb_command(
        &self,
        serial: &str,
        command: &str,
    ) -> Option<String> {

        let adb =
            Self::adb_path();

        let output =
            Command::new(adb)
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

    fn detect_device(
        &mut self,
        resolution_shared:
            &Arc<Mutex<(usize, usize)>>,
    ) {

        let adb =
            Self::adb_path();

        let output =
            Command::new(adb)
                .args([
                    "devices"
                ])
                .output();

        match output {

            Ok(output) => {

                let text =
                    String::from_utf8_lossy(
                        &output.stdout
                    );

                for line in
                    text.lines().skip(1)
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
                            &serial,
                            resolution_shared,
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

    fn clear_device(
        &mut self
    ) {

        self.device =
            None;

        self.model =
            "-".to_string();

        self.android =
            "-".to_string();

        self.resolution =
            "-".to_string();

        self.density =
            "-".to_string();

        self.texture =
            None;
    }

    fn get_device_info(
        &mut self,
        serial: &str,
        resolution_shared:
            &Arc<Mutex<(usize, usize)>>,
    ) {

        if let Some(model) =
            self.adb_command(
                serial,
                "getprop ro.product.model"
            )
        {
            self.model =
                model;
        }

        if let Some(version) =
            self.adb_command(
                serial,
                "getprop ro.build.version.release"
            )
        {
            self.android =
                version;
        }

        if let Some(size) =
            self.adb_command(
                serial,
                "wm size"
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

            if let Some(
                (width, height)
            ) =
                parse_resolution(
                    &self.resolution
                )
            {

                self.video_width =
                    width;

                self.video_height =
                    height;

                if let Ok(
                    mut shared
                ) =
                    resolution_shared.lock()
                {

                    *shared =
                        (
                            width,
                            height
                        );
                }
            }
        }

        if let Some(density) =
            self.adb_command(
                serial,
                "wm density"
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

        self.setup_reverse(
            serial
        );
    }

    fn setup_reverse(
        &self,
        serial: &str,
    ) {

        let adb =
            Self::adb_path();

        let result =
            Command::new(adb)
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
        frame_sender:
            mpsc::SyncSender<Vec<u8>>,
        resolution:
            Arc<Mutex<(usize, usize)>>,
    ) {

        if self.streaming {
            return;
        }

        self.streaming =
            true;

        thread::spawn(
            move || {

                let listener =
                    match TcpListener::bind(
                        "127.0.0.1:5000"
                    ) {

                        Ok(listener) =>
                            listener,

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

                        Ok(stream) => {

                            println!(
                                "Android conectado"
                            );

                            let current_resolution =
                                resolution
                                    .lock()
                                    .map(|r| *r)
                                    .unwrap_or(
                                        (720, 1280)
                                    );

                            let sender =
                                frame_sender
                                    .clone();

                            let counter =
                                Arc::clone(
                                    &frames_received
                                );

                            thread::spawn(
                                move || {

                                    handle_connection(
                                        stream,
                                        sender,
                                        counter,
                                        current_resolution,
                                    );
                                }
                            );
                        }

                        Err(error) => {

                            println!(
                                "Error TCP: {}",
                                error
                            );
                        }
                    }
                }
            }
        );
    }
}

impl eframe::App for TransfelApp {

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
    ) {

        /*
         * Tomamos solamente el frame
         * mas reciente.
         */

        let mut latest_frame:
            Option<Vec<u8>> = None;

        while let Ok(frame) =
            self.frame_receiver.try_recv()
        {

            latest_frame =
                Some(frame);
        }

        /*
         * Actualizamos la textura.
         */

        if let Some(frame) =
            latest_frame
        {

            let expected =
                self.video_width
                    * self.video_height
                    * 4;

            if frame.len() ==
                expected
            {

                let image =
                    egui::ColorImage
                        ::from_rgba_unmultiplied(
                            [
                                self.video_width,
                                self.video_height,
                            ],
                            &frame,
                        );

                if let Some(texture) =
                    &mut self.texture
                {

                    texture.set(
                        image,
                        egui::TextureOptions::LINEAR,
                    );

                } else {

                    self.texture =
                        Some(
                            ui.ctx()
                                .load_texture(
                                    "android_screen",
                                    image,
                                    egui::TextureOptions::LINEAR,
                                )
                        );
                }
            }
        }

        /*
         * Encabezado.
         */

        ui.heading(
            "TransFEL"
        );

        ui.separator();

        ui.horizontal(
            |ui| {

                if ui
                    .button(
                        "Buscar dispositivo"
                    )
                    .clicked()
                {

                    let resolution =
                        Arc::new(
                            Mutex::new(
                                (
                                    self.video_width,
                                    self.video_height,
                                )
                            )
                        );

                    self.detect_device(
                        &resolution
                    );
                }

                ui.label(
                    &self.status
                );
            }
        );

        ui.add_space(
            10.0
        );

        /*
         * Si existe un dispositivo.
         */

        if self.device.is_some() {

            let available =
                ui.available_size();

            /*
             * Panel izquierdo dinamico.
             */

            let left_width =
                (
                    available.x * 0.20
                )
                .clamp(
                    200.0,
                    280.0
                );

            let spacing =
                15.0;

            let right_width =
                (
                    available.x
                    - left_width
                    - spacing
                )
                .max(
                    300.0
                );

            /*
             * Contenedor principal.
             */

            ui.horizontal(
                |ui| {

                    /*
                     * PANEL IZQUIERDO
                     */

                    ui.allocate_ui_with_layout(
                        egui::vec2(
                            left_width,
                            available.y
                        ),
                        egui::Layout::top_down(
                            egui::Align::LEFT
                        ),
                        |ui| {

                            ui.group(
                                |ui| {

                                    ui.heading(
                                        "Dispositivo"
                                    );

                                    ui.add_space(
                                        8.0
                                    );

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

                                    ui.add_space(
                                        8.0
                                    );

                                    ui.colored_label(
                                        egui::Color32::GREEN,
                                        "Conectado por USB"
                                    );
                                }
                            );

                            ui.add_space(
                                15.0
                            );

                            ui.group(
                                |ui| {

                                    ui.heading(
                                        "Funciones"
                                    );

                                    ui.add_space(
                                        8.0
                                    );

                                    ui.add_sized(
                                        [
                                            left_width - 20.0,
                                            30.0
                                        ],
                                        egui::Button::new(
                                            "Pantalla"
                                        )
                                    );

                                    ui.add_space(
                                        5.0
                                    );

                                    ui.add_sized(
                                        [
                                            left_width - 20.0,
                                            30.0
                                        ],
                                        egui::Button::new(
                                            "Control"
                                        )
                                    );

                                    ui.add_space(
                                        5.0
                                    );

                                    ui.add_sized(
                                        [
                                            left_width - 20.0,
                                            30.0
                                        ],
                                        egui::Button::new(
                                            "Archivos"
                                        )
                                    );
                                }
                            );

                            ui.add_space(
                                15.0
                            );

                            let frames =
                                *self
                                    .frames_received
                                    .lock()
                                    .unwrap();

                            ui.label(
                                format!(
                                    "Frames recibidos: {}",
                                    frames
                                )
                            );
                        }
                    );

                    ui.add_space(
                        spacing
                    );

                    /*
                     * PANEL DERECHO
                     */

                    ui.allocate_ui_with_layout(
                        egui::vec2(
                            right_width,
                            available.y
                        ),
                        egui::Layout::top_down(
                            egui::Align::Center
                        ),
                        |ui| {

                            ui.heading(
                                "Vista del dispositivo"
                            );

                            ui.add_space(
                                8.0
                            );

                            /*
                             * Area disponible para
                             * la pantalla.
                             */

                            let video_area =
                                ui.available_size();

                            egui::Frame::group(
                                ui.style()
                            )
                            .show(
                                ui,
                                |ui| {

                                    let area =
                                        ui.available_size();

                                    /*
                                     * Si tenemos video.
                                     */

                                    if let Some(
                                        texture
                                    ) =
                                        &self.texture
                                    {

                                        let texture_size =
                                            texture
                                                .size_vec2();

                                        /*
                                         * Calculamos el
                                         * maximo tamaño
                                         * posible.
                                         */

                                        let scale_x =
                                            area.x
                                                / texture_size.x;

                                        let scale_y =
                                            area.y
                                                / texture_size.y;

                                        let scale =
                                            scale_x
                                                .min(scale_y);

                                        let display_size =
                                            texture_size
                                                * scale;

                                        /*
                                         * Centramos
                                         * verticalmente.
                                         */

                                        let top_space =
                                            (
                                                area.y
                                                - display_size.y
                                            )
                                            .max(
                                                0.0
                                            )
                                            / 2.0;

                                        ui.add_space(
                                            top_space
                                        );

                                        /*
                                         * Centramos
                                         * horizontalmente.
                                         */

                                        ui.horizontal(
                                            |ui| {

                                                let left_space =
                                                    (
                                                        area.x
                                                        - display_size.x
                                                    )
                                                    .max(
                                                        0.0
                                                    )
                                                    / 2.0;

                                                ui.add_space(
                                                    left_space
                                                );

                                                ui.image(
                                                    (
                                                        texture.id(),
                                                        display_size,
                                                    )
                                                );
                                            }
                                        );

                                    } else {

                                        /*
                                         * No hay video
                                         * todavia.
                                         */

                                        ui.set_min_size(
                                            area
                                        );

                                        ui.vertical_centered(
                                            |ui| {

                                                ui.add_space(
                                                    (
                                                        area.y
                                                        / 2.0
                                                    )
                                                    .max(
                                                        0.0
                                                    )
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

                                                ui.label(
                                                    "Esperando video..."
                                                );
                                            }
                                        );
                                    }
                                }
                            );

                            let _ =
                                video_area;
                        }
                    );
                }
            );

        } else {

            /*
             * No hay dispositivo.
             */

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

        /*
         * Actualizamos la interfaz
         * aproximadamente a 60 FPS.
         */

        ui.ctx()
            .request_repaint_after(
                Duration::from_millis(16)
            );
    }
}

fn parse_resolution(
    resolution: &str,
) -> Option<(usize, usize)> {

    let cleaned =
        resolution.trim();

    let parts:
        Vec<&str> =
        cleaned
            .split('x')
            .collect();

    if parts.len() != 2 {
        return None;
    }

    let width =
        parts[0]
            .trim()
            .parse::<usize>()
            .ok()?;

    let height =
        parts[1]
            .trim()
            .parse::<usize>()
            .ok()?;

    Some(
        (
            width,
            height
        )
    )
}

fn handle_connection(
    mut stream: TcpStream,

    frame_sender:
        mpsc::SyncSender<Vec<u8>>,

    frames_received:
        Arc<Mutex<u64>>,

    resolution:
        (usize, usize),
) {

    let ffmpeg =
        TransfelApp::ffmpeg_path();

    println!(
        "FFmpeg: {}",
        ffmpeg.display()
    );

    let width =
        resolution.0;

    let height =
        resolution.1;

    println!(
        "Video: {}x{}",
        width,
        height
    );

    let mut child =
        match Command::new(
            &ffmpeg
        )
        .args([
            "-loglevel",
            "warning",

            "-f",
            "h264",

            "-i",
            "pipe:0",

            "-f",
            "rawvideo",

            "-pix_fmt",
            "rgba",

            "pipe:1",
        ])
        .stdin(
            Stdio::piped()
        )
        .stdout(
            Stdio::piped()
        )
        .stderr(
            Stdio::piped()
        )
        .spawn()
        {

            Ok(child) =>
                child,

            Err(error) => {

                println!(
                    "ERROR FFmpeg: {}",
                    error
                );

                return;
            }
        };

    let mut ffmpeg_input =
        match child.stdin.take()
        {

            Some(stdin) =>
                stdin,

            None => {

                println!(
                    "ERROR: no se pudo abrir stdin de FFmpeg"
                );

                return;
            }
        };

    let mut ffmpeg_output =
        match child.stdout.take()
        {

            Some(stdout) =>
                stdout,

            None => {

                println!(
                    "ERROR: no se pudo abrir stdout de FFmpeg"
                );

                return;
            }
        };

    if let Some(stderr) =
        child.stderr.take()
    {

        thread::spawn(
            move || {

                let mut reader =
                    std::io::BufReader::new(
                        stderr
                    );

                let mut text =
                    String::new();

                loop {

                    text.clear();

                    match reader.read_line(
                        &mut text
                    ) {

                        Ok(0) =>
                            break,

                        Ok(_) => {

                            println!(
                                "FFmpeg: {}",
                                text.trim()
                            );
                        }

                        Err(_) =>
                            break,
                    }
                }
            }
        );
    }

    /*
     * Hilo que recibe los frames
     * convertidos por FFmpeg.
     */

    let output_sender =
        frame_sender.clone();

    thread::spawn(
        move || {

            let frame_size =
                width
                    * height
                    * 4;

            let mut raw_frame =
                vec![
                    0u8;
                    frame_size
                ];

            loop {

                match ffmpeg_output
                    .read_exact(
                        &mut raw_frame
                    )
                {

                    Ok(_) => {

                        match output_sender
                            .try_send(
                                raw_frame.clone()
                            )
                        {

                            Ok(_) => {}

                            Err(
                                mpsc::TrySendError::Full(
                                    _
                                )
                            ) => {}

                            Err(
                                mpsc::TrySendError::Disconnected(
                                    _
                                )
                            ) => {

                                break;
                            }
                        }
                    }

                    Err(error) => {

                        println!(
                            "FFmpeg video detenido: {}",
                            error
                        );

                        break;
                    }
                }
            }
        }
    );

    /*
     * Recibimos los frames H264
     * desde Android.
     */

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
            vec![
                0u8;
                size
            ];

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

        if let Err(error) =
            ffmpeg_input.write_all(
                &frame
            )
        {

            println!(
                "ERROR enviando H264 a FFmpeg: {}",
                error
            );

            break;
        }

        let numero = {

            let mut contador =
                frames_received
                    .lock()
                    .unwrap();

            *contador += 1;

            *contador
        };

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

    drop(
        ffmpeg_input
    );

    let _ =
        child.kill();

    let _ =
        child.wait();
}