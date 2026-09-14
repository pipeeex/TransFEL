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

fn locate(binary: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dir = exe.parent().unwrap_or(Path::new("."));

    // 1) junto al ejecutable: ./tools/<bin>
    let installed = dir.join("tools").join(binary);
    if installed.exists() {
        return installed;
    }

    // 2) desarrollo: ../../tools/<bin>
    if let Some(dev) = dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("tools").join(binary))
    {
        if dev.exists() {
            return dev;
        }
    }

    // 3) PATH del sistema
    PathBuf::from(binary)
}

pub fn adb_path() -> &'static Path {
    ADB.get_or_init(|| locate(if cfg!(windows) { "adb.exe" } else { "adb" }))
}

pub fn ffmpeg_path() -> &'static Path {
    FFMPEG.get_or_init(|| locate(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }))
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