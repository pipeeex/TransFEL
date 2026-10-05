use crate::adb;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;


pub const TAG_RESIZE: u32 = 0xFFFF_0001;
pub const TAG_CROP: u32 = 0xFFFF_0002;
const MAX_PACKET: u32 = 20_000_000;


pub type SharedRes = Arc<Mutex<(usize, usize)>>;


pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

pub struct StreamServer {
    pub rx: mpsc::Receiver<Frame>,
    pub frames: Arc<AtomicU64>,
    pub clients: Arc<AtomicUsize>,
    pub resolution: SharedRes,
    /// Zona util dentro del video cuadrado: el tamaño logico de la pantalla.
    pub content: SharedRes,
}

impl StreamServer {
    pub fn start(width: usize, height: usize) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Frame>(1);
        let frames = Arc::new(AtomicU64::new(0));
        let clients = Arc::new(AtomicUsize::new(0));
        let resolution: SharedRes = Arc::new(Mutex::new((width, height)));


        let content: SharedRes = Arc::new(Mutex::new((width, height)));

        let frames_bg = Arc::clone(&frames);
        let clients_bg = Arc::clone(&clients);
        let res_bg = Arc::clone(&resolution);
        let content_bg = Arc::clone(&content);


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
              let clients = Arc::clone(&clients_bg);
              let content = Arc::clone(&content_bg);

              thread::spawn(move || {
                  clients.fetch_add(1, Ordering::Relaxed);
                  handle_connection(stream, tx, frames, res, content);
                  clients.fetch_sub(1, Ordering::Relaxed);
                  println!("Conexion cerrada");
              });
            }
        });

        Self { rx, frames, clients, resolution, content }
    }



    pub fn is_live(&self) -> bool {
     self.clients.load(Ordering::Relaxed) > 0
    }

    /// Devuelve solo el frame mas reciente, descartando los atrasados.
    pub fn latest_frame(&self) -> Option<Frame> {
        let mut last = None;
        while let Ok(f) = self.rx.try_recv() {
            last = Some(f);
        }
        last
    }

    /// Vacia frames viejos para que no reaparezca la ultima imagen.
    pub fn drain(&self) {
     while self.rx.try_recv().is_ok() {}
    }

    /// Zona util de la pantalla dentro del video cuadrado.
    pub fn content_size(&self) -> (usize, usize) {
        self.content.lock().map(|c| *c).unwrap_or((720, 1600))
    }

    pub fn set_resolution(&self, w: usize, h: usize) {
        if let Ok(mut r) = self.resolution.lock() {
            *r = (w, h);
        }
    }
}

// DECODIFICADOR

struct Decoder {
    child: Child, 
    input: Box<dyn Write + Send>,
    width: usize,
    height: usize
}

impl Decoder {
    fn spawn(width: usize, height: usize, tx: mpsc::SyncSender<Frame>) -> Option<Self> {
        let mut child = adb::silent(adb::ffmpeg_path())
            .args([
                "-loglevel", "warning",
                "-f", "h264",
                "-i", "pipe:0",
                "-f", "rawvideo",
                "-pix_fmt", "rgba",
                "-flush_packets", "1",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| println!("Error FFmpeg: {e}"))
            .ok()?;

        let input = child.stdin.take()?;
        let mut output = child.stdout.take()?;

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

        thread::spawn(move || {
            let size = width * height * 4;
            let mut buf = vec![0u8; size];
            let mut decodificados: u64 = 0;
            loop {
                if output.read_exact(&mut buf).is_err() {
                    println!("Decodificador terminado tras {decodificados} frames");
                    break;
                }
                decodificados += 1;
                if decodificados == 1 || decodificados % 60 == 0 {
                    println!("Frames decodificados: {decodificados}");
                }
                let frame = Frame {
                    width,
                    height,
                    data: std::mem::take(&mut buf),
                };
                match tx.try_send(frame) {
                    Ok(()) => buf = vec![0u8; size],
                    Err(mpsc::TrySendError::Full(devuelto)) => buf = devuelto.data,
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
        });

        println!("Decodificador listo: {width}x{height}");

        Some(Self { child, input: Box::new(input), width, height })
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Lee un u32 big-endian. Sin slicing, sin unwrap.
fn leer_u32(stream: &mut TcpStream) -> Option<u32> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).ok()?;
    Some(u32::from_be_bytes(buf))
}

fn handle_connection(
    mut stream: TcpStream,
    tx: mpsc::SyncSender<Frame>,
    frames: Arc<AtomicU64>,
    (fallback_w, fallback_h): (usize, usize),
    content: SharedRes,
) {
    // No se crea el decodificador hasta conocer el tamano real.
    let mut decoder: Option<Decoder> = None;
    let mut packet = Vec::new();
    let mut recibidos: u64 = 0;
    let mut bytes: u64 = 0;

    loop {
        let Some(tag) = leer_u32(&mut stream) else { break };

        // ── Control: resolucion ──
        if tag == TAG_RESIZE {
            let (Some(w), Some(h)) = (leer_u32(&mut stream), leer_u32(&mut stream)) else {
                break;
            };
            let width = w as usize;
            let height = h as usize;

            if width == 0 || height == 0 || width > 8000 || height > 8000 {
                println!("Tamaño invalido: {width}x{height}");
                continue;
            }

            let igual = decoder
                .as_ref()
                .is_some_and(|d| d.width == width && d.height == height);

            if !igual {
                println!("Resolucion: {width}x{height}");
                if let Some(mut viejo) = decoder.take() {
                    viejo.kill();
                }
                decoder = Decoder::spawn(width, height, tx.clone());
            }
            continue;
        }

        // ── Control: zona util (rotacion) ──
        if tag == TAG_CROP {
            let (Some(w), Some(h)) = (leer_u32(&mut stream), leer_u32(&mut stream)) else {
                break;
            };
            if w > 0 && h > 0 && w <= 8000 && h <= 8000 {
                println!("Orientacion: {w}x{h}");
                if let Ok(mut c) = content.lock() {
                    *c = (w as usize, h as usize);
                }
            }
            continue;
        }

        if tag == 0 {
            continue;
        }
        if tag > MAX_PACKET {
            println!("Paquete fuera de rango ({tag}), cerrando");
            break;
        }

        packet.resize(tag as usize, 0);
        if stream.read_exact(&mut packet).is_err() {
            println!("Lectura TCP cortada");
            break;
        }

        recibidos += 1;
        bytes += tag as u64;
        if recibidos == 1 || recibidos % 60 == 0 {
            println!("Paquetes recibidos: {recibidos} ({bytes} bytes, ultimo {tag})");
        }

        // APK antigua que nunca manda el tamano: usamos el de respaldo.
        if decoder.is_none() {
            println!("Sin paquete de tamaño, usando {fallback_w}x{fallback_h}");
            decoder = Decoder::spawn(fallback_w, fallback_h, tx.clone());
        }

        let Some(dec) = decoder.as_mut() else { continue };
        if dec.input.write_all(&packet).is_err() {
            println!("FFmpeg cerro su entrada");
            break;
        }
        let _ = dec.input.flush();
        frames.fetch_add(1, Ordering::Relaxed);
    }

    if let Some(mut dec) = decoder.take() {
        dec.kill();
    }
}