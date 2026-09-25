use crate::adb;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;


pub const WIFI_PORT: u16 = 5555;


#[derive(Debug)]
pub enum WifiEvent {
	Log(String),
	Success(String),
	Failure(String),
	Busy(bool),
}

pub struct WifiManager {
	pub ip_input: String,
	pub pair_address: String, 
	pub pair_code: String,
	pub log: Vec<(bool, String)>,
	pub busy: bool, 
	pub auto_reconnect: bool,

	tx: mpsc::Sender<WifiEvent>,
	rx: mpsc::Receiver<WifiEvent>,
} 

impl Default for WifiManager {
	fn default() -> Self {
		let (tx,rx) = mpsc::channel();
		let saved = load_last_address();

		let mut manager = Self {
			ip_input: saved.clone().unwrap_or_default(),
			pair_address: String::new(),
			pair_code: String::new(),
			log: Vec::new(),
			busy: false,
			auto_reconnect: true, 
			tx,
			rx,
		};

		//reconexion silenciosa
		if let Some(address) = saved {
			manager.push_log(false, format!("Reintentando {address}.."));
			manager.connect(&address);
		}
		manager
	} 
}

impl WifiManager{
	fn push_log(&mut self, error: bool, text: String){
		self.log.push((error, text));
		if self.log.len() > 40 {
			self.log.remove(0);
		}
	}

	fn spawn<F>(&mut self, job: F)
	where 
		F: FnOnce(&mpsc::Sender<WifiEvent>) + Send + 'static,
	{
		let tx = self.tx.clone();
		self.busy = true;
		thread::spawn(move || {
			let _ = tx.send(WifiEvent::Busy(true));
			job(&tx);
			let _ = tx.send(WifiEvent::Busy(false));
		});
	}

pub fn enable_from_usb(&mut self, serial: String) {
	self.spawn(move |tx| {
		let say = |m: &str| {
			let _ = tx.send(WifiEvent::Log(m.to_string()));
		};

		say("Abriendo puerto TCP");
		if adb:: run(&["-s", &serial, "tcpip", &WIFI_PORT.to_string()]).is_none(){
			let _  = tx.send(WifiEvent::Failure(
				"No se pudo abrir el puerto. Sigue conectado por cable".into(),
			));
			return; 
		}

		// El dispositivo se reinicia en modo TCP; hay que esperarlo.
        thread::sleep(Duration::from_millis(2500));


		say("Leyendo la ip del dispositivo...");
		let Some(ip) = device_ip(&serial) else {
			let _ = tx.send(WifiEvent::Failure(
				"No se pudo ver la ip. Asegurese q este conectaod a internet".into(),
			));
			return;
		};

		let address = format!("{ip}:{WIFI_PORT}");
		say(&format!("Conectando a {address}"));

		match try_connect(&address){
			Ok(msg) => {
				save_last_address(&address);
				let _ = tx.send(WifiEvent::Success(msg));
			}
			Err(msg) => {
				let _ = tx.send(WifiEvent::Failure(msg));
			}
		}
	});
}
pub fn connect(&mut self, address: &str) {
        let address = normalize(address);
        self.spawn(move |tx| {
            let _ = tx.send(WifiEvent::Log(format!("Conectando a {address}...")));
            match try_connect(&address) {
                Ok(msg) => {
                    save_last_address(&address);
                    let _ = tx.send(WifiEvent::Success(msg));
                }
                Err(msg) => {
                    let _ = tx.send(WifiEvent::Failure(msg));
                }
            }
        });
    }
 /// Emparejamiento de "Depuración inalambrica" (Android 11+), sin cable.
    pub fn pair(&mut self, address: &str, code: &str) {
        let address = address.trim().to_string();
        let code = code.trim().to_string();

        self.spawn(move |tx| {
            let _ = tx.send(WifiEvent::Log(format!("Emparejando con {address}...")));

            let output = adb::run(&["pair", &address, &code]);
            match output {
                Some(text) if text.to_lowercase().contains("successfully") => {
                    let _ = tx.send(WifiEvent::Log("Emparejado. Ahora conecta con la IP y el puerto de conexión (no el de emparejamiento).".into()));
                    let _ = tx.send(WifiEvent::Success("Emparejamiento correcto".into()));
                }
                Some(text) => {
                    let _ = tx.send(WifiEvent::Failure(first_line(&text)));
                }
                None => {
                    let _ = tx.send(WifiEvent::Failure(
                        "Falló el emparejamiento. Revisa el código y que la pantalla siga abierta.".into(),
                    ));
                }
            }
        });
    }

    pub fn disconnect(&mut self, serial: String) {
        self.spawn(move |tx| {
            let _ = adb::run(&["disconnect", &serial]);
            let _ = tx.send(WifiEvent::Success(format!("Desconectado de {serial}")));
        });
    }

    pub fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                WifiEvent::Log(text) => self.push_log(false, text),
                WifiEvent::Success(text) => self.push_log(false, format!("✔ {text}")),
                WifiEvent::Failure(text) => self.push_log(true, format!("✕ {text}")),
                WifiEvent::Busy(value) => self.busy = value,
            }
        }
    }
}

// ─────────────────────────── Helpers ───────────────────────────

fn normalize(address: &str) -> String {
    let a = address.trim();
    if a.contains(':') {
        a.to_string()
    } else {
        format!("{a}:{WIFI_PORT}")
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).trim().to_string()
}

fn try_connect(address: &str) -> Result<String, String> {
    // adb connect devuelve 0 incluso al fallar: hay que leer el texto.
    let text = adb::run(&["connect", address])
        .ok_or_else(|| "No se pudo ejecutar adb connect".to_string())?;
    let lower = text.to_lowercase();

    if lower.contains("connected to") {
        Ok(format!("Conectado a {address}"))
    } else if lower.contains("already connected") {
        Ok(format!("Ya estaba conectado a {address}"))
    } else if lower.contains("failed to authenticate") {
        Err("El dispositivo no autorizó la conexión. Conéctalo por USB una vez y acepta el aviso.".into())
    } else {
        Err(first_line(&text))
    }
}

/// Lee la IP de la interfaz WiFi del dispositivo.
pub fn device_ip(serial: &str) -> Option<String> {
    //   la interfaz wlan0 directamente.
    if let Some(out) = adb::shell(serial, "ip -f inet addr show wlan0") {
        if let Some(ip) = out
            .split_whitespace()
            .skip_while(|t| *t != "inet")
            .nth(1)
            .and_then(|cidr| cidr.split('/').next())
        {
            if valid_ip(ip) {
                return Some(ip.to_string());
            }
        }
    }

    // la ruta por defecto (funciona en ROMs que renombran la interfaz).
    if let Some(out) = adb::shell(serial, "ip route") {
        for line in out.lines() {
            if let Some(ip) = line.split_whitespace().skip_while(|t| *t != "src").nth(1) {
                if valid_ip(ip) {
                    return Some(ip.to_string());
                }
            }
        }
    }

    None
}

fn valid_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| p.parse::<u8>().is_ok())
        && !ip.starts_with("127.")
}

// ── Persistencia de la ultima dirección ──

fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(crate::files::home_dir);
    base.join("TransFEL")
}

fn load_last_address() -> Option<String> {
    let text = std::fs::read_to_string(config_path().join("wifi.txt")).ok()?;
    let trimmed = text.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn save_last_address(address: &str) {
    let dir = config_path();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("wifi.txt"), address);
}


