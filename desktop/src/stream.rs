use crate::adb;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;


pub const TAG_RESIZE: u32 = 0xFFFF_0001;
pub const TAG_CROP: u32 = 0xFFFF_0002;

// Eventos que el PC manda al telefono por el mismo socket.
pub const CTRL_RUEDA: u32 = 0x0105;
pub const CTRL_ABAJO: u32 = 0x0101;
pub const CTRL_MOVER: u32 = 0x0102;
pub const CTRL_ARRIBA: u32 = 0x0103;
pub const CTRL_ACCION: u32 = 0x0104;

/// Acciones globales de AccessibilityService.
pub const ACCION_ATRAS: u32 = 1;
pub const ACCION_INICIO: u32 = 2;
pub const ACCION_RECIENTES: u32 = 3;
const MAX_PACKET: u32 = 20_000_000;


pub type SharedRes = Arc<Mutex<(usize, usize)>>;

/// Hueco de un solo frame. Sustituye al canal: el productor deja el frame
/// nuevo y se lleva el anterior para reutilizar su memoria, asi no se asignan
/// 10 MB en cada fotograma.
pub type Hueco = Arc<Mutex<Option<Frame>>>;

/// Lado maximo del video decodificado. Por encima de esto no se gana nitidez
/// visible en pantalla y si se gasta mucho ancho de banda hacia la GPU.
const LADO_MAXIMO: usize = 1080;

/// Calcula el tamaño de salida respetando la proporcion.
fn escalar(width: usize, height: usize) -> (usize, usize) {
    let mayor = width.max(height);
    if mayor <= LADO_MAXIMO {
        return (width, height);
    }
    let factor = LADO_MAXIMO as f32 / mayor as f32;
    (
        ((width as f32 * factor) as usize) & !1,
        ((height as f32 * factor) as usize) & !1,
    )
}


pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

pub struct StreamServer {
    pub hueco: Hueco,
    pub frames: Arc<AtomicU64>,
    pub clients: Arc<AtomicUsize>,
    pub resolution: SharedRes,
    /// Zona util dentro del video cuadrado: el tamaño logico de la pantalla.
    pub content: SharedRes,
    /// Mitad de escritura del socket, para enviar eventos de control.
    pub control: Arc<Mutex<Option<TcpStream>>>,
}

