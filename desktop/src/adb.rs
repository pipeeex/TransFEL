use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

static ADB: OnceLock<PathBuf> = OnceLock::new();
static FFMPEG: OnceLock<PathBuf> = OnceLock::new();

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

/// Command sin ventana de consola (Windows) y sin heredar stdio.
pub fn silent(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(NO_WINDOW);
    }
    cmd
}

fn adb_name() -> &'static str {
    if cfg!(windows) { "adb.exe" } else { "adb" }
}

fn ffmpeg_name() -> &'static str {
    if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }
}

#[cfg(unix)]
fn asegurar_ejecutable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut permisos = meta.permissions();
        if permisos.mode() & 0o111 == 0 {
            permisos.set_mode(0o755);
            let _ = std::fs::set_permissions(path, permisos);
        }
    }
}
#[cfg(not(unix))]
fn asegurar_ejecutable(_path: &Path) {}

fn locate(binary: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dir = exe.parent().unwrap_or(Path::new(".")).to_path_buf();

    // 1) junto al ejecutable: ./tools/<bin>
    let instalado = dir.join("tools").join(binary);
    if instalado.exists() {
        asegurar_ejecutable(&instalado);
        return instalado;
    }

    // 2) macOS: TransFEL.app/Contents/Resources/tools/<bin>
    #[cfg(target_os = "macos")]
    {
        if let Some(padre) = dir.parent() {
            let recursos = padre.join("Resources").join("tools").join(binary);
            if recursos.exists() {
                asegurar_ejecutable(&recursos);
                return recursos;
            }
        }
    }

    // 3) desarrollo: ../../tools/<bin>
    if let Some(desarrollo) = dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("tools").join(binary))
    {
        if desarrollo.exists() {
            asegurar_ejecutable(&desarrollo);
            return desarrollo;
        }
    }

    // 4) PATH del sistema (lo normal en Linux y macOS)
    PathBuf::from(binary)
}

pub fn adb_path() -> &'static Path {
    ADB.get_or_init(|| locate(adb_name()))
}

pub fn ffmpeg_path() -> &'static Path {
    FFMPEG.get_or_init(|| locate(ffmpeg_name()))
}

/// Ejecuta `adb <args>` y devuelve stdout limpio.
pub fn run(args: &[&str]) -> Option<String> {
    let out = silent(adb_path())
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Ejecuta `adb -s <serial> shell <cmd>`.
pub fn shell(serial: &str, cmd: &str) -> Option<String> {
    run(&["-s", serial, "shell", cmd])
}

/// Lee una propiedad del sistema Android.
pub fn getprop(serial: &str, prop: &str) -> Option<String> {
    shell(serial, &format!("getprop {prop}"))
}


