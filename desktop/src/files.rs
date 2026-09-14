use crate::adb;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

// ─────────────────────────────── Tipos ───────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Pc,
    Android,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Todos,
    Fotos,
    Documentos,
    Video,
    Audio,
    Otros,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Todos,
        Category::Fotos,
        Category::Documentos,
        Category::Video,
        Category::Audio,
        Category::Otros,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Todos => "Todos",
            Category::Fotos => "Fotos",
            Category::Documentos => "Documentos",
            Category::Video => "Video",
            Category::Audio => "Audio",
            Category::Otros => "Otros",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Category::Todos => "▤",
            Category::Fotos => "🖼",
            Category::Documentos => "📄",
            Category::Video => "🎬",
            Category::Audio => "🎵",
            Category::Otros => "◻",
        }
    }
}

pub fn category_of(name: &str) -> Category {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "heif") => Category::Fotos,
        Some("pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "csv"
            | "odt" | "rtf") => Category::Documentos,
        Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "3gp") => Category::Video,
        Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus") => Category::Audio,
        _ => Category::Otros,
    }
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,   // ruta completa (PC o Android)
    pub is_dir: bool,
    pub size: u64,
    pub category: Category,
}

impl FileEntry {
    pub fn icon(&self) -> &'static str {
        if self.is_dir { "📁" } else { self.category.icon() }
    }

    pub fn size_label(&self) -> String {
        if self.is_dir {
            return "—".into();
        }
        human_size(self.size)
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

// ──────────────────────── Bandeja y transferencias ────────────────────────

/// Archivo "cargado al programa", listo para transferir.
#[derive(Clone)]
pub struct StagedFile {
    pub name: String,
    pub path: String,
    pub origin: Side,
    pub size: u64,
    pub category: Category,
}

#[derive(Clone, PartialEq)]
pub enum TransferState {
    Pendiente,
    EnCurso,
    Listo,
    Error(String),
}

pub struct Transfer {
    pub name: String,
    pub direction: Side, // destino
    pub state: TransferState,
}

pub enum TransferMsg {
    Started(usize),
    Finished(usize, Result<(), String>),
}

// ─────────────────────────── Gestor principal ───────────────────────────

pub struct FileManager {
    pub side: Side,
    pub category: Category,
    pub search: String,

    pub pc_path: PathBuf,
    pub pc_entries: Vec<FileEntry>,

    pub android_path: String,
    pub android_entries: Vec<FileEntry>,

    pub staged: Vec<StagedFile>,
    pub transfers: Vec<Transfer>,

    pub error: Option<String>,

    tx: mpsc::Sender<TransferMsg>,
    rx: mpsc::Receiver<TransferMsg>,
}

impl Default for FileManager {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut fm = Self {
            side: Side::Pc,
            category: Category::Todos,
            search: String::new(),
            pc_path: dirs_home(),
            pc_entries: Vec::new(),
            android_path: "/sdcard".into(),
            android_entries: Vec::new(),
            staged: Vec::new(),
            transfers: Vec::new(),
            error: None,
            tx,
            rx,
        };
        fm.refresh_pc();
        fm
    }
}

fn dirs_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

impl FileManager {
    // ── Listados ────────────────────────────────────────────────────────

