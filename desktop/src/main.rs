// Sin ventana de consola en Windows. Solo en release: en depuracion
// conviene conservarla para ver los println! del decodificador.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adb;
mod app;
mod device;
mod files;
mod rango;
mod stream;
mod thumbs;
mod theme;
mod ui;
mod watcher;
mod wifi; 

use app::TransfelApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TransFEL")
            .with_icon(cargar_icono())
            .with_inner_size([1250.0, 800.0])
            .with_min_inner_size([950.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "TransFEL",
        options,
        Box::new(|cc| Ok(Box::new(TransfelApp::new(cc)))),
    )
}

/// Icono de la ventana y de la barra de tareas.
///
/// Va aparte del icono del ejecutable (que lo incrusta build.rs): este es el
/// que ve el sistema mientras la aplicacion esta abierta, y funciona igual en
/// Windows, Linux y macOS.
fn cargar_icono() -> egui::IconData {
    const PNG: &[u8] = include_bytes!("../../packaging/icono/transfel.png");

    match image::load_from_memory(PNG) {
        Ok(imagen) => {
            let rgba = imagen.into_rgba8();
            let (ancho, alto) = rgba.dimensions();
            egui::IconData {
                rgba: rgba.into_raw(),
                width: ancho,
                height: alto,
            }
        }
        Err(_) => egui::IconData::default(),
    }
}
