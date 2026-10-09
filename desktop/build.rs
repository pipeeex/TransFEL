fn main() {
    // Icono del ejecutable en Windows (el que ve el usuario en el explorador
    // y en la barra de tareas).
    #[cfg(windows)]
    {
        let icono = "../packaging/icono/transfel.ico";
        println!("cargo:rerun-if-changed={icono}");

        let mut recurso = winres::WindowsResource::new();
        recurso.set_icon(icono);

        if let Err(e) = recurso.compile() {
            println!("cargo:warning=No se pudo incrustar el icono: {e}");
        }
    }
}
