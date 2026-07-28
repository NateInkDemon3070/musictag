use std::fs;
use std::path::{Path, PathBuf};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lofty::{
    config::{ParseOptions, WriteOptions},
    file::{BoundTaggedFile, TaggedFileExt},
    picture::{Picture, PictureType},
    read_from_path,
    tag::ItemKey,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize, StatefulImage};

const FIELD_KEYS: &[&str] = &[
    "title", "artist", "album", "albumartist", "year", "track", "disc", "genre", "comment",
];

const SUPPORTED_EXT: &[&str] = &["mp3", "flac", "ogg", "opus", "m4a", "aac", "wma", "wav", "ape"];

#[derive(Clone)]
#[allow(dead_code)]
struct ColorScheme {
    background: Color,
    foreground: Color,
    header_bg: Color,
    header_fg: Color,
    dir_path: Color,
    help_text: Color,
    folder: Color,
    selected: Color,
    normal_file: Color,
    highlight_bg: Color,
    highlight_fg: Color,
    info: Color,
    error: Color,
    success: Color,
    edit_batch: Color,
    edit_individual: Color,
    active_label_bg: Color,
    active_label_fg: Color,
    inactive_label: Color,
    active_value_bg: Color,
    active_value_fg: Color,
    empty_value: Color,
    filled_value: Color,
    preview_border: Color,
    filter_border: Color,
    delete_border: Color,
    cover_border: Color,
    input_border: Color,
    metadata_label: Color,
    metadata_value: Color,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

impl ColorScheme {
    fn catppuccin_mocha() -> Self {
        Self {
            background: Color::Rgb(0x1e, 0x1e, 0x2e),
            foreground: Color::Rgb(0xcd, 0xd6, 0xf4),
            header_bg: Color::Rgb(0x31, 0x32, 0x44),
            header_fg: Color::Rgb(0xcd, 0xd6, 0xf4),
            dir_path: Color::Rgb(0xcd, 0xd6, 0xf4),
            help_text: Color::Rgb(0xcd, 0xd6, 0xf4),
            folder: Color::Rgb(0x89, 0xdc, 0xeb),
            selected: Color::Rgb(0xcb, 0xa6, 0xf7),
            normal_file: Color::Rgb(0xcd, 0xd6, 0xf4),
            highlight_bg: Color::Rgb(0x89, 0xb4, 0xfa),
            highlight_fg: Color::Rgb(0x1e, 0x1e, 0x2e),
            info: Color::Rgb(0xcd, 0xd6, 0xf4),
            error: Color::Rgb(0xf3, 0x8b, 0xa8),
            success: Color::Rgb(0xa6, 0xe3, 0xa1),
            edit_batch: Color::Rgb(0xcb, 0xa6, 0xf7),
            edit_individual: Color::Rgb(0x89, 0xb4, 0xfa),
            active_label_bg: Color::Rgb(0xf9, 0xe2, 0xaf),
            active_label_fg: Color::Rgb(0x1e, 0x1e, 0x2e),
            inactive_label: Color::Rgb(0xcd, 0xd6, 0xf4),
            active_value_bg: Color::Rgb(0xcd, 0xd6, 0xf4),
            active_value_fg: Color::Rgb(0x1e, 0x1e, 0x2e),
            empty_value: Color::Rgb(0x6c, 0x70, 0x86),
            filled_value: Color::Rgb(0xcd, 0xd6, 0xf4),
            preview_border: Color::Rgb(0x45, 0x47, 0x5a),
            filter_border: Color::Rgb(0xf9, 0xe2, 0xaf),
            delete_border: Color::Rgb(0xf3, 0x8b, 0xa8),
            cover_border: Color::Rgb(0x89, 0xb4, 0xfa),
            input_border: Color::Rgb(0xcd, 0xd6, 0xf4),
            metadata_label: Color::Rgb(0xcb, 0xa6, 0xf7),
            metadata_value: Color::Rgb(0xcd, 0xd6, 0xf4),
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("musictag")
        .join("config")
}

fn load_config() -> (Option<PathBuf>, bool) {
    let path = config_path();
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return (None, true),
    };

    let mut dir: Option<PathBuf> = None;
    let mut show_preview = true;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "default_dir" => {
                    let p = PathBuf::from(value.trim());
                    if p.exists() && p.is_dir() {
                        dir = Some(p);
                    }
                }
                "show_preview" => {
                    show_preview = value.trim() != "false";
                }
                _ => {}
            }
        } else if dir.is_none() {
            let p = PathBuf::from(line);
            if p.exists() && p.is_dir() {
                dir = Some(p);
            }
        }
    }

    (dir, show_preview)
}

fn save_config(dir: &Path, show_preview: bool) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create config dir: {}", e))?;
    }
    let content = format!(
        "default_dir={}\nshow_preview={}\n",
        dir.display(),
        show_preview
    );
    fs::write(&path, content.as_bytes())
        .map_err(|e| format!("Could not save config: {}", e))?;
    Ok(())
}

mod icon {
    pub const FOLDER: &str = "\u{f07b}";
    pub const CHECK: &str = "\u{f00c}";
    pub const CLOSE: &str = "\u{f00d}";
    pub const EDIT: &str = "\u{f303}";
    pub const SEARCH: &str = "\u{f002}";
    pub const ARROW_DOWN: &str = "\u{f078}";
    pub const ARROW_UP: &str = "\u{f077}";
    pub const DISK: &str = "\u{f7c2}";
    pub const FILTER: &str = "\u{f0b0}";
    pub const MUSIC_FILE: &str = "\u{f1c6}";
    pub const TAG: &str = "\u{f02c}";
    pub const SELECT: &str = "\u{f14a}";
    pub const TRASH: &str = "\u{f2ed}";
    pub const ARROW_LEFT: &str = "\u{f053}";
    pub const ARROW_RIGHT: &str = "\u{f054}";
    pub const WARNING: &str = "\u{f071}";
    pub const BULK: &str = "\u{f0c3}";
    pub const GLOBE: &str = "\u{f0ac}";
    pub const IMAGE: &str = "\u{f03e}";
}

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Es,
    En,
}

struct L {
    lang: Lang,
}

#[allow(dead_code)]
impl L {
    fn detect() -> Self {
        let locale = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .or_else(|_| std::env::var("LC_MESSAGES"))
            .unwrap_or_default()
            .to_lowercase();

        let lang = if locale.starts_with("es") {
            Lang::Es
        } else {
            Lang::En
        };

        Self { lang }
    }

