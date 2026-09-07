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
            .with_inner_size([1250.0, 800.0])
            .with_min_inner_size([950.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "TransFEL",
        options,
        Box::new(|cc| {
            Ok(Box::new(
                TransfelApp::new(cc)
            ))
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

    frame_receiver:
        mpsc::Receiver<Vec<u8>>,

    frame_sender:
        mpsc::SyncSender<Vec<u8>>,

    texture:
        Option<egui::TextureHandle>,

    video_width: usize,
    video_height: usize,
}

impl TransfelApp {

    fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Self {

        /*
         * Tema oscuro.
         */

        cc.egui_ctx.set_visuals(
            egui::Visuals::dark()
        );

        let frames_received =
            Arc::new(
                Mutex::new(0)
            );

        let (
            frame_sender,
            frame_receiver
        ) =
            mpsc::sync_channel::<Vec<u8>>(
                1
            );

        let resolution =
            Arc::new(
                Mutex::new(
                    (
                        720usize,
                        1280usize
                    )
                )
            );

        let mut app = Self {

            device: None,

            model:
                "-".to_string(),

            android:
                "-".to_string(),

            resolution:
                "-".to_string(),

            density:
                "-".to_string(),

            status:
                "Buscando dispositivo..."
                    .to_string(),

            streaming:
                false,

            frames_received:
                Arc::clone(
                    &frames_received
                ),

            frame_receiver,

            frame_sender:
                frame_sender.clone(),

            texture:
                None,

            video_width:
                720,

            video_height:
                1280,
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
                    "Conecta tu dispositivo Android"
                        .to_string();
            }

            Err(_) => {

                self.clear_device();

                self.status =
                    "No se pudo iniciar ADB"
                        .to_string();
            }
        }
    }

    fn clear_device(
        &mut self
    ) {

        self.device = None;

        self.model =
            "-".to_string();

        self.android =
            "-".to_string();

        self.resolution =
            "-".to_string();

        self.density =
            "-".to_string();

        self.texture = None;
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
                                "Error puerto 5000: {}",
                                error
                            );

                            return;
                        }
                    };

                println!(
                    "TransFEL listo"
                );

                for connection
                    in listener.incoming()
                {

                    match connection {

                        Ok(stream) => {

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
         * Obtener solamente el frame
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
         * Actualizar video.
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
         * Colores de interfaz.
         */

        let background =
            egui::Color32::from_rgb(
                15,
                15,
                18
            );

        let panel =
            egui::Color32::from_rgb(
                22,
                22,
                27
            );

        let card =
            egui::Color32::from_rgb(
                28,
                28,
                34
            );

        let border =
            egui::Color32::from_rgb(
                55,
                55,
                65
            );

        let accent =
            egui::Color32::from_rgb(
                90,
                130,
                255
            );

        let success =
            egui::Color32::from_rgb(
                70,
                200,
                120
            );

        ui.painter().rect_filled(
            ui.max_rect(),
            0.0,
            background
        );

        /*
         * Barra superior.
         */

        egui::Panel::top(
            "top_bar"
        )
        .frame(
            egui::Frame::new()
                .fill(panel)
                .inner_margin(
                    egui::Margin::symmetric(
                        20,
                        12
                    )
                )
        )
        .show(
            ui,
            |ui| {

                ui.horizontal(
                    |ui| {

                        ui.heading(
                            egui::RichText::new(
                                "TransFEL"
                            )
                            .size(22.0)
                            .strong()
                        );

                        ui.add_space(
                            15.0
                        );

                        let status_color =
                            if self.device.is_some() {
                                success
                            } else {
                                egui::Color32::GRAY
                            };

                        ui.colored_label(
                            status_color,
                            if self.device.is_some() {
                                "● Dispositivo conectado"
                            } else {
                                "● Sin dispositivo"
                            }
                        );

                        ui.with_layout(
                            egui::Layout::right_to_left(
                                egui::Align::Center
                            ),
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
                            }
                        );
                    }
                );
            }
        );

        /*
         * Barra lateral.
         */

        egui::Panel::left(
            "sidebar"
        )
        .resizable(true)
        .default_size(260.0)
        .size_range(220.0..=320.0)
        .frame(
            egui::Frame::new()
                .fill(panel)
                .inner_margin(
                    egui::Margin::same(
                        16
                    )
                )
        )
        .show(
            ui,
            |ui| {

                /*
                 * Informacion del dispositivo.
                 */

                ui.label(
                    egui::RichText::new(
                        "DISPOSITIVO"
                    )
                    .size(12.0)
                    .strong()
                    .color(
                        egui::Color32::GRAY
                    )
                );

                ui.add_space(
                    8.0
                );

                egui::Frame::group(
                    ui.style()
                )
                .fill(card)
                .stroke(
                    egui::Stroke::new(
                        1.0,
                        border
                    )
                )
                .show(
                    ui,
                    |ui| {

                        ui.set_width(
                            ui.available_width()
                        );

                        ui.label(
                            egui::RichText::new(
                                &self.model
                            )
                            .size(18.0)
                            .strong()
                        );

                        ui.add_space(
                            8.0
                        );

                        ui.label(
                            format!(
                                "Android {}",
                                self.android
                            )
                        );

                        ui.label(
                            format!(
                                "{}",
                                self.resolution
                            )
                        );

                        ui.label(
                            format!(
                                "Densidad {}",
                                self.density
                            )
                        );

                        ui.add_space(
                            8.0
                        );

                        ui.colored_label(
                            success,
                            "USB conectado"
                        );
                    }
                );

                ui.add_space(
                    25.0
                );

                /*
                 * Menu.
                 */

                ui.label(
                    egui::RichText::new(
                        "FUNCIONES"
                    )
                    .size(12.0)
                    .strong()
                    .color(
                        egui::Color32::GRAY
                    )
                );

                ui.add_space(
                    8.0
                );

                /*
                 * Pantalla.
                 */

                let pantalla =
                    egui::Button::new(
                        egui::RichText::new(
                            "▣   Pantalla"
                        )
                        .size(15.0)
                    )
                    .min_size(
                        egui::vec2(
                            ui.available_width(),
                            42.0
                        )
                    );

                if ui
                    .add(pantalla)
                    .clicked()
                {
                }

                ui.add_space(
                    7.0
                );

                /*
                 * Control.
                 */

                let control =
                    egui::Button::new(
                        egui::RichText::new(
                            "⌨   Control"
                        )
                        .size(15.0)
                    )
                    .min_size(
                        egui::vec2(
                            ui.available_width(),
                            42.0
                        )
                    );

                if ui
                    .add(control)
                    .clicked()
                {
                }

                ui.add_space(
                    7.0
                );

                /*
                 * Archivos.
                 */

                let archivos =
                    egui::Button::new(
                        egui::RichText::new(
                            "▤   Archivos"
                        )
                        .size(15.0)
                    )
                    .min_size(
                        egui::vec2(
                            ui.available_width(),
                            42.0
                        )
                    );

                if ui
                    .add(archivos)
                    .clicked()
                {
                }

                /*
                 * Estado de transmision.
                 */

                ui.add_space(
                    25.0
                );

                egui::Frame::group(
                    ui.style()
                )
                .fill(card)
                .stroke(
                    egui::Stroke::new(
                        1.0,
                        border
                    )
                )
                .show(
                    ui,
                    |ui| {

                        ui.label(
                            egui::RichText::new(
                                "TRANSMISION"
                            )
                            .size(12.0)
                            .strong()
                            .color(
                                egui::Color32::GRAY
                            )
                        );

                        ui.add_space(
                            8.0
                        );

                        if self.texture.is_some() {

                            ui.colored_label(
                                success,
                                "● Transmision activa"
                            );

                        } else {

                            ui.label(
                                "● Inactiva"
                            );
                        }
                    }
                );

                ui.with_layout(
                    egui::Layout::bottom_up(
                        egui::Align::LEFT
                    ),
                    |ui| {

                        ui.add_space(
                            10.0
                        );

                        ui.label(
                            egui::RichText::new(
                                "TransFEL"
                            )
                            .size(12.0)
                            .color(
                                egui::Color32::GRAY
                            )
                        );
                    }
                );
            }
        );

        /*
         * Area principal.
         */

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(background)
                    .inner_margin(
                        egui::Margin::same(
                            20
                        )
                    )
            )
            .show(
                ui,
                |ui| {

                    /*
                     * Encabezado.
                     */

                    ui.horizontal(
                        |ui| {

                            ui.label(
                                egui::RichText::new(
                                    "Pantalla del dispositivo"
                                )
                                .size(20.0)
                                .strong()
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(
                                    egui::Align::Center
                                ),
                                |ui| {

                                    if self.texture.is_some() {

                                        ui.colored_label(
                                            success,
                                            "● EN VIVO"
                                        );
                                    }
                                }
                            );
                        }
                    );

                    ui.add_space(
                        12.0
                    );

                    /*
                     * Contenedor de video.
                     */

                    egui::Frame::group(
                        ui.style()
                    )
                    .fill(
                        egui::Color32::from_rgb(
                            8,
                            8,
                            10
                        )
                    )
                    .stroke(
                        egui::Stroke::new(
                            1.0,
                            border
                        )
                    )
                    .show(
                        ui,
                        |ui| {

                            let area =
                                ui.available_size();

                            if let Some(
                                texture
                            ) =
                                &self.texture
                            {

                                let texture_size =
                                    texture
                                        .size_vec2();

                                /*
                                 * Escalado dinamico.
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
                                 * Centrado vertical.
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
                                 * Centrado horizontal.
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
                                                display_size
                                            )
                                        );
                                    }
                                );

                            } else {

                                /*
                                 * Estado inicial.
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
                                            - 70.0
                                        );

                                        ui.label(
                                            egui::RichText::new(
                                                "Pantalla Android"
                                            )
                                            .size(24.0)
                                            .strong()
                                        );

                                        ui.add_space(
                                            10.0
                                        );

                                        ui.label(
                                            egui::RichText::new(
                                                "Inicia la transmision desde la app movil"
                                            )
                                            .size(15.0)
                                            .color(
                                                egui::Color32::GRAY
                                            )
                                        );
                                    }
                                );
                            }
                        }
                    );
                }
            );

        /*
         * Actualizacion fluida.
         */

        ui.ctx()
            .request_repaint_after(
                Duration::from_millis(
                    16
                )
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

    let width =
        resolution.0;

    let height =
        resolution.1;

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
                    "Error FFmpeg: {}",
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
                return;
            }
        };

    let mut ffmpeg_output =
        match child.stdout.take()
        {

            Some(stdout) =>
                stdout,

            None => {
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
     * Procesamiento de video.
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

                    Err(_) => {
                        break;
                    }
                }
            }
        }
    );

    /*
     * Recepcion H264.
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
            break;
        }

        if ffmpeg_input
            .write_all(
                &frame
            )
            .is_err()
        {
            break;
        }

        let mut counter =
            frames_received
                .lock()
                .unwrap();

        *counter += 1;
    }

    drop(
        ffmpeg_input
    );

    let _ =
        child.kill();

    let _ =
        child.wait();
}