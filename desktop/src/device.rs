use crate::adb;

#[derive(Clone)]
pub struct DeviceInfo {
    pub serial: Option<String>,
    pub model: String,
    pub android: String,
    pub resolution: String,
    pub density: String,
    pub width: usize,
    pub height: usize,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self::disconnected()
    }
}

impl DeviceInfo {
    pub fn disconnected() -> Self {
        Self {
            serial: None,
            model: "-".to_string(),
            android: "-".to_string(),
            resolution: "-".to_string(),
            density: "-".to_string(),
            width: 720,
            height: 1280,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.serial.is_some()
    }

    /// Busca el primer dispositivo en estado "device".
    pub fn detect() -> Result<Self, String> {
        let list = adb::run(&["devices"])
            .ok_or_else(|| "No se pudo iniciar ADB".to_string())?;

        let serial = list
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut p = line.split_whitespace();
                let serial = p.next()?;
                let state = p.next()?;
                if state == "device" {
                    Some(serial.to_string())
                } else {
                    None
                }
            })
            .next()
            .ok_or_else(|| "Conecta tu dispositivo Android".to_string())?;

        let mut info = Self::disconnected();
        info.serial = Some(serial.clone());

        if let Some(v) = adb::getprop(&serial, "ro.product.model") {
            info.model = v;
        }
        if let Some(v) = adb::getprop(&serial, "ro.build.version.release") {
            info.android = v;
        }
        if let Some(v) = adb::shell(&serial, "wm size") {
            info.resolution = v.replace("Physical size:", "").trim().to_string();
            if let Some((w, h)) = parse_resolution(&info.resolution) {
                info.width = w;
                info.height = h;
            }
        }
        if let Some(v) = adb::shell(&serial, "wm density") {
            info.density = v.replace("Physical density:", "").trim().to_string();
        }

        setup_reverse(&serial);
        Ok(info)
    }
}

pub fn setup_reverse(serial: &str) {
    let ok = adb::run(&["-s", serial, "reverse", "tcp:5000", "tcp:5000"]).is_some();
    println!("ADB reverse: {}", if ok { "OK" } else { "fallo" });
}

pub fn parse_resolution(s: &str) -> Option<(usize, usize)> {
    let (w, h) = s.trim().split_once('x')?;
    let width = w.trim().parse::<usize>().ok()?;
    let height = h.trim().parse::<usize>().ok()?;
    Some((width, height))
}