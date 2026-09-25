use crate::adb;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const MAX_RESULTS: usize = 600;
const DEBOUNCE: Duration = Duration::from_millis(350);

// ─────────────────────────────── Tipos ───────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Pc,
    Android,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Carpeta,
    Dispositivo,
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

    /// Extensiones para construir el `find` de Android.
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Category::Fotos => &["jpg", "jpeg", "png", "gif", "webp", "bmp", "heic", "heif"],
            Category::Documentos => &[
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "csv", "odt",
                "rtf", "epub",
            ],
            Category::Video => &["mp4", "mkv", "avi", "mov", "webm", "3gp"],
            Category::Audio => &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"],
            _ => &[],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AndroidDest {
    Descargas,
    Camara,
    Documentos,
    Musica,
    Videos,
    Actual,
}

impl AndroidDest {
    pub const ALL: [AndroidDest; 6] = [
        AndroidDest::Descargas,
        AndroidDest::Camara,
        AndroidDest::Documentos,
        AndroidDest::Musica,
        AndroidDest::Videos,
        AndroidDest::Actual,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AndroidDest::Descargas => "⬇  Descargas",
            AndroidDest::Camara => "📷  Cámara (DCIM)",
            AndroidDest::Documentos => "📄  Documentos",
            AndroidDest::Musica => "🎵  Música",
            AndroidDest::Videos => "🎬  Películas",
            AndroidDest::Actual => "📂  Carpeta abierta",
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            AndroidDest::Descargas => "/sdcard/Download",
            AndroidDest::Camara => "/sdcard/DCIM/Camera",
            AndroidDest::Documentos => "/sdcard/Documents",
            AndroidDest::Musica => "/sdcard/Music",
            AndroidDest::Videos => "/sdcard/Movies",
            AndroidDest::Actual => "",
        }
    }
}

pub fn default_pc_dest() -> PathBuf {
    home_dir().join("Downloads").join("TransFEL")
}

pub fn category_of(name: &str) -> Category {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "heif") => Category::Fotos,
        Some(
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "csv"
            | "odt" | "rtf" | "epub",
        ) => Category::Documentos,
        Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "3gp") => Category::Video,
        Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus") => Category::Audio,
        _ => Category::Otros,
    }
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub category: Category,
}