    pub fn refresh_pc(&mut self) {
        self.pc_entries.clear();
        let Ok(read) = std::fs::read_dir(&self.pc_path) else {
            self.error = Some("No se pudo leer la carpeta".into());
            return;
        };

        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map_or(false, |m| m.is_dir());
            self.pc_entries.push(FileEntry {
                category: if is_dir { Category::Otros } else { category_of(&name) },
                size: meta.map_or(0, |m| m.len()),
                path: entry.path().to_string_lossy().to_string(),
                name,
                is_dir,
            });
        }
        sort_entries(&mut self.pc_entries);
        self.error = None;
    }

    pub fn refresh_android(&mut self, serial: &str) {
        self.android_entries.clear();

        let base = self.android_path.trim_end_matches('/');
        let cmd = format!("ls -la \"{}/\"", base);

        let Some(out) = adb::shell(serial, &cmd) else {
            self.error = Some("No se pudo listar el dispositivo".to_string());
            return;
        };

        for line in out.lines() {
            if line.starts_with("total "){
                continue;
            }
            if let Some(entry) = parse_ls_line(line, &self.android_path) {
                self.android_entries.push(entry);
            }
        }
        sort_entries(&mut self.android_entries);
        self.error = None;
    }

    pub fn refresh_current(&mut self, serial: Option<&str>) {
        match self.side {
            Side::Pc => self.refresh_pc(),
            Side::Android => {
                if let Some(s) = serial {
                    self.refresh_android(s);
                }
            }
        }
    }

    // ── Navegacion ──────────────────────────────────────────────────────

    pub fn enter(&mut self, entry: &FileEntry, serial: Option<&str>) {
        match self.side {
            Side::Pc => {
                self.pc_path = PathBuf::from(&entry.path);
                self.refresh_pc();
            }
            Side::Android => {
                self.android_path = entry.path.clone();
                if let Some(s) = serial {
                    self.refresh_android(s);
                }
            }
        }
    }

    pub fn go_up(&mut self, serial: Option<&str>) {
        match self.side {
            Side::Pc => {
                if let Some(parent) = self.pc_path.parent() {
                    self.pc_path = parent.to_path_buf();
                    self.refresh_pc();
                }
            }
            Side::Android => {
                if self.android_path != "/" {
                    let parent = self
                        .android_path
                        .rsplit_once('/')
                        .map(|(p, _)| if p.is_empty() { "/" } else { p })
                        .unwrap_or("/")
                        .to_string();
                    self.android_path = parent;
                    if let Some(s) = serial {
                        self.refresh_android(s);
                    }
                }
            }
        }
    }

    pub fn current_path_label(&self) -> String {
        match self.side {
            Side::Pc => self.pc_path.to_string_lossy().to_string(),
            Side::Android => self.android_path.clone(),
        }
    }

    /// Listado ya filtrado por categoria + busqueda.
    pub fn visible(&self) -> Vec<&FileEntry> {
        let needle = self.search.to_lowercase();
        let source = match self.side {
            Side::Pc => &self.pc_entries,
            Side::Android => &self.android_entries,
        };

        source
            .iter()
            .filter(|e| {
                if !needle.is_empty() && !e.name.to_lowercase().contains(&needle) {
                    return false;
                }
                if e.is_dir {
                    return self.category == Category::Todos;
                }
                self.category == Category::Todos || e.category == self.category
            })
            .collect()
    }

    // ── Bandeja ─────────────────────────────────────────────────────────

    pub fn is_staged(&self, path: &str) -> bool {
        self.staged.iter().any(|s| s.path == path)
    }

    pub fn stage(&mut self, entry: &FileEntry) {
        if entry.is_dir || self.is_staged(&entry.path) {
            return;
        }
        self.staged.push(StagedFile {
            name: entry.name.clone(),
            path: entry.path.clone(),
            origin: self.side,
            size: entry.size,
            category: entry.category,
        });
    }

    pub fn unstage(&mut self, path: &str) {
        self.staged.retain(|s| s.path != path);
    }

    /// Aade archivos del PC con el dialogo nativo.
    pub fn stage_from_dialog(&mut self) {
        let Some(paths) = rfd::FileDialog::new().pick_files() else {
            return;
        };
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let path = path.to_string_lossy().to_string();
            if self.is_staged(&path) {
                continue;
            }
            let size = std::fs::metadata(&path).map_or(0, |m| m.len());
            self.staged.push(StagedFile {
                category: category_of(&name),
                name,
                path,
                origin: Side::Pc,
                size,
            });
        }
    }

    pub fn go_home(&mut self) {
        match self.side {
        Side::Pc => self.pc_path = dirs_home(),
        Side::Android => self.android_path = "/sdcard".to_string(),
        }
    }

    // ── Transferencias ──────────────────────────────────────────────────

    /// Envia todo lo que hay en la bandeja hacia el lado contrario a su origen.
    pub fn transfer_staged(&mut self, serial: &str, android_dest: &str, pc_dest: &PathBuf) {
        let staged = std::mem::take(&mut self.staged);

        for file in staged {
            let index = self.transfers.len();
            let destino = match file.origin {
                Side::Pc => Side::Android,
                Side::Android => Side::Pc,
            };

            self.transfers.push(Transfer {
                name: file.name.clone(),
                direction: destino,
                state: TransferState::Pendiente,
            });

            let tx = self.tx.clone();
            let serial = serial.to_string();
            let src = file.path.clone();
            let dst = match destino {
                Side::Android => android_dest.trim_end_matches('/').to_string(),
                Side::Pc => pc_dest.to_string_lossy().to_string(),
            };

            thread::spawn(move || {
                let _ = tx.send(TransferMsg::Started(index));

                let args: Vec<&str> = match destino {
                    Side::Android => vec!["-s", &serial, "push", &src, &dst],
                    Side::Pc => vec!["-s", &serial, "pull", &src, &dst],
                };

                let result = adb::run(&args)
                    .map(|_| ())
                    .ok_or_else(|| "adb falló (revisa permisos o ruta)".to_string());

                let _ = tx.send(TransferMsg::Finished(index, result));
            });
        }
    }

    /// Llamar una vez por frame para refrescar estados.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(msg) = self.rx.try_recv() {
            changed = true;
            match msg {
                TransferMsg::Started(i) => {
                    if let Some(t) = self.transfers.get_mut(i) {
                        t.state = TransferState::EnCurso;
                    }
                }
                TransferMsg::Finished(i, res) => {
                    if let Some(t) = self.transfers.get_mut(i) {
                        t.state = match res {
                            Ok(()) => TransferState::Listo,
                            Err(e) => TransferState::Error(e),
                        };
                    }
                }
            }
        }
        changed
    }

    pub fn active_transfers(&self) -> usize {
        self.transfers
            .iter()
            .filter(|t| matches!(t.state, TransferState::Pendiente | TransferState::EnCurso))
            .count()
    }
}

// ─────────────────────────── Utilidades ───────────────────────────

fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// Parsea una linea de `ls -la` de toybox/Android.
/// Ej: `-rw-rw---- 1 root sdcard_rw 12345 2024-01-05 10:33 foto.jpg`
fn parse_ls_line(line: &str, base: &str) -> Option<FileEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 8 {
        return None;
    }

    let perms = parts[0];
    if !(perms.starts_with('-') || perms.starts_with('d') || perms.starts_with('l')) {
        return None;
    }


    // links simbolicos: "nombre -> destino"
    let is_link = perms.starts_with('l');
    let mut is_dir = perms.starts_with('d');
    let size = parts[4].parse::<u64>().unwrap_or(0);

    let raw = parts[7..].join(" ");
    let name = raw.split(" -> ").next().unwrap_or(&raw);;

    let name = name.rsplit('/').next().unwrap_or(name).to_string();


    if name.is_empty() || name == "." || name == ".." || name.starts_with('.') {
        return None;
    }

    let path = if base == "/" {
        format!("/{name}")
    } else {
        format!("{}/{}", base.trim_end_matches('/'), name)
    };

    Some(FileEntry {
        category: if is_dir { Category::Otros } else { category_of(&name) },
        name,
        path,
        is_dir,
        size,
    })
}