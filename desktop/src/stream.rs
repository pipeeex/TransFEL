use crate::adb;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

pub type SharedRes = Arc<Mutex<(usize, usize)>>;

pub struct StreamServer {
    pub rx: mpsc::Receiver<Vec<u8>>,
    pub frames: Arc<AtomicU64>,
    pub resolution: SharedRes,
}

impl StreamServer {
    pub fn start(width: usize, height: usize) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
        let frames = Arc::new(AtomicU64::new(0));
        let resolution: SharedRes = Arc::new(Mutex::new((width, height)));

        let frames_bg = Arc::clone(&frames);
        let res_bg = Arc::clone(&resolution);

        thread::spawn(move || {
            let listener = match TcpListener::bind("127.0.0.1:5000") {
                Ok(l) => l,
                Err(e) => return println!("Error puerto 5000: {e}"),
            };
            println!("TransFEL escuchando en 5000");

            for conn in listener.incoming() {
                let Ok(stream) = conn else { continue };
                let res = res_bg.lock().map(|r| *r).unwrap_or((720, 1280));
                let tx = tx.clone();
                let frames = Arc::clone(&frames_bg);
                thread::spawn(move || handle_connection(stream, tx, frames, res));
            }
        });

        Self { rx, frames, resolution }
    }

    /// Devuelve solo el frame mas reciente, descartando los atrasados.
    pub fn latest_frame(&self) -> Option<Vec<u8>> {
        let mut last = None;
        while let Ok(f) = self.rx.try_recv() {
            last = Some(f);
        }
        last
    }

    pub fn set_resolution(&self, w: usize, h: usize) {
        if let Ok(mut r) = self.resolution.lock() {
            *r = (w, h);
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    tx: mpsc::SyncSender<Vec<u8>>,
    frames: Arc<AtomicU64>,
    (width, height): (usize, usize),
) {
    let mut child = match adb::silent(adb::ffmpeg_path())
        .args([
            "-loglevel", "warning",
            "-f", "h264",
            "-i", "pipe:0",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return println!("Error FFmpeg: {e}"),
    };

    let (Some(mut input), Some(mut output)) = (child.stdin.take(), child.stdout.take()) else {
        return;
    };

    if let Some(err) = child.stderr.take() {
        thread::spawn(move || {
            let mut reader = BufReader::new(err);
            let mut line = String::new();
            while matches!(reader.read_line(&mut line), Ok(n) if n > 0) {
                println!("FFmpeg: {}", line.trim());
                line.clear();
            }
        });
    }

    // Hilo de salida: RGBA -> UI
    thread::spawn(move || {
        let size = width * height * 4;
        let mut buf = vec![0u8; size];
        loop {
            if output.read_exact(&mut buf).is_err() {
                break;
            }
            // Se entrega el buffer; solo se re-aloja si se consumio.
            match tx.try_send(std::mem::take(&mut buf)) {
                Ok(()) => buf = vec![0u8; size],
                Err(mpsc::TrySendError::Full(returned)) => buf = returned,
                Err(mpsc::TrySendError::Disconnected(_)) => break,
            }
        }
    });

    // Hilo principal de la conexion: TCP -> FFmpeg
    let mut header = [0u8; 4];
    let mut packet = Vec::new();

    loop {
        if stream.read_exact(&mut header).is_err() {
            break;
        }
        let size = u32::from_be_bytes(header) as usize;
        if size == 0 {
            continue;
        }
        if size > 20_000_000 {
            break;
        }

        packet.resize(size, 0);
        if stream.read_exact(&mut packet).is_err() {
            break;
        }
        if input.write_all(&packet).is_err() {
            break;
        }
        frames.fetch_add(1, Ordering::Relaxed);
    }

    drop(input);
    let _ = child.kill();
    let _ = child.wait();
}