    fn field_title(&self) -> &str {
        match self.lang {
            Lang::Es => "Titulo",
            Lang::En => "Title",
        }
    }

    fn field_artist(&self) -> &str {
        match self.lang {
            Lang::Es => "Artista",
            Lang::En => "Artist",
        }
    }

    fn field_album(&self) -> &str {
        "Album"
    }

    fn field_albumartist(&self) -> &str {
        match self.lang {
            Lang::Es => "Artista del Album",
            Lang::En => "Album Artist",
        }
    }

    fn field_year(&self) -> &str {
        match self.lang {
            Lang::Es => "Año",
            Lang::En => "Year",
        }
    }

    fn field_track(&self) -> &str {
        match self.lang {
            Lang::Es => "Pista",
            Lang::En => "Track",
        }
    }

    fn field_disc(&self) -> &str {
        match self.lang {
            Lang::Es => "Disco",
            Lang::En => "Disc",
        }
    }

    fn field_genre(&self) -> &str {
        match self.lang {
            Lang::Es => "Genero",
            Lang::En => "Genre",
        }
    }

    fn field_comment(&self) -> &str {
        match self.lang {
            Lang::Es => "Comentario",
            Lang::En => "Comment",
        }
    }

    fn fields(&self) -> Vec<(&str, &str)> {
        vec![
            ("title", self.field_title()),
            ("artist", self.field_artist()),
            ("album", self.field_album()),
            ("albumartist", self.field_albumartist()),
            ("year", self.field_year()),
            ("track", self.field_track()),
            ("disc", self.field_disc()),
            ("genre", self.field_genre()),
            ("comment", self.field_comment()),
        ]
    }

    fn header_title(&self) -> String {
        match self.lang {
            Lang::Es => format!(" {} musictag - Editor de Metadatos de Musica ", icon::TAG),
            Lang::En => format!(" {} musictag - Music Metadata Editor ", icon::TAG),
        }
    }

    fn help_browse(&self) -> String {
        match self.lang {
            Lang::Es => format!(
                "{}{} mover  {} abrir  {} seleccionar  {} seleccionar todo  {} editar seleccion  {} eliminar  c:portada  P:preview  R:colores  C:predeterminada  {} salir",
                icon::ARROW_UP, icon::ARROW_DOWN, icon::ARROW_RIGHT, icon::SELECT, icon::BULK, icon::EDIT, icon::TRASH, icon::CLOSE,
            ),
            Lang::En => format!(
                "{}{} move  {} open  {} select  {} select all  {} edit selected  {} delete  c:cover  P:preview  R:colors  C:default  {} quit",
                icon::ARROW_UP, icon::ARROW_DOWN, icon::ARROW_RIGHT, icon::SELECT, icon::BULK, icon::EDIT, icon::TRASH, icon::CLOSE,
            ),
        }
    }

    fn help_edit(&self) -> String {
        match self.lang {
            Lang::Es => format!(
                "{}{} campo  {}{} cursor  Enter:guardar  Esc:cancelar  Ctrl+S:guardar y avanzar",
                icon::ARROW_UP, icon::ARROW_DOWN, icon::ARROW_LEFT, icon::ARROW_RIGHT,
            ),
            Lang::En => format!(
                "{}{} field  {}{} cursor  Enter:save  Esc:cancel  Ctrl+S:save & next",
                icon::ARROW_UP, icon::ARROW_DOWN, icon::ARROW_LEFT, icon::ARROW_RIGHT,
            ),
        }
    }

    fn info_files(&self, idx: usize, total: usize, audio_count: usize) -> String {
        match self.lang {
            Lang::Es => format!(" {}/{} | {} archivos de audio", idx, total, audio_count),
            Lang::En => format!(" {}/{} | {} audio files", idx, total, audio_count),
        }
    }

    fn info_filter(&self, text: &str) -> String {
        match self.lang {
            Lang::Es => format!(" | {} Filtro: {}", icon::FILTER, text),
            Lang::En => format!(" | {} Filter: {}", icon::FILTER, text),
        }
    }

    fn info_selected(&self, count: usize) -> String {
        match self.lang {
            Lang::Es => format!(" | {} {} seleccionados", icon::SELECT, count),
            Lang::En => format!(" | {} {} selected", icon::SELECT, count),
        }
    }

    fn edit_single(&self, fname: &str) -> String {
        match self.lang {
            Lang::Es => format!(" {} Editando: {}", icon::EDIT, fname),
            Lang::En => format!(" {} Editing: {}", icon::EDIT, fname),
        }
    }

    fn edit_batch(&self, count: usize) -> String {
        match self.lang {
            Lang::Es => format!(" {} Edicion masiva: {} archivos", icon::BULK, count),
            Lang::En => format!(" {} Batch edit: {} files", icon::BULK, count),
        }
    }