impl StreamServer {
    pub fn start(width: usize, height: usize) -> Self {
        let hueco: Hueco = Arc::new(Mutex::new(None));
        let frames = Arc::new(AtomicU64::new(0));
        let clients = Arc::new(AtomicUsize::new(0));
        let resolution: SharedRes = Arc::new(Mutex::new((width, height)));


        let content: SharedRes = Arc::new(Mutex::new((width, height)));
        let control: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));

        let hueco_bg = Arc::clone(&hueco);
        let frames_bg = Arc::clone(&frames);
        let clients_bg = Arc::clone(&clients);
        let res_bg = Arc::clone(&resolution);
        let content_bg = Arc::clone(&content);
        let control_bg = Arc::clone(&control);


        thread::spawn(move || {
            let listener = match TcpListener::bind("127.0.0.1:5000") {
                Ok(l) => l,
                Err(e) => return println!("Error puerto 5000: {e}"),
            };
            println!("TransFEL escuchando en 5000");

          for conn in listener.incoming() {
              let Ok(stream) = conn else { continue };
              let res = res_bg.lock().map(|r| *r).unwrap_or((720, 1280));
              let hueco = Arc::clone(&hueco_bg);
              let frames = Arc::clone(&frames_bg);
              let clients = Arc::clone(&clients_bg);
              let content = Arc::clone(&content_bg);
              let control = Arc::clone(&control_bg);

              // Mitad de escritura para el control remoto.
              if let Ok(escritor) = stream.try_clone() {
                  if let Ok(mut guardado) = control.lock() {
                      *guardado = Some(escritor);
                  }
              }

              thread::spawn(move || {
                  clients.fetch_add(1, Ordering::Relaxed);
                  handle_connection(stream, hueco, frames, res, content);
                  if let Ok(mut guardado) = control.lock() {
                      *guardado = None;
                  }
                  clients.fetch_sub(1, Ordering::Relaxed);
                  println!("Conexion cerrada");
              });
            }
        });

        Self { hueco, frames, clients, resolution, content, control }
    }



    pub fn is_live(&self) -> bool {
     self.clients.load(Ordering::Relaxed) > 0
    }

    /// Devuelve solo el frame mas reciente, descartando los atrasados.
    /// Toma el frame mas reciente, si hay uno sin consumir.
    pub fn latest_frame(&self) -> Option<Frame> {
        self.hueco.lock().ok()?.take()
    }

    /// Vacia frames viejos para que no reaparezca la ultima imagen.
    pub fn drain(&self) {
        if let Ok(mut h) = self.hueco.lock() {
            *h = None;
        }
    }

    /// Rueda del raton: desplazamiento vertical en pixeles del dispositivo.
    pub fn enviar_rueda(&self, x: i32, y: i32, delta: i32) {
        let Ok(mut guardado) = self.control.lock() else { return };
        let Some(socket) = guardado.as_mut() else { return };

        let mut buffer = [0u8; 16];
        buffer[0..4].copy_from_slice(&CTRL_RUEDA.to_be_bytes());
        buffer[4..8].copy_from_slice(&x.to_be_bytes());
        buffer[8..12].copy_from_slice(&y.to_be_bytes());
        buffer[12..16].copy_from_slice(&delta.to_be_bytes());

        if socket.write_all(&buffer).is_err() {
            *guardado = None;
        }
    }

    pub fn control_disponible(&self) -> bool {
        self.control.lock().map(|c| c.is_some()).unwrap_or(false)
    }

    /// Envia un evento tactil al telefono por el mismo socket del video.
    pub fn enviar_toque(&self, tag: u32, x: i32, y: i32) {
        let Ok(mut guardado) = self.control.lock() else { return };
        let Some(socket) = guardado.as_mut() else { return };

        let mut buffer = [0u8; 12];
        buffer[0..4].copy_from_slice(&tag.to_be_bytes());
        buffer[4..8].copy_from_slice(&x.to_be_bytes());
        buffer[8..12].copy_from_slice(&y.to_be_bytes());

        if socket.write_all(&buffer).is_err() {
            *guardado = None;
        }
    }

    pub fn enviar_accion(&self, codigo: u32) {
        let Ok(mut guardado) = self.control.lock() else { return };
        let Some(socket) = guardado.as_mut() else { return };

        let mut buffer = [0u8; 8];
        buffer[0..4].copy_from_slice(&CTRL_ACCION.to_be_bytes());
        buffer[4..8].copy_from_slice(&codigo.to_be_bytes());

        if socket.write_all(&buffer).is_err() {
            *guardado = None;
        }
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
    fn spawn(width: usize, height: usize, hueco: Hueco) -> Option<Self> {
        // Escalar en FFmpeg (SIMD, en C) sale mucho mas barato que mover
        // frames enormes hasta la GPU en cada repintado.
        let (ancho_salida, alto_salida) = escalar(width, height);
        let filtro = format!("scale={ancho_salida}:{alto_salida}:flags=fast_bilinear");

        let mut child = adb::silent(adb::ffmpeg_path())
            .args([
                "-loglevel", "warning",
                "-f", "h264",
                "-i", "pipe:0",
                "-vf", &filtro,
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
            let size = ancho_salida * alto_salida * 4;
            let mut buf = vec![0u8; size];
            let mut decodificados: u64 = 0;

            loop {
                if output.read_exact(&mut buf).is_err() {
                    println!("Decodificador terminado tras {decodificados} frames");
                    break;
                }

                decodificados += 1;
                if decodificados == 1 || decodificados % 300 == 0 {
                    println!("Frames decodificados: {decodificados}");
                }

                let frame = Frame {
                    width: ancho_salida,
                    height: alto_salida,
                    data: std::mem::take(&mut buf),
                };

                // Se deja el frame nuevo y se recupera el anterior para
                // reutilizar su memoria: en regimen no se asigna nada.
                let anterior = match hueco.lock() {
                    Ok(mut h) => h.replace(frame),
                    Err(_) => break,
                };

                buf = match anterior {
                    Some(viejo) if viejo.data.len() == size => viejo.data,
                    _ => vec![0u8; size],
                };
            }
        });

        println!("Decodificador listo: {width}x{height} -> {ancho_salida}x{alto_salida}");

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
    hueco: Hueco,
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
                decoder = Decoder::spawn(width, height, Arc::clone(&hueco));
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
            decoder = Decoder::spawn(fallback_w, fallback_h, Arc::clone(&hueco));
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