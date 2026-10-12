//! Miniaturas de imagenes y video para la lista de archivos.
//!
//! Se generan bajo demanda y solo para las filas visibles: pedirlas todas
//! obligaria a traer cada foto del telefono entera, que es carisimo.

use crate::adb;
use eframe::egui;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;

/// Lado maximo de la miniatura, en pixeles.
const LADO: u32 = 96;

/// Cuantas se generan a la vez. Mas hilos no van mas rapido: el cuello de
/// botella es el cable o el WiFi.
const EN_PARALELO: usize = 3;

/// Por encima de esto no se intenta: traer el archivo costaria mas que mirarlo.
pub const TAMANO_MAXIMO: u64 = 25 * 1024 * 1024;

const LIMITE_CACHE: usize = 500;

/// De donde sale el archivo.
#[derive(Clone)]
pub enum Origen {
    Pc(String),
    /// (serial, ruta en el dispositivo)
    Android(String, String),
}

/// Como hay que extraer la imagen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Imagen,
    Video,
}

/// Mapa de bytes ya decodificado, listo para subir a la GPU.
pub struct Mapa {
    ancho: usize,
    alto: usize,
    datos: Vec<u8>,
}

enum Estado {
    EnCurso,
    Lista(egui::TextureHandle),
    Fallida,
}

pub struct Miniaturas {
    cache: HashMap<String, Estado>,
    tx: mpsc::Sender<(String, Option<Mapa>)>,
    rx: mpsc::Receiver<(String, Option<Mapa>)>,
    en_curso: usize,
    pub activadas: bool,
}

impl Default for Miniaturas {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            cache: HashMap::new(),
            tx,
            rx,
            en_curso: 0,
            activadas: true,
        }
    }
}

impl Miniaturas {
    /// Recoge las que terminaron y las convierte en texturas.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((clave, resultado)) = self.rx.try_recv() {
            self.en_curso = self.en_curso.saturating_sub(1);

            let estado = match resultado {
                Some(mapa) => {
                    let imagen = egui::ColorImage::from_rgba_unmultiplied(
                        [mapa.ancho, mapa.alto],
                        &mapa.datos,
                    );
                    Estado::Lista(ctx.load_texture(
                        format!("mini:{clave}"),
                        imagen,
                        egui::TextureOptions::LINEAR,
                    ))
                }
                None => Estado::Fallida,
            };

            self.cache.insert(clave, estado);
        }
    }

    /// Devuelve la miniatura si ya esta; si no, la encola y devuelve None.
    pub fn obtener(
        &mut self,
        clave: &str,
        origen: Origen,
        clase: Clase,
    ) -> Option<&egui::TextureHandle> {
        if !self.activadas {
            return None;
        }

        // Ya esta lista.
        if matches!(self.cache.get(clave), Some(Estado::Lista(_))) {
            return match self.cache.get(clave) {
                Some(Estado::Lista(textura)) => Some(textura),
                _ => None,
            };
        }

        // En curso o descartada: no se vuelve a pedir.
        if self.cache.contains_key(clave) {
            return None;
        }

        if self.en_curso >= EN_PARALELO {
            return None; // se reintenta en el siguiente repintado
        }

        if self.cache.len() > LIMITE_CACHE {
            self.cache.clear();
        }

        self.cache.insert(clave.to_string(), Estado::EnCurso);
        self.en_curso += 1;

        let tx = self.tx.clone();
        let clave = clave.to_string();
        thread::spawn(move || {
            let resultado = generar(&origen, clase);
            let _ = tx.send((clave, resultado));
        });

        None
    }

    pub fn limpiar(&mut self) {
        self.cache.clear();
    }

    pub fn pendientes(&self) -> usize {
        self.en_curso
    }
}

// ─────────────────────────── Generacion ───────────────────────────

fn generar(origen: &Origen, clase: Clase) -> Option<Mapa> {
    match (origen, clase) {
        (Origen::Pc(ruta), Clase::Imagen) => {
            let bytes = std::fs::read(ruta).ok()?;
            decodificar(&bytes)
        }

        (Origen::Pc(ruta), Clase::Video) => {
            let png = fotograma_ffmpeg(&["-ss", "1", "-i", ruta])?;
            decodificar(&png)
        }

        (Origen::Android(serial, ruta), Clase::Imagen) => {
            // exec-out entrega los bytes crudos, sin tocar saltos de linea.
            let bytes = adb::exec_out(serial, &format!("cat \"{ruta}\""))?;
            decodificar(&bytes)
        }

        (Origen::Android(serial, ruta), Clase::Video) => {
            // Se expone el archivo por HTTP con soporte de rangos: asi FFmpeg
            // salta al indice del final y a los primeros fotogramas sin tener
            // que traerse el video entero.
            let servidor = crate::rango::Servidor::iniciar(serial.clone(), ruta.clone())?;
            let png = fotograma_ffmpeg(&["-ss", "1", "-i", &servidor.url])?;
            decodificar(&png)
        }
    }
}

/// Decodifica cualquier formato soportado y lo reduce.
fn decodificar(bytes: &[u8]) -> Option<Mapa> {
    let imagen = image::load_from_memory(bytes).ok()?;
    let mini = imagen.thumbnail(LADO, LADO).to_rgba8();

    Some(Mapa {
        ancho: mini.width() as usize,
        alto: mini.height() as usize,
        datos: mini.into_raw(),
    })
}

/// Saca un fotograma con FFmpeg y lo devuelve como PNG.
fn fotograma_ffmpeg(entrada: &[&str]) -> Option<Vec<u8>> {
    let mut orden = adb::silent(adb::ffmpeg_path());
    orden.args(["-loglevel", "error"]);
    orden.args(entrada);
    orden.args([
        // Suficiente para encontrar el indice sin leer de mas.
        "-analyzeduration", "4000000",
        "-probesize", "4000000",
        "-frames:v", "1",
        "-vf", &format!("scale={LADO}:-1"),
        "-f", "image2",
        "-vcodec", "png",
        "-y", "pipe:1",
    ]);

    let salida = orden
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    (!salida.stdout.is_empty()).then_some(salida.stdout)
}