    fn status_reloaded(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Recargado", icon::CHECK),
            Lang::En => format!("{} Reloaded", icon::CHECK),
        }
    }

    fn status_selected_all(&self, count: usize) -> String {
        match self.lang {
            Lang::Es => format!("{} Seleccionados {} archivos", icon::SELECT, count),
            Lang::En => format!("{} Selected {} files", icon::SELECT, count),
        }
    }

    fn status_selection_cleared(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Seleccion limpiada", icon::TRASH),
            Lang::En => format!("{} Selection cleared", icon::TRASH),
        }
    }

    fn err_could_not_read(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} No se pudo leer metadatos", icon::CLOSE),
            Lang::En => format!("{} Could not read metadata", icon::CLOSE),
        }
    }

    fn err_could_not_read_file(&self, name: &str) -> String {
        match self.lang {
            Lang::Es => format!("{} No se pudo leer metadatos de {}", icon::CLOSE, name),
            Lang::En => format!("{} Could not read metadata from {}", icon::CLOSE, name),
        }
    }

    fn saved(&self, name: &str) -> String {
        match self.lang {
            Lang::Es => format!("{} Guardado: {}", icon::CHECK, name),
            Lang::En => format!("{} Saved: {}", icon::CHECK, name),
        }
    }

    fn save_error(&self, e: &str) -> String {
        match self.lang {
            Lang::Es => format!("{} Error al guardar: {}", icon::CLOSE, e),
            Lang::En => format!("{} Save error: {}", icon::CLOSE, e),
        }
    }

    fn saved_batch(&self, count: u32, total: usize) -> String {
        match self.lang {
            Lang::Es => format!("{} Guardado en {}/{} archivos", icon::CHECK, count, total),
            Lang::En => format!("{} Saved to {}/{} files", icon::CHECK, count, total),
        }
    }

    fn deleted(&self, count: u32) -> String {
        match self.lang {
            Lang::Es => format!("{} Eliminado(s) {} archivo(s)", icon::TRASH, count),
            Lang::En => format!("{} Deleted {} file(s)", icon::TRASH, count),
        }
    }

    fn cancelled(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Cancelado", icon::CHECK),
            Lang::En => format!("{} Cancelled", icon::CHECK),
        }
    }

    fn no_selected(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} No hay archivos seleccionados", icon::CLOSE),
            Lang::En => format!("{} No files selected", icon::CLOSE),
        }
    }

    fn confirm_delete_title(&self) -> String {
        match self.lang {
            Lang::Es => format!(" {} Confirmar eliminacion ", icon::TRASH),
            Lang::En => format!(" {} Confirm delete ", icon::TRASH),
        }
    }

    fn confirm_delete_one(&self, name: &str) -> String {
        match self.lang {
            Lang::Es => format!("{} Eliminar '{}'?", icon::WARNING, name),
            Lang::En => format!("{} Delete '{}'?", icon::WARNING, name),
        }
    }

    fn confirm_delete_many(&self, count: usize) -> String {
        match self.lang {
            Lang::Es => format!("{} Eliminar {} archivos?", icon::WARNING, count),
            Lang::En => format!("{} Delete {} files?", icon::WARNING, count),
        }
    }

    fn confirm_hint(&self) -> &str {
        match self.lang {
            Lang::Es => "  y: si   n: no  ",
            Lang::En => "  y: yes  n: no  ",
        }
    }

    fn filter_title(&self) -> String {
        match self.lang {
            Lang::Es => format!(" {} Filtrar archivos ", icon::SEARCH),
            Lang::En => format!(" {} Filter files ", icon::SEARCH),
        }
    }

    fn val_empty(&self) -> &str {
        match self.lang {
            Lang::Es => "(vacio)",
            Lang::En => "(empty)",
        }
    }

    fn status_field(&self, idx: usize, total: usize, cursor: usize) -> String {
        format!(" {}/{}  {}  cursor: {}", idx, total, icon::DISK, cursor)
    }

    fn lang_name(&self) -> &str {
        match self.lang {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    fn lang_detected(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Idioma detectado: Espanol", icon::GLOBE),
            Lang::En => format!("{} Language detected: English", icon::GLOBE),
        }
    }

    fn err_invalid_dir(&self, path: &Path) -> String {
        match self.lang {
            Lang::Es => format!("Error: '{}' no es un directorio valido", path.display()),
            Lang::En => format!("Error: '{}' is not a valid directory", path.display()),
        }
    }

    fn config_saved(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Carpeta predeterminada guardada", icon::CHECK),
            Lang::En => format!("{} Default folder saved", icon::CHECK),
        }
    }

    fn config_error(&self, e: &str) -> String {
        match self.lang {
            Lang::Es => format!("{} Error al guardar configuracion: {}", icon::CLOSE, e),
            Lang::En => format!("{} Config save error: {}", icon::CLOSE, e),
        }
    }

    fn config_current(&self, path: &Path) -> String {
        match self.lang {
            Lang::Es => format!(
                "{} Carpeta actual: {} (C para cambiar)",
                icon::FOLDER,
                path.display()
            ),
            Lang::En => format!(
                "{} Current folder: {} (C to change)",
                icon::FOLDER,
                path.display()
            ),
        }
    }

    fn cover_title(&self) -> String {
        match self.lang {
            Lang::Es => format!(" {} Asignar portada ", icon::IMAGE),
            Lang::En => format!(" {} Set cover art ", icon::IMAGE),
        }
    }

    fn cover_hint(&self) -> &str {
        match self.lang {
            Lang::Es => "  Enter:aplicar  Esc:cancelar  ",
            Lang::En => "  Enter:apply   Esc:cancel   ",
        }
    }

    fn preview_enabled(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Preview de imagenes activado", icon::CHECK),
            Lang::En => format!("{} Image preview enabled", icon::CHECK),
        }
    }

    fn preview_disabled(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Preview de imagenes desactivado", icon::CHECK),
            Lang::En => format!("{} Image preview disabled", icon::CHECK),
        }
    }

    fn reload_colors(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Colores recargados", icon::CHECK),
            Lang::En => format!("{} Colors reloaded", icon::CHECK),
        }
    }
}

fn field_key(key: &str) -> ItemKey {
    match key {
        "title" => ItemKey::TrackTitle,
        "artist" => ItemKey::TrackArtist,
        "album" => ItemKey::AlbumTitle,
        "albumartist" => ItemKey::AlbumArtist,
        "year" => ItemKey::RecordingDate,
        "track" => ItemKey::TrackNumber,
        "disc" => ItemKey::DiscNumber,
        "genre" => ItemKey::Genre,
        "comment" => ItemKey::Comment,
        _ => ItemKey::Unknown(String::new()),
    }
}

#[derive(Clone, PartialEq)]
enum AppMode {
    Browse,
    Edit,
    Filter,
    DeleteConfirm,
    SetCoverArt,
}

struct App {
    current_dir: PathBuf,
    dir_entries: Vec<PathBuf>,
    files: Vec<PathBuf>,
    current_idx: usize,
    scroll_offset: usize,
    mode: AppMode,
    edit_idx: usize,
    edit_vals: Vec<String>,
    edit_cursor: usize,
    batch_mode: bool,
    batch_indices: Vec<usize>,
    selected: std::collections::HashSet<usize>,
    filter_text: String,
    filter_cursor: usize,
    delete_indices: Vec<usize>,
    cover_path: String,
    cover_cursor: usize,
    show_preview: bool,
    status_msg: String,
    status_is_error: bool,
    should_quit: bool,
    l: L,
    colors: ColorScheme,
    picker: Option<Picker>,
    cover_protocol: Option<StatefulProtocol>,
}

impl App {
    fn new(start_dir: PathBuf, l: L, show_preview: bool) -> Self {
        let mut app = App {
            current_dir: start_dir,
            dir_entries: Vec::new(),
            files: Vec::new(),
            current_idx: 0,
            scroll_offset: 0,
            mode: AppMode::Browse,
            edit_idx: 0,
            edit_vals: Vec::new(),
            edit_cursor: 0,
            batch_mode: false,
            batch_indices: Vec::new(),
            selected: std::collections::HashSet::new(),
            filter_text: String::new(),
            filter_cursor: 0,
            delete_indices: Vec::new(),
            cover_path: String::new(),
            cover_cursor: 0,
            show_preview,
            status_msg: String::new(),
            status_is_error: false,
            should_quit: false,
            l,
            colors: ColorScheme::default(),
            picker: None,
            cover_protocol: None,
        };
        app.load_dir();
        app
    }