impl FileEntry {
    pub fn icon(&self) -> &'static str {
        if self.is_dir { "📁" } else { self.category.icon() }
    }

    pub fn size_label(&self) -> String {
        if self.is_dir || self.size == 0 {
            "—".to_string()
        } else {
            human_size(self.size)
        }
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

// ─────────────────────────── Accesos rápidos ───────────────────────────

#[derive(Clone, Copy)]
pub enum Target {
    Dir(&'static str),
    Media(MediaKind),
}

#[derive(Clone, Copy)]
pub enum MediaKind {
    Images,
    Video,
    Audio,
}

impl MediaKind {
    fn uri(self) -> &'static str {
        match self {
            MediaKind::Images => "content://media/external/images/media",
            MediaKind::Video => "content://media/external/video/media",
            MediaKind::Audio => "content://media/external/audio/media",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Shortcut {
    pub icon: &'static str,
    pub label: &'static str,
    pub target: Target,
}

pub const ANDROID_SHORTCUTS: [Shortcut; 6] = [
    Shortcut { icon: "📷", label: "Camara",    target: Target::Dir("/sdcard/DCIM/Camera") },
    Shortcut { icon: "🖼", label: "Galeria",   target: Target::Media(MediaKind::Images) },
    Shortcut { icon: "⬇",  label: "Descargas", target: Target::Dir("/sdcard/Download") },
    Shortcut { icon: "🎬", label: "Videos",    target: Target::Media(MediaKind::Video) },
    Shortcut { icon: "🎵", label: "Musica",    target: Target::Media(MediaKind::Audio) },
    Shortcut { icon: "💬", label: "WhatsApp",  target: Target::Dir("/sdcard/Android/media/com.whatsapp/WhatsApp/Media") },
];

pub const PC_SHORTCUTS: [Shortcut; 4] = [
    Shortcut { icon: "⬇",  label: "Descargas", target: Target::Dir("Downloads") },
    Shortcut { icon: "🖥", label: "Escritorio", target: Target::Dir("Desktop") },
    Shortcut { icon: "📄", label: "Documentos", target: Target::Dir("Documents") },
    Shortcut { icon: "🖼", label: "Imagenes",   target: Target::Dir("Pictures") },
];

// ─────────────────────────── Bandeja / transferencias ───────────────────────────

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
    pub direction: Side,
    pub state: TransferState,
}

// ─────────────────────────── Mensajeria con los workers ───────────────────────────

enum Job {
    ListPc(PathBuf),
    ListAndroid { serial: String, path: String },
    SearchPc { root: PathBuf, query: String, category: Category },
    SearchAndroid { serial: String, query: String, category: Category },
    MediaAndroid { serial: String, kind: MediaKind },
    Push { index: usize, serial: String, src: String, dst: String },
    Pull { index: usize, serial: String, src: String, dst: String },
}

enum Event {
    Result { token: u64, key: Option<String>, entries: Vec<FileEntry> },
    Failed { token: u64, message: String },
    TransferStarted(usize),
    TransferDone(usize, Result<(), String>),
}

// ─────────────────────────── Gestor ───────────────────────────

pub struct FileManager {
    pub side: Side,
    pub category: Category,
    pub scope: Scope,
    pub search: String,

    pub pc_path: PathBuf,
    pub android_path: String,

    pub pc_dest: PathBuf,
    pub android_dest: AndroidDest,
    pub notice: Option<String>,

    pub entries: Vec<FileEntry>,
    pub loading: bool,
    pub result_label: Option<String>,
    pub error: Option<String>,

    pub staged: Vec<StagedFile>,
    pub transfers: Vec<Transfer>,

    cache: HashMap<String, Vec<FileEntry>>,
    token: Arc<AtomicU64>,
    last_key: Option<String>,
    pending_search: Option<Instant>,
    last_query: String,

    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
}

impl Default for FileManager {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut fm = Self {
            side: Side::Pc,
            category: Category::Todos,
            scope: Scope::Carpeta,
            search: String::new(),
            pc_path: home_dir(),
            android_path: "/sdcard".to_string(),
            pc_dest: default_pc_dest(),
            android_dest: AndroidDest::Descargas,
            notice: None,
            entries: Vec::new(),
            loading: false,
            result_label: None,
            error: None,
            staged: Vec::new(),
            transfers: Vec::new(),
            cache: HashMap::new(),
            token: Arc::new(AtomicU64::new(0)),
            last_key: None,
            pending_search: None,
            last_query: String::new(),
            tx,
            rx,
        };
        fm.reload(None);
        fm
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

impl FileManager {
    // ── Despacho ────────────────────────────────────────────────────

    fn bump(&mut self) -> u64 {
        self.token.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn dispatch(&self, job: Job, token: u64, key: Option<String>) {
        let tx = self.tx.clone();
        thread::spawn(move || run_job(job, token, key, tx));
    }

    fn key(&self) -> String {
        match self.side {
            Side::Pc => format!("pc:{}", self.pc_path.to_string_lossy()),
            Side::Android => format!("an:{}", self.android_path),
        }
    }

    /// Recarga la vista actual. Si hay cache, la muestra YA y refresca detras.
    pub fn reload(&mut self, serial: Option<&str>) {
        let key = self.key();
        let token = self.bump();
        self.result_label = None;
        self.last_key = Some(key.clone());

        if let Some(cached) = self.cache.get(&key) {
            self.entries = cached.clone();
            self.loading = false;
        } else {
            self.entries.clear();
            self.loading = true;
        }
        self.error = None;

        let job = match self.side {
            Side::Pc => Job::ListPc(self.pc_path.clone()),
            Side::Android => {
                let Some(serial) = serial else {
                    self.loading = false;
                    self.error = Some("Sin dispositivo".to_string());
                    return;
                };
                Job::ListAndroid { serial: serial.to_string(), path: self.android_path.clone() }
            }
        };

        self.dispatch(job, token, Some(key));
    }

    // ── Navegacion ──────────────────────────────────────────────────

    pub fn enter(&mut self, entry: &FileEntry, serial: Option<&str>) {
        match self.side {
            Side::Pc => self.pc_path = PathBuf::from(&entry.path),
            Side::Android => self.android_path = entry.path.clone(),
        }
        self.scope = Scope::Carpeta;
        self.search.clear();
        self.reload(serial);
    }

    pub fn go_up(&mut self, serial: Option<&str>) {
        match self.side {
            Side::Pc => {
                if let Some(p) = self.pc_path.parent() {
                    self.pc_path = p.to_path_buf();
                }
            }
            Side::Android => {
                if self.android_path != "/" {
                    self.android_path = self
                        .android_path
                        .rsplit_once('/')
                        .map(|(p, _)| if p.is_empty() { "/" } else { p })
                        .unwrap_or("/")
                        .to_string();
                }
            }
        }
        self.reload(serial);
    }

    pub fn go_home(&mut self, serial: Option<&str>) {
        match self.side {
            Side::Pc => self.pc_path = home_dir(),
            Side::Android => self.android_path = "/sdcard".to_string(),
        }
        self.reload(serial);
    }

    pub fn open_shortcut(&mut self, sc: &Shortcut, serial: Option<&str>) {
        self.search.clear();
        self.scope = Scope::Carpeta;

        match sc.target {
            Target::Dir(dir) => {
                match self.side {
                    Side::Pc => self.pc_path = home_dir().join(dir),
                    Side::Android => self.android_path = dir.to_string(),
                }
                self.reload(serial);
            }
            Target::Media(kind) => {
                let Some(serial) = serial else { return };
                let token = self.bump();
                self.entries.clear();
                self.loading = true;
                self.error = None;
                self.result_label = Some(format!("{} (indice del sistema)", sc.label));
                self.dispatch(
                    Job::MediaAndroid { serial: serial.to_string(), kind },
                    token,
                    None,
                );
            }
        }
    }

    pub fn current_path_label(&self) -> String {
        if let Some(label) = &self.result_label {
            return label.clone();
        }
        match self.side {
            Side::Pc => self.pc_path.to_string_lossy().to_string(),
            Side::Android => self.android_path.clone(),
        }
    }

    pub fn switch_side(&mut self, side: Side, serial: Option<&str>) {
        if self.side == side {
            return;
        }
        self.side = side;
        self.search.clear();
        self.scope = Scope::Carpeta;
        self.reload(serial);
    }

    // ── Busqueda ────────────────────────────────────────────────────

    /// Llamar cuando cambie el texto o el ambito.
    pub fn on_search_changed(&mut self) {
        if self.scope == Scope::Dispositivo {
            self.pending_search = Some(Instant::now());
        }
    }

    pub fn set_scope(&mut self, scope: Scope, serial: Option<&str>) {
        if self.scope == scope {
            return;
        }
        self.scope = scope;
        match scope {
            Scope::Carpeta => self.reload(serial),
            Scope::Dispositivo => self.pending_search = Some(Instant::now()),
        }
    }

    fn launch_search(&mut self, serial: Option<&str>) {
        let query = self.search.trim().to_string();
        if query.len() < 2 && self.category == Category::Todos {
            return;
        }

        let token = self.bump();
        self.entries.clear();
        self.loading = true;
        self.error = None;
        self.result_label = Some(format!("Busqueda global: \"{query}\""));
        self.last_query = query.clone();

        let job = match self.side {
            Side::Pc => Job::SearchPc {
                root: home_dir(),
                query,
                category: self.category,
            },
            Side::Android => {
                let Some(serial) = serial else {
                    self.loading = false;
                    return;
                };
                Job::SearchAndroid {
                    serial: serial.to_string(),
                    query,
                    category: self.category,
                }
            }
        };

        self.dispatch(job, token, None);
    }

    /// Filtro local sobre lo que ya esta cargado.
    pub fn visible(&self) -> Vec<&FileEntry> {
        let needle = self.search.to_lowercase();
        let filter_name = self.scope == Scope::Carpeta && !needle.is_empty();

        self.entries
            .iter()
            .filter(|e| {
                if filter_name && !e.name.to_lowercase().contains(&needle) {
                    return false;
                }
                if e.is_dir {
                    return self.category == Category::Todos;
                }
                self.category == Category::Todos || e.category == self.category
            })
            .collect()
    }

    // ── Bandeja ─────────────────────────────────────────────────────
    pub fn tray_origin(&self) -> Option<Side> {
        self.staged.first().map(|f| f.origin)
    }

    pub fn clear_tray(&mut self) {
        self.staged.clear();
        self.notice = None;
    }

    /// Lado al que irán los archivos de la bandeja.
    pub fn tray_destination(&self) -> Option<Side> {
        match self.tray_origin()? {
            Side::Pc => Some(Side::Android),
            Side::Android => Some(Side::Pc),
        }
    }

    pub fn transfer_label(&self) -> String {
        let n = self.staged.len();
        match self.tray_origin() {
            Some(Side::Android) => format!("⬅   Traer {n} al PC"),
            Some(Side::Pc) => format!("➡   Enviar {n} al celular"),
            None => "Selecciona archivos".to_string(),
        }
    }


    pub fn is_staged(&self, path: &str) -> bool {
        self.staged.iter().any(|s| s.path == path)
    }

    pub fn stage(&mut self, entry: &FileEntry) {
        if entry.is_dir || self.is_staged(&entry.path) {
            return;
        }

        if let Some(origin) = self.tray_origin() {
            if origin != self.side {
                self.notice = Some(
                    "La bandeja ya tiene archivos del otro dispositivo. Transfierelos o vaciala primero.".to_string(),
                );
                return;
            }
        }

        self.notice = None;
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

    pub fn stage_all_visible(&mut self) {
        let items: Vec<FileEntry> = self.visible().into_iter().cloned().collect();
        for e in items {
            self.stage(&e);
        }
    }

    pub fn stage_from_dialog(&mut self) {
        if self.tray_origin() == Some(Side::Android) {
            self.notice = Some(
                "La bandeja tiene archivos del celular. Vacíala para cargar desde el PC.".to_string(),
            );
            return;
        }

        let Some(paths) = rfd::FileDialog::new().pick_files() else {
            return;
        };

        self.notice = None;
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let p = path.to_string_lossy().to_string();
            if self.is_staged(&p) {
                continue;
            }
            let size = std::fs::metadata(&path).map_or(0, |m| m.len());
            self.staged.push(StagedFile {
                category: category_of(&name),
                name,
                path: p,
                origin: Side::Pc,
                size,
            });
        }
    }

    pub fn pick_pc_dest(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_directory(&self.pc_dest)
            .pick_folder()
        {
            self.pc_dest = dir;
        }
    }

    /// Cuántos elementos se ocultan por los filtros activos.
    pub fn filter_summary(&self) -> (usize, usize) {
        (self.visible().len(), self.entries.len())
    }

    pub fn filters_active(&self) -> bool {
        self.category != Category::Todos || !self.search.trim().is_empty()
    }

    pub fn clear_filters(&mut self, serial: Option<&str>) {
        self.category = Category::Todos;
        self.search.clear();
        if self.scope == Scope::Dispositivo {
            self.set_scope(Scope::Carpeta, serial);
        }
    }


    // ── Transferencias ──────────────────────────────────────────────

    pub fn transfer_staged(&mut self, serial: &str) {
        let staged = std::mem::take(&mut self.staged);
        self.notice = None;

        let android_dest = match self.android_dest {
            AndroidDest::Actual => self.android_path.trim_end_matches('/').to_string(),
            other => other.path().to_string(),
        };

        let pc_dest = self.pc_dest.clone();
        let _ = std::fs::create_dir_all(&pc_dest);

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

            let job = match destino {
                Side::Android => Job::Push {
                    index,
                    serial: serial.to_string(),
                    src: file.path.clone(),
                    dst: android_dest.clone(),
                },
                Side::Pc => Job::Pull {
                    index,
                    serial: serial.to_string(),
                    src: file.path.clone(),
                    dst: pc_dest.to_string_lossy().to_string(),
                },
            };

            self.dispatch(job, 0, None);
        }
    }

    pub fn active_transfers(&self) -> usize {
        self.transfers
            .iter()
            .filter(|t| matches!(t.state, TransferState::Pendiente | TransferState::EnCurso))
            .count()
    }

    // ── Bombeo por frame ────────────────────────────────────────────

    pub fn poll(&mut self, serial: Option<&str>) {
        // Debounce de la busqueda global.
        if let Some(at) = self.pending_search {
            if at.elapsed() >= DEBOUNCE {
                self.pending_search = None;
                self.launch_search(serial);
            }
        }

        let current = self.token.load(Ordering::Relaxed);
        let mut refresh_after_transfer = false;

        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Result { token, key, entries } => {
                    if token < current {
                        continue; // resultado viejo, se descarta
                    }
                    if let Some(k) = key {
                        self.cache.insert(k, entries.clone());
                    }
                    self.entries = entries;
                    self.loading = false;
                    self.error = None;
                }
                Event::Failed { token, message } => {
                    if token < current {
                        continue;
                    }
                    self.loading = false;
                    self.error = Some(message);
                }
                Event::TransferStarted(i) => {
                    if let Some(t) = self.transfers.get_mut(i) {
                        t.state = TransferState::EnCurso;
                    }
                }
                Event::TransferDone(i, res) => {
                    if let Some(t) = self.transfers.get_mut(i) {
                        t.state = match res {
                            Ok(()) => TransferState::Listo,
                            Err(e) => TransferState::Error(e),
                        };
                    }
                    refresh_after_transfer = true;
                }
            }
        }

        if refresh_after_transfer && self.active_transfers() == 0 {
            self.cache.clear();
            self.reload(serial);
        }
    }

    pub fn invalidate(&mut self) {
        self.cache.clear();
    }
}

// ─────────────────────────── Workers ───────────────────────────

fn run_job(job: Job, token: u64, key: Option<String>, tx: mpsc::Sender<Event>) {
    let event = match job {
        Job::ListPc(path) => match list_pc(&path) {
            Ok(entries) => Event::Result { token, key, entries },
            Err(e) => Event::Failed { token, message: e },
        },
        Job::ListAndroid { serial, path } => match list_android(&serial, &path) {
            Ok(entries) => Event::Result { token, key, entries },
            Err(e) => Event::Failed { token, message: e },
        },
        Job::SearchPc { root, query, category } => {
            Event::Result { token, key: None, entries: search_pc(&root, &query, category) }
        }
        Job::SearchAndroid { serial, query, category } => {
            match search_android(&serial, &query, category) {
                Ok(entries) => Event::Result { token, key: None, entries },
                Err(e) => Event::Failed { token, message: e },
            }
        }
        Job::MediaAndroid { serial, kind } => match media_query(&serial, kind) {
            Ok(entries) => Event::Result { token, key: None, entries },
            Err(e) => Event::Failed { token, message: e },
        },
        Job::Push { index, serial, src, dst } => {
            let _ = tx.send(Event::TransferStarted(index));
            let res = adb::run(&["-s", &serial, "push", &src, &dst])
                .map(|_| ())
                .ok_or_else(|| "adb push falló".to_string());
            let _ = tx.send(Event::TransferDone(index, res));
            return;
        }
        Job::Pull { index, serial, src, dst } => {
            let _ = tx.send(Event::TransferStarted(index));
            let res = adb::run(&["-s", &serial, "pull", &src, &dst])
                .map(|_| ())
                .ok_or_else(|| "adb pull falló".to_string());
            let _ = tx.send(Event::TransferDone(index, res));
            return;
        }
    };

    let _ = tx.send(event);
}

// ── PC ──────────────────────────────────────────────────────────────

fn list_pc(path: &PathBuf) -> Result<Vec<FileEntry>, String> {
    let read = std::fs::read_dir(path).map_err(|e| format!("No se pudo leer: {e}"))?;
    let mut out = Vec::new();

    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map_or(false, |m| m.is_dir());
        out.push(FileEntry {
            category: if is_dir { Category::Otros } else { category_of(&name) },
            size: meta.map_or(0, |m| m.len()),
            path: entry.path().to_string_lossy().to_string(),
            name,
            is_dir,
        });
    }

    sort_entries(&mut out);
    Ok(out)
}

fn search_pc(root: &PathBuf, query: &str, category: Category) -> Vec<FileEntry> {
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];

    while let Some(dir) = stack.pop() {
        if out.len() >= MAX_RESULTS {
            break;
        }
        let Ok(read) = std::fs::read_dir(&dir) else { continue };

        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" || name == "AppData" {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };

            if meta.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                continue;
            }
            let cat = category_of(&name);
            if category != Category::Todos && cat != category {
                continue;
            }
            out.push(FileEntry {
                path: entry.path().to_string_lossy().to_string(),
                name,
                is_dir: false,
                size: meta.len(),
                category: cat,
            });

            if out.len() >= MAX_RESULTS {
                break;
            }
        }
    }

    sort_entries(&mut out);
    out
}

