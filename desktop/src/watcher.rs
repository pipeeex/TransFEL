use crate::adb;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSummary {
    pub serial: String,
    pub state: String, 
    pub Wifi: bool, 
}



impl DeviceSummary {
    pub fn ready(&self) -> bool {
        self.state == "device"
    }

    pub fn label(&self) -> String {
        if self.Wifi {
            format!("📶  {}", self.serial)
        }else {
            format!("🔌  {}", self.serial)
        }
    }
}


#[derive(Debug, Clone)]
pub enum DeviceEvent{
    Devices(Vec<DeviceSummary>),
}


/// Escucha el servidor ADB y avisa en cuanto entra o sale un dispositivo.
pub fn spawn() -> mpsc::Receiver<DeviceEvent> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut last: Vec<DeviceSummary> = Vec::new();

        loop {
            // Asegura que el servidor ADB este vivo.
            let _ = adb::run(&["start-server"]);

            if let Ok(stream) = TcpStream::connect("127.0.0.1:5037") {
                if track(stream, &tx, &mut last).is_err() {
                    // se cayo el socket, reintenta
                }
            } else {
                // Fallback: sondeo cada 2s
                poll_once(&tx, &mut last);
            }

            thread::sleep(Duration::from_millis(2000));
        }
    });

    rx
}

fn track(
    mut stream: TcpStream,
    tx: &mpsc::Sender<DeviceEvent>,
    last: &mut Vec<DeviceSummary>, 
) -> std::io::Result<()> {
    // Protocolo ADB: longitud en 4 dgitos hex + payload.

    let msg = "host:track-devices";
    stream.write_all(format!("{:04x}{}", msg.len(), msg).as_bytes())?;
    stream.flush()?;

    let mut status = [0u8; 4];
    stream.read_exact(&mut status)?;
    if &status != b"OKAY" {
        return Err(std::io::Error::other("ADB rechazo track-devices"));
    }

    loop {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;

        let len = usize::from_str_radix(
            std::str::from_utf8(&len_buf).unwrap_or("0000"),
            16,
        )
        .unwrap_or(0);

        let mut payload = vec![0u8; len];
        if len > 0 {
            stream.read_exact(&mut payload)?;
        }

        let text = String::from_utf8_lossy(&payload);
        emit(&text, tx, last);
    }
}

fn poll_once(tx: &mpsc::Sender<DeviceEvent>, last: &mut Vec<DeviceSummary>) {
    let text = adb::run(&["devices"]).unwrap_or_default();
    // `adb devices` trae cabecera; la quitamos para reusar el mismo parser.
    let body: String = text.lines().skip(1).collect::<Vec<_>>().join("\n");
    emit(&body, tx, last);
}

fn emit(text: &str, tx: &mpsc::Sender<DeviceEvent>, last: &mut Vec<DeviceSummary>) {
    let current: Vec<DeviceSummary> = text
        .lines()
        .filter_map(|line| {
            let mut p = line.split_whitespace();
            let serial = p.next()?.to_string();
            let state = p.next()?.to_string();

            if serial.is_empty(){
                return None; 
            }
            Some(DeviceSummary {
                Wifi: serial.contains(':'),
                serial,
                state,
            })
        })
        .collect();

    if current == *last {
        return;
    }

    let _ = tx.send(DeviceEvent::Devices(current.clone()));
    *last = current;
}