    fn load_dir(&mut self) {
        self.dir_entries.clear();
        self.files.clear();

        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            let mut all: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    !p.file_name()
                        .and_then(|n| n.to_str())
                        .map_or(false, |n| n.starts_with('.'))
                })
                .collect();

            all.sort_by(|a, b| {
                let a_dir = a.is_dir();
                let b_dir = b.is_dir();
                if a_dir != b_dir {
                    return b_dir.cmp(&a_dir);
                }
                a.file_name().cmp(&b.file_name())
            });

            for entry in all {
                if entry.is_dir() {
                    self.dir_entries.push(entry);
                } else if let Some(ext) = entry.extension().and_then(|e| e.to_str()) {
                    if SUPPORTED_EXT.contains(&ext.to_lowercase().as_str()) {
                        if self.filter_text.is_empty()
                            || entry
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map_or(false, |n| {
                                    n.to_lowercase().contains(&self.filter_text.to_lowercase())
                                })
                        {
                            self.files.push(entry);
                        }
                    }
                }
            }
        }

        let total = self.total_items();
        if total > 0 && self.current_idx >= total {
            self.current_idx = total - 1;
        }
        self.update_cover_state();
    }

    fn update_cover_state(&mut self) {
        self.cover_protocol = None;
        let picker = match &self.picker {
            Some(p) => p,
            None => return,
        };
        if self.current_idx < self.dir_entries.len() {
            return;
        }
        let fidx = self.current_idx - self.dir_entries.len();
        if fidx >= self.files.len() {
            return;
        }
        let filepath = &self.files[fidx];
        let tagged_file = match read_from_path(filepath) {
            Ok(f) => f,
            Err(_) => return,
        };
        let tag = match tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
            Some(t) => t,
            None => return,
        };
        let picture = match tag.pictures().first() {
            Some(p) => p,
            None => return,
        };
        let data = picture.data().to_vec();
        if data.is_empty() {
            return;
        }
        let img = match image::load_from_memory(&data) {
            Ok(i) => i,
            Err(_) => return,
        };
        self.cover_protocol = Some(picker.new_resize_protocol(img));
    }

    fn total_items(&self) -> usize {
        self.dir_entries.len() + self.files.len()
    }

    fn get_item(&self, idx: usize) -> Option<(&PathBuf, bool)> {
        if idx < self.dir_entries.len() {
            Some((&self.dir_entries[idx], true))
        } else {
            let fidx = idx - self.dir_entries.len();
            if fidx < self.files.len() {
                Some((&self.files[fidx], false))
            } else {
                None
            }
        }
    }

    fn get_fields(&self) -> Vec<(&str, &str)> {
        self.l.fields()
    }

    fn enter_edit_mode(&mut self, batch: bool) {
        if batch && !self.selected.is_empty() {
            self.batch_mode = true;
            self.batch_indices = self.selected.iter().copied().collect();
            self.batch_indices.sort();

            let first = self.batch_indices[0];
            let fidx = first - self.dir_entries.len();
            if fidx >= self.files.len() {
                return;
            }

            let filepath = &self.files[fidx];
            match read_metadata(filepath) {
                Some(vals) => {
                    self.edit_vals = vals;
                    self.edit_idx = 0;
                    self.edit_cursor = self.edit_vals[0].len();
                    self.mode = AppMode::Edit;
                    self.status_msg.clear();
                }
                None => {
                    self.status_msg = self.l.err_could_not_read();
                    self.status_is_error = true;
                }
            }
        } else {
            self.batch_mode = false;
            self.batch_indices.clear();

            if self.current_idx < self.dir_entries.len() {
                return;
            }
            let fidx = self.current_idx - self.dir_entries.len();
            if fidx >= self.files.len() {
                return;
            }

            let filepath = &self.files[fidx];
            match read_metadata(filepath) {
                Some(vals) => {
                    self.edit_vals = vals;
                    self.edit_idx = 0;
                    self.edit_cursor = self.edit_vals[0].len();
                    self.mode = AppMode::Edit;
                    self.status_msg.clear();
                }
                None => {
                    let name = filepath.file_name().unwrap_or_default().to_string_lossy();
                    self.status_msg = self.l.err_could_not_read_file(&name);
                    self.status_is_error = true;
                }
            }
        }
    }

    fn save_current(&mut self) -> bool {
        if self.batch_mode {
            return self.save_batch();
        }

        let fidx = self.current_idx - self.dir_entries.len();
        if fidx >= self.files.len() {
            return false;
        }
        let filepath = &self.files[fidx];
        match write_metadata(filepath, &self.edit_vals) {
            Ok(()) => {
                let name = filepath.file_name().unwrap_or_default().to_string_lossy();
                self.status_msg = self.l.saved(&name);
                self.status_is_error = false;
                true
            }
            Err(e) => {
                self.status_msg = self.l.save_error(&e);
                self.status_is_error = true;
                false
            }
        }
    }

    fn save_batch(&mut self) -> bool {
        let mut count = 0u32;
        let total = self.batch_indices.len();

        for &idx in &self.batch_indices {
            let fidx = idx - self.dir_entries.len();
            if fidx >= self.files.len() {
                continue;
            }
            let filepath = &self.files[fidx];
            if write_metadata(filepath, &self.edit_vals).is_ok() {
                count += 1;
            }
        }

        self.status_msg = self.l.saved_batch(count, total);
        self.status_is_error = false;
        count > 0
    }

    fn save_and_next(&mut self) {
        if self.save_current() {
            if self.batch_mode {
                self.batch_mode = false;
                self.batch_indices.clear();
                self.selected.clear();
                self.mode = AppMode::Browse;
                return;
            }
            let total = self.total_items();
            if self.current_idx < total - 1 {
                self.current_idx += 1;
                self.enter_edit_mode(false);
            }
        }
    }

    fn open_item(&mut self) {
        if let Some((item, is_dir)) = self.get_item(self.current_idx) {
            if is_dir {
                self.current_dir = item.clone();
                self.current_idx = 0;
                self.scroll_offset = 0;
                self.selected.clear();
                self.load_dir();
            } else {
                self.enter_edit_mode(false);
            }
        }
    }

    fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            if parent != self.current_dir {
                self.current_dir = parent.to_path_buf();
                self.current_idx = 0;
                self.scroll_offset = 0;
                self.selected.clear();
                self.load_dir();
            }
        }
    }

    fn toggle_select(&mut self) {
        if self.current_idx < self.dir_entries.len() {
            return;
        }
        if self.selected.contains(&self.current_idx) {
            self.selected.remove(&self.current_idx);
        } else {
            self.selected.insert(self.current_idx);
        }
    }

    fn select_all(&mut self) {
        let total = self.total_items();
        for idx in self.dir_entries.len()..total {
            self.selected.insert(idx);
        }
    }

    fn confirm_delete(&mut self) {
        let mut to_delete: Vec<usize> = if self.selected.is_empty() {
            vec![self.current_idx]
        } else {
            self.selected.iter().copied().collect()
        };
        to_delete.sort();

        let mut count = 0u32;
        for &idx in &to_delete {
            let fidx = idx - self.dir_entries.len();
            if fidx >= self.files.len() {
                continue;
            }
            let filepath = &self.files[fidx];
            if fs::remove_file(filepath).is_ok() {
                count += 1;
            }
        }

        if !self.selected.is_empty() {
            self.selected.clear();
        }

        self.status_msg = self.l.deleted(count);
        self.status_is_error = false;
        self.load_dir();
    }

    fn enter_delete_confirm(&mut self) {
        if self.selected.is_empty() && self.current_idx >= self.dir_entries.len() {
            self.delete_indices = vec![self.current_idx];
        } else if !self.selected.is_empty() {
            self.delete_indices = self.selected.iter().copied().collect();
            self.delete_indices.sort();
        } else {
            return;
        }
        self.mode = AppMode::DeleteConfirm;
    }

    fn enter_set_cover_art(&mut self) {
        if self.current_idx < self.dir_entries.len() {
            return;
        }
        self.cover_path.clear();
        self.cover_cursor = 0;
        self.mode = AppMode::SetCoverArt;
    }

    fn apply_cover_art(&mut self) -> bool {
        if self.current_idx < self.dir_entries.len() {
            return false;
        }
        let fidx = self.current_idx - self.dir_entries.len();
        if fidx >= self.files.len() {
            return false;
        }

        let filepath = &self.files[fidx];
        let img_path = Path::new(&self.cover_path);

        if !img_path.exists() {
            self.status_msg = match self.l.lang {
                Lang::Es => format!("{} La imagen no existe", icon::CLOSE),
                Lang::En => format!("{} Image does not exist", icon::CLOSE),
            };
            self.status_is_error = true;
            return false;
        }

        match set_cover_art(filepath, img_path) {
            Ok(()) => {
                let name = filepath.file_name().unwrap_or_default().to_string_lossy();
                self.status_msg = match self.l.lang {
                    Lang::Es => format!("{} Portada asignada: {}", icon::CHECK, name),
                    Lang::En => format!("{} Cover art set: {}", icon::CHECK, name),
                };
                self.status_is_error = false;
                true
            }
            Err(e) => {
                self.status_msg = match self.l.lang {
                    Lang::Es => format!("{} Error al asignar portada: {}", icon::CLOSE, e),
                    Lang::En => format!("{} Cover art error: {}", icon::CLOSE, e),
                };
                self.status_is_error = true;
                false
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            AppMode::Browse => self.handle_browse_key(key),
            AppMode::Edit => self.handle_edit_key(key),
            AppMode::Filter => self.handle_filter_key(key),
            AppMode::DeleteConfirm => self.handle_delete_confirm_key(key),
            AppMode::SetCoverArt => self.handle_cover_art_key(key),
        }
    }

    fn handle_browse_key(&mut self, key: KeyEvent) {
        let total = self.total_items();

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.current_idx < total.saturating_sub(1) {
                    self.current_idx += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.current_idx > 0 {
                    self.current_idx -= 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.go_parent();
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                self.open_item();
            }
            KeyCode::Esc => {
                self.go_parent();
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.current_idx = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.current_idx = total.saturating_sub(1);
            }
            KeyCode::Char('a') | KeyCode::Char(' ') => {
                self.toggle_select();
            }
            KeyCode::Char('f') | KeyCode::Char('/') => {
                self.filter_text.clear();
                self.filter_cursor = 0;
                self.mode = AppMode::Filter;
            }
            KeyCode::Char('r') => {
                self.load_dir();
                self.status_msg = self.l.status_reloaded();
                self.status_is_error = false;
            }
            KeyCode::Char('A') => {
                self.enter_edit_mode(true);
            }
            KeyCode::Char('V') | KeyCode::Char('v') => {
                self.select_all();
                let count = self.selected.len();
                self.status_msg = self.l.status_selected_all(count);
                self.status_is_error = false;
            }
            KeyCode::Char('d') => {
                self.selected.clear();
                self.status_msg = self.l.status_selection_cleared();
                self.status_is_error = false;
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                self.enter_delete_confirm();
            }
            KeyCode::Char('C') => {
                match save_config(&self.current_dir, self.show_preview) {
                    Ok(()) => {
                        self.status_msg = self.l.config_saved();
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_msg = self.l.config_error(&e);
                        self.status_is_error = true;
                    }
                }
            }
            KeyCode::Char('c') => {
                self.enter_set_cover_art();
            }
            KeyCode::Char('R') => {
                self.colors = ColorScheme::default();
                self.status_msg = self.l.reload_colors();
                self.status_is_error = false;
            }
            KeyCode::Char('P') => {
                self.show_preview = !self.show_preview;
                self.status_msg = if self.show_preview {
                    self.l.preview_enabled()
                } else {
                    self.l.preview_disabled()
                };
                self.status_is_error = false;
            }
            _ => {}
        }
        self.update_cover_state();
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        let field_count = self.get_fields().len();

        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Browse;
                self.batch_mode = false;
                self.batch_indices.clear();
                self.status_msg.clear();
            }
            KeyCode::Enter => {
                self.save_current();
                if self.batch_mode {
                    self.batch_mode = false;
                    self.batch_indices.clear();
                    self.selected.clear();
                }
                self.mode = AppMode::Browse;
            }
            KeyCode::Tab => {
                self.edit_idx = (self.edit_idx + 1) % field_count;
                self.edit_cursor = self.edit_vals[self.edit_idx].len();
            }
            KeyCode::BackTab => {
                if self.edit_idx == 0 {
                    self.edit_idx = field_count - 1;
                } else {
                    self.edit_idx -= 1;
                }
                self.edit_cursor = self.edit_vals[self.edit_idx].len();
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_and_next();
            }
            KeyCode::Backspace => {
                if self.edit_cursor > 0 {
                    self.edit_cursor -= 1;
                    self.edit_vals[self.edit_idx].remove(self.edit_cursor);
                }
            }
            KeyCode::Delete => {
                if self.edit_cursor < self.edit_vals[self.edit_idx].len() {
                    self.edit_vals[self.edit_idx].remove(self.edit_cursor);
                }
            }
            KeyCode::Left => {
                if self.edit_cursor > 0 {
                    self.edit_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.edit_cursor < self.edit_vals[self.edit_idx].len() {
                    self.edit_cursor += 1;
                }
            }
            KeyCode::Up => {
                if self.edit_idx > 0 {
                    self.edit_idx -= 1;
                    self.edit_cursor = self.edit_vals[self.edit_idx].len();
                }
            }
            KeyCode::Down => {
                if self.edit_idx < field_count - 1 {
                    self.edit_idx += 1;
                    self.edit_cursor = self.edit_vals[self.edit_idx].len();
                }
            }
            KeyCode::Home => {
                self.edit_cursor = 0;
            }
            KeyCode::End => {
                self.edit_cursor = self.edit_vals[self.edit_idx].len();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_vals[self.edit_idx].clear();
                self.edit_cursor = 0;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enter_edit_mode(true);
            }
            KeyCode::Char(c) => {
                self.edit_vals[self.edit_idx].insert(self.edit_cursor, c);
                self.edit_cursor += 1;
            }
            _ => {}
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Browse;
                self.filter_text.clear();
                self.load_dir();
            }
            KeyCode::Enter => {
                self.mode = AppMode::Browse;
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter_text.clear();
                self.filter_cursor = 0;
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            KeyCode::Backspace => {
                if self.filter_cursor > 0 {
                    self.filter_cursor -= 1;
                    self.filter_text.remove(self.filter_cursor);
                }
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            KeyCode::Left => {
                if self.filter_cursor > 0 {
                    self.filter_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.filter_cursor < self.filter_text.len() {
                    self.filter_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.filter_cursor = 0;
            }
            KeyCode::End => {
                self.filter_cursor = self.filter_text.len();
            }
            KeyCode::Char(c) => {
                self.filter_text.insert(self.filter_cursor, c);
                self.filter_cursor += 1;
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            _ => {}
        }
    }

    fn handle_delete_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.confirm_delete();
                self.mode = AppMode::Browse;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = AppMode::Browse;
                self.status_msg = self.l.cancelled();
                self.status_is_error = false;
            }
            _ => {}
        }
    }

    fn handle_cover_art_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Browse;
                self.status_msg = self.l.cancelled();
                self.status_is_error = false;
            }
            KeyCode::Enter => {
                if self.cover_path.is_empty() {
                    return;
                }
                if self.apply_cover_art() {
                    self.mode = AppMode::Browse;
                    self.update_cover_state();
                }
            }
            KeyCode::Char(c) => {
                self.cover_path.insert(self.cover_cursor, c);
                self.cover_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cover_cursor > 0 {
                    self.cover_cursor -= 1;
                    self.cover_path.remove(self.cover_cursor);
                }
            }
            KeyCode::Delete => {
                if self.cover_cursor < self.cover_path.len() {
                    self.cover_path.remove(self.cover_cursor);
                }
            }
            KeyCode::Left => {
                if self.cover_cursor > 0 {
                    self.cover_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cover_cursor < self.cover_path.len() {
                    self.cover_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.cover_cursor = 0;
            }
            KeyCode::End => {
                self.cover_cursor = self.cover_path.len();
            }
            _ => {}
        }
    }
}