// ── Android ─────────────────────────────────────────────────────────

fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\'' | '`' | '$' | '\\' | ';' | '&' | '|' | '\n'))
        .collect()
}

fn list_android(serial: &str, path: &str) -> Result<Vec<FileEntry>, String> {
    let base = path.trim_end_matches('/');
    // La barra final es clave: /sdcard es un symlink.
    let cmd = format!("ls -la \"{}/\"", sanitize(base));
    let out = adb::shell(serial, &cmd).ok_or("No se pudo listar el dispositivo")?;

    let mut entries = Vec::new();
    for line in out.lines() {
        if line.starts_with("total ") {
            continue;
        }
        if let Some(e) = parse_ls_line(line, path) {
            entries.push(e);
        }
    }
    sort_entries(&mut entries);
    Ok(entries)
}

/// Busqueda global. Para fotos/video/audio usa el índice MediaStore (instantaneo);
/// para el resto, `find` acotado a /sdcard.
fn search_android(
    serial: &str,
    query: &str,
    category: Category,
) -> Result<Vec<FileEntry>, String> {
    let needle = query.to_lowercase();

    if matches!(category, Category::Fotos | Category::Video | Category::Audio) {
        let kind = match category {
            Category::Fotos => MediaKind::Images,
            Category::Video => MediaKind::Video,
            _ => MediaKind::Audio,
        };
        let mut all = media_query(serial, kind)?;
        if !needle.is_empty() {
            all.retain(|e| e.name.to_lowercase().contains(&needle));
        }
        all.truncate(MAX_RESULTS);
        return Ok(all);
    }

    let q = sanitize(query);
    let ext_expr = match category.extensions() {
        [] => String::new(),
        exts => {
            let parts: Vec<String> = exts.iter().map(|e| format!("-iname \"*.{e}\"")).collect();
            format!(" \\( {} \\)", parts.join(" -o "))
        }
    };

    let cmd = format!(
        "find /sdcard -type f{} -iname \"*{}*\" 2>/dev/null | head -n {}",
        ext_expr, q, MAX_RESULTS
    );

    let out = adb::shell(serial, &cmd).ok_or("La busqueda falló")?;
    Ok(paths_to_entries(out.lines()))
}

