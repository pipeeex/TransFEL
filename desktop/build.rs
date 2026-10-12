fn main() {
    // Icono del ejecutable en Windows: el que se ve en el explorador,
    // en la barra de tareas y en los accesos directos.
    #[cfg(windows)]
    {
        let icono = "../packaging/icono/transfel.ico";
        println!("cargo:rerun-if-changed={icono}");

        let mut recurso = winresource::WindowsResource::new();
        recurso.set_icon(icono);

        // Si falla, no se aborta la compilacion: el icono tambien se carga
        // en tiempo de ejecucion y el instalador trae el suyo.
        if let Err(e) = recurso.compile() {
            println!("cargo:warning=No se pudo incrustar el icono: {e}");
        }
    }
}