fn read_metadata(filepath: &Path) -> Option<Vec<String>> {
    let tagged_file = read_from_path(filepath).ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    let vals: Vec<String> = FIELD_KEYS
        .iter()
        .map(|key| {
            tag.get_string(&field_key(key))
                .unwrap_or("")
                .to_string()
        })
        .collect();

    Some(vals)
}

fn write_metadata(filepath: &Path, vals: &[String]) -> Result<(), String> {
    use std::fs::OpenOptions;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(filepath)
        .map_err(|e| format!("Could not open: {}", e))?;

    let mut bound = BoundTaggedFile::read_from(file, ParseOptions::new())
        .map_err(|e| format!("Could not read: {}", e))?;

    let tag = if bound.primary_tag_mut().is_some() {
        bound.primary_tag_mut().unwrap()
    } else if bound.first_tag_mut().is_some() {
        bound.first_tag_mut().unwrap()
    } else {
        return Err("No tag found".to_string());
    };

    for (i, key) in FIELD_KEYS.iter().enumerate() {
        let item_key = field_key(key);
        if vals[i].is_empty() {
            tag.remove_key(&item_key);
        } else {
            tag.insert_text(item_key, vals[i].clone());
        }
    }

    bound
        .save(WriteOptions::default())
        .map_err(|e| format!("Could not save: {}", e))?;

    Ok(())
}