/// Consulta el indice multimedia de Android. Mucho mas rapido que `find`.
fn media_query(serial: &str, kind: MediaKind) -> Result<Vec<FileEntry>, String> {
    let cmd = format!(
        "content query --uri {} --projection _data --sort \"date_modified DESC\" 2>/dev/null",
        kind.uri()
    );
    let out = adb::shell(serial, &cmd).ok_or("No se pudo leer el indice multimedia")?;

    let paths = out.lines().filter_map(|line| {
        // Row: 0 _data=/storage/emulated/0/DCIM/Camera/IMG_0001.jpg
        line.split("_data=").nth(1).map(|p| p.trim())
    });

    let mut entries = paths_to_entries(paths);
    entries.truncate(MAX_RESULTS);
    Ok(entries)
}

fn paths_to_entries<'a, I: Iterator<Item = &'a str>>(paths: I) -> Vec<FileEntry> {
    paths
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|path| {
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            FileEntry {
                category: category_of(&name),
                name,
                path: path.to_string(),
                is_dir: false,
                size: 0, // se omite a proposito: pedir stat por archivo mata el rendimiento
            }
        })
        .collect()
}

fn parse_ls_line(line: &str, base: &str) -> Option<FileEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 8 {
        return None;
    }

    let perms = parts[0];
    if !(perms.starts_with('-') || perms.starts_with('d') || perms.starts_with('l')) {
        return None;
    }

    let is_link = perms.starts_with('l');
    let mut is_dir = perms.starts_with('d');
    let size = parts[4].parse::<u64>().unwrap_or(0);

    let raw = parts[7..].join(" ");
    let name = raw.split(" -> ").next().unwrap_or(&raw);
    let name = name.rsplit('/').next().unwrap_or(name).to_string();

    if name.is_empty() || name == "." || name == ".." || name.starts_with('.') {
        return None;
    }
    if is_link && !name.contains('.') {
        is_dir = true;
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

fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}