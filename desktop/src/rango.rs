//! Servidor HTTP minimo que expone un archivo del telefono por rangos.
//!
//! El indice de un MP4 de camara esta al final del archivo. Si se le pasa el
//! video a FFmpeg por una tuberia, tiene que leerlo entero para llegar hasta
//! ahi. Sirviendolo por HTTP con soporte de Range, FFmpeg salta directamente
//! a donde necesita y acaba leyendo unos pocos megas en vez de todo.

use crate::adb;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Tamaño de bloque de `dd`. Toybox no admite lecturas a nivel de byte,
/// asi que se lee alineado y se recorta aqui.
const BLOQUE: u64 = 4096;

pub struct Servidor {
    pub url: String,
    parar: Arc<AtomicBool>,
    puerto: u16,
}

impl Servidor {
    /// Levanta el servidor para un archivo concreto del dispositivo.
    pub fn iniciar(serial: String, ruta: String) -> Option<Self> {
        let tamano = tamano_remoto(&serial, &ruta)?;
        if tamano == 0 {
            return None;
        }

        let escucha = TcpListener::bind("127.0.0.1:0").ok()?;
        let puerto = escucha.local_addr().ok()?.port();

        let parar = Arc::new(AtomicBool::new(false));
        let parar_hilo = Arc::clone(&parar);

        thread::spawn(move || {
            for conexion in escucha.incoming() {
                if parar_hilo.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(flujo) = conexion else { continue };

                let serial = serial.clone();
                let ruta = ruta.clone();
                thread::spawn(move || {
                    atender(flujo, &serial, &ruta, tamano);
                });
            }
        });

        Some(Self {
            url: format!("http://127.0.0.1:{puerto}/v"),
            parar,
            puerto,
        })
    }
}

impl Drop for Servidor {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        // Una conexion de cortesia para desbloquear el accept().
        let _ = TcpStream::connect(("127.0.0.1", self.puerto));
    }
}

// ─────────────────────────── Interno ───────────────────────────

fn tamano_remoto(serial: &str, ruta: &str) -> Option<u64> {
    let salida = adb::shell(serial, &format!("stat -c %s \"{ruta}\""))?;
    salida.trim().parse::<u64>().ok()
}

fn atender(mut flujo: TcpStream, serial: &str, ruta: &str, tamano: u64) {
    let mut lector = BufReader::new(match flujo.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });

    let mut peticion = String::new();
    if lector.read_line(&mut peticion).is_err() {
        return;
    }

    let solo_cabecera = peticion.starts_with("HEAD");

    // Cabeceras: solo interesa Range.
    let mut desde: u64 = 0;
    let mut hasta: u64 = tamano - 1;

    loop {
        let mut linea = String::new();
        match lector.read_line(&mut linea) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => return,
        }
        if linea.trim().is_empty() {
            break;
        }
        if let Some(valor) = linea.to_lowercase().strip_prefix("range:") {
            if let Some((a, b)) = interpretar_rango(valor.trim(), tamano) {
                desde = a;
                hasta = b;
            }
        }
    }

    if desde >= tamano {
        let _ = flujo.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\n\r\n");
        return;
    }
    hasta = hasta.min(tamano - 1);
    let largo = hasta - desde + 1;

    let parcial = desde != 0 || hasta != tamano - 1;
    let estado = if parcial {
        "HTTP/1.1 206 Partial Content"
    } else {
        "HTTP/1.1 200 OK"
    };

    let mut cabecera = format!(
        "{estado}\r\n\
         Accept-Ranges: bytes\r\n\
         Content-Type: application/octet-stream\r\n\
         Content-Length: {largo}\r\n"
    );
    if parcial {
        cabecera.push_str(&format!(
            "Content-Range: bytes {desde}-{hasta}/{tamano}\r\n"
        ));
    }
    cabecera.push_str("Connection: close\r\n\r\n");

    if flujo.write_all(cabecera.as_bytes()).is_err() || solo_cabecera {
        return;
    }

    if let Some(datos) = leer_rango(serial, ruta, desde, largo) {
        let _ = flujo.write_all(&datos);
    }
    let _ = flujo.flush();
}

fn interpretar_rango(valor: &str, tamano: u64) -> Option<(u64, u64)> {
    let rango = valor.strip_prefix("bytes=")?;
    let (a, b) = rango.split_once('-')?;

    if a.is_empty() {
        // "-500": los ultimos 500 bytes. Asi busca FFmpeg el indice.
        let ultimos: u64 = b.trim().parse().ok()?;
        let desde = tamano.saturating_sub(ultimos);
        return Some((desde, tamano - 1));
    }

    let desde: u64 = a.trim().parse().ok()?;
    let hasta = if b.trim().is_empty() {
        tamano - 1
    } else {
        b.trim().parse().unwrap_or(tamano - 1)
    };

    Some((desde, hasta))
}

/// Lee un rango del archivo remoto. `dd` trabaja por bloques, asi que se
/// pide alineado y se recorta el sobrante.
fn leer_rango(serial: &str, ruta: &str, desde: u64, largo: u64) -> Option<Vec<u8>> {
    let bloque_inicial = desde / BLOQUE;
    let desplazamiento = (desde % BLOQUE) as usize;
    let bloques = (desplazamiento as u64 + largo).div_ceil(BLOQUE);

    let orden = format!(
        "dd if=\"{ruta}\" bs={BLOQUE} skip={bloque_inicial} count={bloques} 2>/dev/null"
    );

    let mut datos = adb::exec_out(serial, &orden)?;

    if desplazamiento >= datos.len() {
        return None;
    }
    datos.drain(..desplazamiento);
    datos.truncate(largo as usize);

    Some(datos)
}