fn set_cover_art(filepath: &Path, image_path: &Path) -> Result<(), String> {
    use std::fs::OpenOptions;

    let img_data = std::fs::read(image_path)
        .map_err(|e| format!("Could not read image: {}", e))?;

    let picture = Picture::from_reader(&mut &img_data[..])
        .map_err(|e| format!("Invalid image: {}", e))?;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(filepath)
        .map_err(|e| format!("Could not open: {}", e))?;

    let mut bound = BoundTaggedFile::read_from(file, ParseOptions::new())
        .map_err(|e| format!("Could not read: {}", e))?;

    let tag = if bound.primary_tag_mut().is_some() {
        bound.primary_tag_mut().unwrap()
    } else if bound.first_tag_mut().is_some() {
        bound.first_tag_mut().unwrap()
    } else {
        return Err("No tag found".to_string());
    };

    tag.remove_picture_type(PictureType::CoverFront);
    tag.push_picture(picture);

    bound
        .save(WriteOptions::default())
        .map_err(|e| format!("Could not save: {}", e))?;

    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // help bar
            Constraint::Length(1), // dir path / title
            Constraint::Min(5),   // content area
            Constraint::Length(1), // status
            Constraint::Length(1), // message
        ])
        .split(frame.area());

    let chunks: Vec<Rect> = chunks.iter().copied().collect();

    let header_style = Style::default()
        .fg(app.colors.header_fg)
        .bg(app.colors.header_bg)
        .add_modifier(Modifier::BOLD);
    let header = Paragraph::new(app.l.header_title()).style(header_style);
    frame.render_widget(header, chunks[0]);

    match app.mode {
        AppMode::Browse => render_browse(frame, app, &chunks),
        AppMode::Edit => render_edit(frame, app, &chunks),
        AppMode::Filter => {
            render_browse(frame, app, &chunks);
            render_filter_popup(frame, app);
        }
        AppMode::DeleteConfirm => {
            render_browse(frame, app, &chunks);
            render_delete_confirm(frame, app);
        }
        AppMode::SetCoverArt => {
            render_browse(frame, app, &chunks);
            render_cover_art_popup(frame, app);
        }
    }
}

fn render_browse(frame: &mut Frame, app: &mut App, chunks: &[Rect]) {
    let dir_style = Style::default()
        .fg(app.colors.dir_path)
        .add_modifier(Modifier::BOLD);
    let dir_text = format!(" {} {}", icon::FOLDER, app.current_dir.display());
    let dir_para = Paragraph::new(dir_text).style(dir_style);
    frame.render_widget(dir_para, chunks[2]);

    let help_para = Paragraph::new(app.l.help_browse())
        .style(Style::default().fg(app.colors.help_text))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(help_para, chunks[1]);

    let (list_area, preview_area) = if app.show_preview {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(chunks[3]);
        let cc: Vec<Rect> = content_chunks.iter().copied().collect();
        (cc[0], Some(cc[1]))
    } else {
        (chunks[3], None)
    };

    let total = app.total_items();
    let list_height = list_area.height as usize;

    let mut adjusted_scroll = app.scroll_offset;
    if app.current_idx >= adjusted_scroll + list_height {
        adjusted_scroll = app.current_idx.saturating_sub(list_height - 1);
    }
    if app.current_idx < adjusted_scroll {
        adjusted_scroll = app.current_idx;
    }

    let items: Vec<ListItem> = (0..total)
        .skip(adjusted_scroll)
        .take(list_height)
        .filter_map(|idx| app.get_item(idx).map(|(item, is_dir)| (idx, item, is_dir)))
        .map(|(idx, item, is_dir)| {
            let name = item.file_name().unwrap_or_default().to_string_lossy();
            let is_selected = app.selected.contains(&idx);

            let (icon_str, icon_style) = if is_dir {
                (
                    format!("{} ", icon::FOLDER),
                    Style::default()
                        .fg(app.colors.folder)
                        .add_modifier(Modifier::BOLD),
                )
            } else if is_selected {
                (
                    format!("{} ", icon::CHECK),
                    Style::default().fg(app.colors.selected),
                )
            } else {
                (
                    format!("{} ", icon::MUSIC_FILE),
                    Style::default().fg(app.colors.normal_file),
                )
            };

            let line = Line::from(vec![
                Span::styled(icon_str, icon_style),
                Span::styled(name, Style::default().fg(Color::Rgb(255, 255, 255))),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    let selected_row = app.current_idx.saturating_sub(adjusted_scroll);
    if selected_row < list_height && total > 0 {
        state.select(Some(selected_row));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(
            Style::default()
                .bg(app.colors.highlight_bg)
                .fg(app.colors.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, list_area, &mut state);

    if let Some(area) = preview_area {
        render_preview(frame, app, area);
    }

    let mut info = app.l.info_files(app.current_idx + 1, total, app.files.len());
    if !app.filter_text.is_empty() {
        info.push_str(&app.l.info_filter(&app.filter_text));
    }
    if !app.selected.is_empty() {
        info.push_str(&app.l.info_selected(app.selected.len()));
    }

    let info_style = Style::default()
        .fg(app.colors.info)
        .add_modifier(Modifier::DIM);
    let info_para = Paragraph::new(info)
        .style(info_style)
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(info_para, chunks[4]);

    if !app.status_msg.is_empty() {
        let status_style = if app.status_is_error {
            Style::default()
                .fg(app.colors.error)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.colors.success)
                .add_modifier(Modifier::BOLD)
        };
        let status_para = Paragraph::new(app.status_msg.clone())
            .style(status_style)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(status_para, chunks[5]);
    }
}

fn render_edit(frame: &mut Frame, app: &App, chunks: &[Rect]) {
    let title_text = if app.batch_mode {
        app.l.edit_batch(app.batch_indices.len())
    } else {
        let fname = app
            .files
            .get(app.current_idx.saturating_sub(app.dir_entries.len()))
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        app.l.edit_single(&fname)
    };

    let title_style = Style::default()
        .fg(if app.batch_mode {
            app.colors.edit_batch
        } else {
            app.colors.edit_individual
        })
        .add_modifier(Modifier::BOLD);
    let title = Paragraph::new(title_text).style(title_style);
    frame.render_widget(title, chunks[2]);

    let help_para = Paragraph::new(app.l.help_edit())
        .style(Style::default().fg(app.colors.help_text))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(help_para, chunks[1]);

    let fields = app.get_fields();
    let mut lines: Vec<Line> = Vec::new();

    for (i, (_key, label)) in fields.iter().enumerate() {
        let val = &app.edit_vals[i];
        let is_active = i == app.edit_idx;

        let arrow = if is_active {
            icon::ARROW_RIGHT
        } else {
            " "
        };

        let label_span = Span::styled(
            format!("  {} {:<22}", arrow, label),
            if is_active {
                Style::default()
                    .fg(app.colors.active_label_fg)
                    .bg(app.colors.active_label_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(app.colors.inactive_label)
                    .add_modifier(Modifier::BOLD)
            },
        );

        let val_display = if val.is_empty() {
            app.l.val_empty().to_string()
        } else {
            val.clone()
        };

        let val_style = if is_active {
            Style::default()
                .fg(app.colors.active_value_fg)
                .bg(app.colors.active_value_bg)
                .add_modifier(Modifier::BOLD)
        } else if val.is_empty() {
            Style::default()
                .fg(app.colors.empty_value)
                .add_modifier(Modifier::ITALIC)
        } else {
            Style::default().fg(app.colors.filled_value)
        };

        let val_span = Span::styled(val_display, val_style);

        lines.push(Line::from(vec![label_span, val_span]));
    }

    let form_widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(form_widget, chunks[3]);

    let status_style = Style::default()
        .fg(app.colors.info)
        .add_modifier(Modifier::DIM);
    let status = Paragraph::new(app.l.status_field(
        app.edit_idx + 1,
        fields.len(),
        app.edit_cursor,
    ))
    .style(status_style)
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(status, chunks[4]);

    if !app.status_msg.is_empty() {
        let status_style = if app.status_is_error {
            Style::default()
                .fg(app.colors.error)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.colors.success)
                .add_modifier(Modifier::BOLD)
        };
        let status_para = Paragraph::new(app.status_msg.clone())
            .style(status_style)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(status_para, chunks[5]);
    }
}


fn render_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.colors.preview_border))
        .title(" Preview ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    let fidx = app.current_idx.checked_sub(app.dir_entries.len());

    let mut lines: Vec<Line> = Vec::new();

    if app.current_idx < app.dir_entries.len() {
        let dir_name = app.dir_entries[app.current_idx]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let text = format!("{} {}", icon::FOLDER, dir_name);
        lines.push(Line::from(Span::styled(
            text,
            Style::default().fg(app.colors.folder).add_modifier(Modifier::BOLD),
        )));
    } else if let Some(fidx) = fidx {
        if fidx >= app.files.len() {
            return;
        }
        let filepath = &app.files[fidx];
        let filename = filepath
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let ext = filepath
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_uppercase();

        lines.push(Line::from(Span::styled(
            format!("{} {}", icon::MUSIC_FILE, filename),
            Style::default()
                .fg(app.colors.normal_file)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", ext),
            Style::default().fg(app.colors.empty_value),
        )));

        if let Ok(meta) = std::fs::metadata(filepath) {
            let size = if meta.len() >= 1_048_576 {
                format!("{:.1} MB", meta.len() as f64 / 1_048_576.0)
            } else if meta.len() >= 1024 {
                format!("{:.1} KB", meta.len() as f64 / 1024.0)
            } else {
                format!("{} B", meta.len())
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", size),
                Style::default().fg(app.colors.empty_value),
            )));
        }

        if let Ok(tagged_file) = read_from_path(filepath) {
            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                lines.push(Line::from(""));

                let fields = [
                    (ItemKey::TrackTitle, app.l.field_title()),
                    (ItemKey::TrackArtist, app.l.field_artist()),
                    (ItemKey::AlbumTitle, app.l.field_album()),
                    (ItemKey::RecordingDate, app.l.field_year()),
                    (ItemKey::Genre, app.l.field_genre()),
                    (ItemKey::TrackNumber, app.l.field_track()),
                ];

                for (key, label) in fields.iter() {
                    if let Some(val) = tag.get_string(key) {
                        if !val.is_empty() {
                            lines.push(Line::from(vec![
                                Span::styled(
                                    format!("  {}: ", label),
                                    Style::default()
                                        .fg(app.colors.metadata_label)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    val.to_string(),
                                    Style::default().fg(app.colors.metadata_value),
                                ),
                            ]));
                        }
                    }
                }
            }
        }
    }

    if lines.is_empty() {
        return;
    }

    let line_count = lines.len() as u16;

    if app.cover_protocol.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(line_count), Constraint::Min(0)])
            .split(inner);
        let top = chunks[0];
        let bottom = chunks[1];

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(para, top);

        if let Some(protocol) = &mut app.cover_protocol {
            if bottom.height >= 2 && bottom.width >= 4 {
                frame.render_widget(Clear, bottom);
                let image = StatefulImage::default().resize(Resize::Fit(None));
                frame.render_stateful_widget(image, bottom, protocol);
            }
        }
    } else {
        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(para, inner);
    }
}



fn render_filter_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 3;
    let x = area.width.saturating_sub(popup_width) / 2;
    let y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let block = Block::default()
        .title(app.l.filter_title())
        .borders(Borders::ALL)
        .style(Style::default().fg(app.colors.filter_border));

    let filter_display = format!("> {}_", app.filter_text);
    let filter_para = Paragraph::new(filter_display).style(
        Style::default()
            .fg(app.colors.active_label_fg)
            .bg(app.colors.active_value_bg)
            .add_modifier(Modifier::BOLD),
    );

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    frame.render_widget(filter_para, inner);
}

fn render_delete_confirm(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let count = app.delete_indices.len();
    let filename = if count == 1 {
        app.get_item(app.delete_indices[0])
            .map(|(p, _)| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    let msg = if count == 1 {
        app.l.confirm_delete_one(&filename)
    } else {
        app.l.confirm_delete_many(count)
    };

    let popup_width = (msg.len() as u16 + 6).min(area.width.saturating_sub(2));
    let popup_height = 3;
    let x = area.width.saturating_sub(popup_width) / 2;
    let y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let block = Block::default()
        .title(app.l.confirm_delete_title())
        .borders(Borders::ALL)
        .style(Style::default().fg(app.colors.delete_border));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let msg_para = Paragraph::new(msg)
        .style(
            Style::default()
                .fg(app.colors.foreground)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(
        msg_para,
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );

    let hint = Paragraph::new(app.l.confirm_hint())
        .style(Style::default().fg(app.colors.help_text))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(
        hint,
        Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        },
    );
}

fn render_cover_art_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let title = app.l.cover_title();
    let hint = app.l.cover_hint();

    let popup_width = (60).min(area.width.saturating_sub(4));
    let popup_height = 5;
    let x = area.width.saturating_sub(popup_width) / 2;
    let y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().fg(app.colors.cover_border));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let input_width = inner.width as usize;
    let display_path = if app.cover_path.len() > input_width.saturating_sub(2) {
        let start = app.cover_path.len().saturating_sub(input_width.saturating_sub(2));
        format!("...{}", &app.cover_path[start..])
    } else {
        app.cover_path.clone()
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(app.colors.input_border));

    let input_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 3,
    };
    let input_inner = input_block.inner(input_area);
    frame.render_widget(input_block, input_area);

    let input_para = Paragraph::new(display_path);
    frame.render_widget(input_para, input_inner);

    if app.mode == AppMode::SetCoverArt {
        let cursor_x = input_inner.x + app.cover_cursor as u16;
        frame.set_cursor_position((cursor_x, input_inner.y));
    }

    let hint_para = Paragraph::new(hint)
        .style(Style::default().fg(app.colors.help_text))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(
        hint_para,
        Rect {
            x: inner.x,
            y: inner.y + 3,
            width: inner.width,
            height: 1,
        },
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let l = L::detect();

    let (config_dir, show_preview) = load_config();

    let start_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or(config_dir)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));

    let start_dir = if start_dir.exists() {
        start_dir.canonicalize()?
    } else {
        eprintln!("{}", l.err_invalid_dir(&start_dir));
        std::process::exit(1);
    };

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(start_dir, l, show_preview);

    eprintln!("{}", app.l.lang_detected());

    app.picker = Some(Picker::from_query_stdio()?);
    app.update_cover_state();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
