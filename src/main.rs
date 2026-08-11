use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
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
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize, StatefulImage};
use serde::Deserialize;

#[derive(Deserialize)]
struct MbResponse {
    recordings: Vec<MbRecording>,
}

#[derive(Deserialize)]
struct MbRecording {
    title: String,
    #[serde(rename = "artist-credit")]
    artist_credit: Vec<MbArtistCredit>,
    releases: Option<Vec<MbRelease>>,
}

#[derive(Deserialize)]
struct MbArtistCredit {
    name: String,
}

#[derive(Deserialize)]
struct MbRelease {
    id: String,
    title: String,
    date: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct MbReleaseDetail {
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    media: Option<Vec<MbMedium>>,
    tags: Option<Vec<MbTag>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MbMedium {
    position: Option<u16>,
    #[serde(rename = "track-count")]
    track_count: Option<u16>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MbTag {
    name: String,
    count: Option<u16>,
}

#[derive(Deserialize)]
struct MbReleaseSearchResponse {
    releases: Vec<MbReleaseSearchResult>,
}

#[derive(Deserialize)]
struct MbReleaseSearchResult {
    id: String,
    title: String,
    date: Option<String>,
    status: Option<String>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
}

#[derive(Clone, Copy, PartialEq)]
enum MbSource {
    Recording,
    Release,
}

struct MbSuggestion {
    source: MbSource,
    title: String,
    artist: String,
    album: String,
    albumartist: String,
    year: String,
    disc: String,
    genre: String,
    comment: String,
    release_id: String,
    release_status: String,
}

enum CoverState {
    None,
    Loading,
    Loaded(Vec<u8>),
    Error(String),
}

enum MbState {
    Idle,
    LoadingSuggestions,
    Suggestions {
        results: Vec<MbSuggestion>,
        index: usize,
        cover_state: CoverState,
    },
    Error(String),
}

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

    fn set(&mut self, key: &str, value: &str) {
        let Some(c) = parse_hex_color(value) else {
            return;
        };
        match key {
            "background" => self.background = c,
            "foreground" => self.foreground = c,
            "header_bg" => self.header_bg = c,
            "header_fg" => self.header_fg = c,
            "dir_path" => self.dir_path = c,
            "help_text" => self.help_text = c,
            "folder" => self.folder = c,
            "selected" => self.selected = c,
            "normal_file" => self.normal_file = c,
            "highlight_bg" => self.highlight_bg = c,
            "highlight_fg" => self.highlight_fg = c,
            "info" => self.info = c,
            "error" => self.error = c,
            "success" => self.success = c,
            "edit_batch" => self.edit_batch = c,
            "edit_individual" => self.edit_individual = c,
            "active_label_bg" => self.active_label_bg = c,
            "active_label_fg" => self.active_label_fg = c,
            "inactive_label" => self.inactive_label = c,
            "active_value_bg" => self.active_value_bg = c,
            "active_value_fg" => self.active_value_fg = c,
            "empty_value" => self.empty_value = c,
            "filled_value" => self.filled_value = c,
            "preview_border" => self.preview_border = c,
            "filter_border" => self.filter_border = c,
            "delete_border" => self.delete_border = c,
            "cover_border" => self.cover_border = c,
            "input_border" => self.input_border = c,
            "metadata_label" => self.metadata_label = c,
            "metadata_value" => self.metadata_value = c,
            _ => {}
        }
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#').trim_start_matches("0x");
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("musictag")
}

fn config_path() -> PathBuf {
    config_dir().join("config")
}

fn covers_dir() -> PathBuf {
    config_dir().join("covers")
}

struct Config {
    dir: Option<PathBuf>,
    show_preview: bool,
    nav: NavScheme,
    colors: ColorScheme,
    custom_keys: HashMap<Action, Vec<KeySpec>>,
    border: BorderStyle,
    border_overrides: HashMap<String, BorderStyle>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            dir: None,
            show_preview: true,
            nav: NavScheme::default(),
            colors: ColorScheme::default(),
            custom_keys: HashMap::new(),
            border: BorderStyle::default(),
            border_overrides: HashMap::new(),
        }
    }
}

fn load_config() -> Config {
    let path = config_path();
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Config::default(),
    };

    let mut config = Config::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "default_dir" => {
                    let p = PathBuf::from(value);
                    if p.exists() && p.is_dir() {
                        config.dir = Some(p);
                    }
                }
                "show_preview" => {
                    config.show_preview = value != "false";
                }
                "nav" => {
                    config.nav = NavScheme::parse(value);
                }
                "border" => {
                    config.border = BorderStyle::parse(value);
                }
                _ => {
                    if let Some(color_key) = key.strip_prefix("color.") {
                        config.colors.set(color_key, value);
                    } else if let Some(action_key) = key.strip_prefix("key.") {
                        if let Some(action) = Action::from_name(action_key) {
                            let keys: Vec<KeySpec> = value
                                .split(',')
                                .filter_map(|p| KeySpec::parse(p.trim()))
                                .collect();
                            config.custom_keys.insert(action, keys);
                        }
                    } else if let Some(border_key) = key.strip_prefix("border.") {
                        config.border_overrides.insert(
                            border_key.trim().to_string(),
                            BorderStyle::parse(value),
                        );
                    }
                }
            }
        } else if config.dir.is_none() {
            let p = PathBuf::from(line);
            if p.exists() && p.is_dir() {
                config.dir = Some(p);
            }
        }
    }

    config
}

fn color_hex(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => String::new(),
    }
}

fn example_theme_block() -> String {
    let c = ColorScheme::catppuccin_mocha();
    let color_lines = [
        ("background", c.background),
        ("foreground", c.foreground),
        ("header_bg", c.header_bg),
        ("header_fg", c.header_fg),
        ("dir_path", c.dir_path),
        ("help_text", c.help_text),
        ("folder", c.folder),
        ("selected", c.selected),
        ("normal_file", c.normal_file),
        ("highlight_bg", c.highlight_bg),
        ("highlight_fg", c.highlight_fg),
        ("info", c.info),
        ("error", c.error),
        ("success", c.success),
        ("edit_batch", c.edit_batch),
        ("edit_individual", c.edit_individual),
        ("active_label_bg", c.active_label_bg),
        ("active_label_fg", c.active_label_fg),
        ("inactive_label", c.inactive_label),
        ("active_value_bg", c.active_value_bg),
        ("active_value_fg", c.active_value_fg),
        ("empty_value", c.empty_value),
        ("filled_value", c.filled_value),
        ("preview_border", c.preview_border),
        ("filter_border", c.filter_border),
        ("delete_border", c.delete_border),
        ("cover_border", c.cover_border),
        ("input_border", c.input_border),
        ("metadata_label", c.metadata_label),
        ("metadata_value", c.metadata_value),
    ];

    let mut out = String::from(
        "\n# ==== Tema de ejemplo (Catppuccin Mocha) ====\n\
         # Descomenta una línea para cambiar ese color.\n",
    );
    for (key, color) in color_lines {
        out.push_str(&format!("# color.{}={}\n", key, color_hex(color)));
    }
    out.push_str(
        "\n# ==== Bordes (single, rounded, double, thick, none) ====\n\
         # border=rounded\n\
         # border.preview=double\n\
         # border.filter=rounded\n\
         # border.delete=double\n\
         # border.cover=rounded\n\
         # border.input=single\n",
    );
    out
}

fn save_config(dir: &Path, show_preview: bool, nav: NavScheme) -> Result<String, String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create config dir: {}", e))?;
    }
    let mut extras: Vec<String> = Vec::new();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            let t = line.trim_start();
            if t.starts_with("color.")
                || t.starts_with("key.")
                || t.starts_with("border.")
                || t == "border"
            {
                extras.push(line.to_string());
            }
        }
    }
    let mut content = format!(
        "default_dir={}\nshow_preview={}\nnav={}\n",
        dir.display(),
        show_preview,
        nav.name()
    );
    for e in extras {
        content.push_str(&e);
        content.push('\n');
    }
    content.push_str(&example_theme_block());
    fs::write(&path, content.as_bytes())
        .map_err(|e| format!("Could not save config: {}", e))?;
    Ok(content)
}

mod icon {
    pub const FOLDER: &str = "\u{f07b}";
    pub const CHECK: &str = "\u{f00c}";
    pub const CLOSE: &str = "\u{f00d}";
    pub const EDIT: &str = "\u{f303}";
    pub const SEARCH: &str = "\u{f002}";
    pub const DISK: &str = "\u{f7c2}";
    pub const FILTER: &str = "\u{f0b0}";
    pub const MUSIC_FILE: &str = "\u{f1c6}";
    pub const TAG: &str = "\u{f02c}";
    pub const SELECT: &str = "\u{f14a}";
    pub const TRASH: &str = "\u{f2ed}";
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
        match self.lang {
            Lang::Es => "Álbum",
            Lang::En => "Album",
        }
    }

    fn field_albumartist(&self) -> &str {
        match self.lang {
            Lang::Es => "Artista del Álbum",
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
            Lang::Es => format!(" {} musictag - Editor de Metadatos de Música ", icon::TAG),
            Lang::En => format!(" {} musictag - Music Metadata Editor ", icon::TAG),
        }
    }

    fn help_browse(&self, km: &Keymap) -> String {
        let first = |a: Action| {
            km.keys_for(a, false)
                .first()
                .map(|k| k.label())
                .unwrap_or_default()
        };
        match self.lang {
            Lang::Es => format!(
                " {}/{}:mover  {}:abrir  {}:seleccionar  {}:todo  {}:editar  {}:eliminar  {}:portada  {}:extraer  {}:filtrar  {}:teclas  {}:preview  {}:cfg  {}:ayuda  {}:salir",
                first(Action::Up),
                first(Action::Down),
                first(Action::Right),
                first(Action::ToggleSelect),
                first(Action::ApplySelected),
                first(Action::ApplyAll),
                first(Action::Delete),
                first(Action::SetCoverArt),
                first(Action::ExtractCover),
                first(Action::Filter),
                first(Action::ToggleNav),
                first(Action::TogglePreview),
                first(Action::SaveConfig),
                first(Action::ToggleHelp),
                first(Action::Quit),
            ),
            Lang::En => format!(
                " {}/{}:move  {}:open  {}:select  {}:all  {}:edit  {}:delete  {}:cover  {}:extract  {}:filter  {}:keys  {}:preview  {}:cfg  {}:help  {}:quit",
                first(Action::Up),
                first(Action::Down),
                first(Action::Right),
                first(Action::ToggleSelect),
                first(Action::ApplySelected),
                first(Action::ApplyAll),
                first(Action::Delete),
                first(Action::SetCoverArt),
                first(Action::ExtractCover),
                first(Action::Filter),
                first(Action::ToggleNav),
                first(Action::TogglePreview),
                first(Action::SaveConfig),
                first(Action::ToggleHelp),
                first(Action::Quit),
            ),
        }
    }

    fn help_edit(&self, km: &Keymap) -> String {
        let first = |a: Action| {
            km.keys_for(a, true)
                .first()
                .map(|k| k.label())
                .unwrap_or_default()
        };
        match self.lang {
            Lang::Es => format!(
                " {}/{}:campo  {}/{}:cursor  {}:aplicar MB  {}/{}:MB nav  {}:MB navegador  {}:guardar  {}:cancelar  {}:guardar y avanzar",
                first(Action::Up),
                first(Action::Down),
                first(Action::Left),
                first(Action::Right),
                first(Action::ApplyMb),
                first(Action::MbPrev),
                first(Action::MbNext),
                first(Action::MbBrowser),
                first(Action::Enter),
                first(Action::Escape),
                first(Action::SaveNext),
            ),
            Lang::En => format!(
                " {}/{}:field  {}/{}:cursor  {}:apply MB  {}/{}:MB nav  {}:MB browser  {}:save  {}:cancel  {}:save & next",
                first(Action::Up),
                first(Action::Down),
                first(Action::Left),
                first(Action::Right),
                first(Action::ApplyMb),
                first(Action::MbPrev),
                first(Action::MbNext),
                first(Action::MbBrowser),
                first(Action::Enter),
                first(Action::Escape),
                first(Action::SaveNext),
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

    fn help_shown(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Ayuda visible", icon::CHECK),
            Lang::En => format!("{} Help shown", icon::CHECK),
        }
    }

    fn help_hidden(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Ayuda oculta", icon::CHECK),
            Lang::En => format!("{} Help hidden", icon::CHECK),
        }
    }

    fn nav_vim(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Teclas vim activadas", icon::CHECK),
            Lang::En => format!("{} Vim keys enabled", icon::CHECK),
        }
    }

    fn nav_arrows(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Teclas de flechas activadas", icon::CHECK),
            Lang::En => format!("{} Arrow keys enabled", icon::CHECK),
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
            Lang::Es => format!("{} Idioma detectado: Español", icon::GLOBE),
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

    fn config_reloaded(&self) -> String {
        match self.lang {
            Lang::Es => format!("{} Config recargado", icon::CHECK),
            Lang::En => format!("{} Config reloaded", icon::CHECK),
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

    fn cover_batch_title(&self, count: usize) -> String {
        match self.lang {
            Lang::Es => format!(" {} Asignar portada ({} archivos) ", icon::IMAGE, count),
            Lang::En => format!(" {} Set cover art ({} files) ", icon::IMAGE, count),
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BorderStyle {
    Single,
    Rounded,
    Double,
    Thick,
    None,
}

impl BorderStyle {
    fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "rounded" | "redondeado" => BorderStyle::Rounded,
            "double" | "doble" => BorderStyle::Double,
            "thick" | "grueso" => BorderStyle::Thick,
            "none" | "ninguno" | "off" => BorderStyle::None,
            _ => BorderStyle::Single,
        }
    }

    fn border_type(&self) -> BorderType {
        match self {
            BorderStyle::Single => BorderType::Plain,
            BorderStyle::Rounded => BorderType::Rounded,
            BorderStyle::Double => BorderType::Double,
            BorderStyle::Thick => BorderType::Thick,
            BorderStyle::None => BorderType::Plain,
        }
    }
}

impl Default for BorderStyle {
    fn default() -> Self {
        BorderStyle::Single
    }
}

#[derive(Clone, Copy, PartialEq)]
enum NavScheme {
    Vim,
    Arrows,
}

impl NavScheme {
    fn parse(s: &str) -> Self {
        match s.trim() {
            "arrows" => NavScheme::Arrows,
            _ => NavScheme::Vim,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            NavScheme::Vim => "vim",
            NavScheme::Arrows => "arrows",
        }
    }
}

impl Default for NavScheme {
    fn default() -> Self {
        NavScheme::Vim
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KeySpec {
    Char(char),
    Ctrl(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

impl KeySpec {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(c) = s.strip_prefix("ctrl+") {
            let mut it = c.chars();
            let ch = it.next()?;
            if it.next().is_some() {
                return None;
            }
            return Some(KeySpec::Ctrl(ch));
        }
        match s.to_lowercase().as_str() {
            "enter" | "return" => Some(KeySpec::Enter),
            "esc" | "escape" => Some(KeySpec::Esc),
            "tab" => Some(KeySpec::Tab),
            "backtab" | "shift-tab" => Some(KeySpec::BackTab),
            "backspace" | "bs" | "retroceso" => Some(KeySpec::Backspace),
            "delete" | "del" | "supr" | "suprimir" => Some(KeySpec::Delete),
            "up" | "arriba" => Some(KeySpec::Up),
            "down" | "abajo" => Some(KeySpec::Down),
            "left" | "izquierda" => Some(KeySpec::Left),
            "right" | "derecha" => Some(KeySpec::Right),
            "home" | "inicio" => Some(KeySpec::Home),
            "end" | "fin" => Some(KeySpec::End),
            "pageup" | "pgup" => Some(KeySpec::PageUp),
            "pagedown" | "pgdn" => Some(KeySpec::PageDown),
            "space" | "espacio" => Some(KeySpec::Char(' ')),
            _ => {
                let mut it = s.chars();
                let ch = it.next()?;
                if it.next().is_some() {
                    None
                } else {
                    Some(KeySpec::Char(ch))
                }
            }
        }
    }

    fn matches(&self, key: &KeyEvent) -> bool {
        match self {
            KeySpec::Char(c) => {
                let mods = key.modifiers.bits() & !KeyModifiers::SHIFT.bits();
                mods == 0 && key.code == KeyCode::Char(*c)
            }
            KeySpec::Ctrl(c) => {
                key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && key.code == KeyCode::Char(*c)
            }
            KeySpec::Enter => key.code == KeyCode::Enter,
            KeySpec::Esc => key.code == KeyCode::Esc,
            KeySpec::Tab => key.code == KeyCode::Tab,
            KeySpec::BackTab => key.code == KeyCode::BackTab,
            KeySpec::Backspace => key.code == KeyCode::Backspace,
            KeySpec::Delete => key.code == KeyCode::Delete,
            KeySpec::Up => key.code == KeyCode::Up,
            KeySpec::Down => key.code == KeyCode::Down,
            KeySpec::Left => key.code == KeyCode::Left,
            KeySpec::Right => key.code == KeyCode::Right,
            KeySpec::Home => key.code == KeyCode::Home,
            KeySpec::End => key.code == KeyCode::End,
            KeySpec::PageUp => key.code == KeyCode::PageUp,
            KeySpec::PageDown => key.code == KeyCode::PageDown,
        }
    }

    fn label(&self) -> String {
        match self {
            KeySpec::Char(c) => c.to_string(),
            KeySpec::Ctrl(c) => format!("Ctrl+{}", c.to_uppercase()),
            KeySpec::Enter => "Enter".into(),
            KeySpec::Esc => "Esc".into(),
            KeySpec::Tab => "Tab".into(),
            KeySpec::BackTab => "Shift+Tab".into(),
            KeySpec::Backspace => "Retr".into(),
            KeySpec::Delete => "Supr".into(),
            KeySpec::Up => "↑".into(),
            KeySpec::Down => "↓".into(),
            KeySpec::Left => "←".into(),
            KeySpec::Right => "→".into(),
            KeySpec::Home => "Inicio".into(),
            KeySpec::End => "Fin".into(),
            KeySpec::PageUp => "Pág↑".into(),
            KeySpec::PageDown => "Pág↓".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    Quit,
    ToggleNav,
    ToggleHelp,
    ToggleSelect,
    Filter,
    Reload,
    ApplyAll,
    ApplySelected,
    ClearSelection,
    Delete,
    SaveConfig,
    SetCoverArt,
    ResetColors,
    TogglePreview,
    ExtractCover,
    NextField,
    PrevField,
    SaveNext,
    ClearField,
    ApplyToAll,
    ApplyMb,
    MbPrev,
    MbNext,
    MbBrowser,
    ConfirmYes,
    ConfirmNo,
    Down,
    Up,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
    Backspace,
    DeleteChar,
}

impl Action {
    fn from_name(s: &str) -> Option<Self> {
        Some(match s.trim() {
            "quit" => Action::Quit,
            "toggle_nav" => Action::ToggleNav,
            "toggle_help" => Action::ToggleHelp,
            "toggle_select" => Action::ToggleSelect,
            "filter" => Action::Filter,
            "reload" => Action::Reload,
            "apply_all" => Action::ApplyAll,
            "apply_selected" => Action::ApplySelected,
            "clear_selection" => Action::ClearSelection,
            "delete" => Action::Delete,
            "save_config" => Action::SaveConfig,
            "set_cover_art" => Action::SetCoverArt,
            "reset_colors" => Action::ResetColors,
            "toggle_preview" => Action::TogglePreview,
            "extract_cover" => Action::ExtractCover,
            "next_field" => Action::NextField,
            "prev_field" => Action::PrevField,
            "save_next" => Action::SaveNext,
            "clear_field" => Action::ClearField,
            "apply_to_all" => Action::ApplyToAll,
            "apply_mb" => Action::ApplyMb,
            "mb_prev" => Action::MbPrev,
            "mb_next" => Action::MbNext,
            "mb_browser" => Action::MbBrowser,
            "confirm_yes" => Action::ConfirmYes,
            "confirm_no" => Action::ConfirmNo,
            "down" => Action::Down,
            "up" => Action::Up,
            "left" => Action::Left,
            "right" => Action::Right,
            "home" => Action::Home,
            "end" => Action::End,
            "enter" => Action::Enter,
            "escape" => Action::Escape,
            "backspace" => Action::Backspace,
            "delete_char" => Action::DeleteChar,
            _ => return None,
        })
    }
}

fn preset_browse(nav: NavScheme) -> Vec<(Action, Vec<KeySpec>)> {
    let ks = |s: &str| KeySpec::parse(s).unwrap();
    let vim = nav == NavScheme::Vim;
    vec![
        (Action::Quit, vec![ks("q"), ks("Q")]),
        (Action::ToggleNav, vec![ks("N")]),
        (Action::ToggleHelp, vec![ks(if vim { "/" } else { "h" })]),
        (Action::ToggleSelect, vec![ks("a"), ks("space")]),
        (
            Action::Filter,
            if vim {
                vec![ks("f")]
            } else {
                vec![ks("f"), ks("/")]
            },
        ),
        (Action::Reload, vec![ks("r")]),
        (Action::ApplyAll, vec![ks("A")]),
        (Action::ApplySelected, vec![ks("V"), ks("v")]),
        (Action::ClearSelection, vec![ks("d")]),
        (Action::Delete, vec![ks("x"), ks("delete")]),
        (Action::SaveConfig, vec![ks("C")]),
        (Action::SetCoverArt, vec![ks("c")]),
        (Action::ResetColors, vec![ks("R")]),
        (Action::TogglePreview, vec![ks("P")]),
        (Action::ExtractCover, vec![ks("e")]),
        (Action::Down, vec![ks(if vim { "j" } else { "down" })]),
        (Action::Up, vec![ks(if vim { "k" } else { "up" })]),
        (Action::Left, vec![ks(if vim { "h" } else { "left" })]),
        (Action::Right, vec![ks(if vim { "l" } else { "right" })]),
        (Action::Home, vec![ks(if vim { "g" } else { "home" })]),
        (Action::End, vec![ks(if vim { "G" } else { "end" })]),
        (Action::Enter, vec![ks("enter")]),
        (Action::Escape, vec![ks("esc")]),
    ]
}

fn preset_text() -> Vec<(Action, Vec<KeySpec>)> {
    let ks = |s: &str| KeySpec::parse(s).unwrap();
    vec![
        (Action::NextField, vec![ks("tab")]),
        (Action::PrevField, vec![ks("backtab")]),
        (Action::SaveNext, vec![ks("ctrl+s")]),
        (Action::ClearField, vec![ks("ctrl+u")]),
        (Action::ApplyToAll, vec![ks("ctrl+c")]),
        (Action::ApplyMb, vec![ks("ctrl+g")]),
        (Action::MbPrev, vec![ks("pageup")]),
        (Action::MbNext, vec![ks("pagedown")]),
        (Action::MbBrowser, vec![ks("ctrl+o")]),
        (Action::ConfirmYes, vec![ks("y"), ks("Y")]),
        (Action::ConfirmNo, vec![ks("n"), ks("N")]),
        (Action::Down, vec![ks("down")]),
        (Action::Up, vec![ks("up")]),
        (Action::Left, vec![ks("left")]),
        (Action::Right, vec![ks("right")]),
        (Action::Home, vec![ks("home")]),
        (Action::End, vec![ks("end")]),
        (Action::Enter, vec![ks("enter")]),
        (Action::Escape, vec![ks("esc")]),
        (Action::Backspace, vec![ks("backspace")]),
        (Action::DeleteChar, vec![ks("delete")]),
    ]
}

struct Keymap {
    browse: HashMap<Action, Vec<KeySpec>>,
    text: HashMap<Action, Vec<KeySpec>>,
}

impl Keymap {
    fn new(nav: NavScheme, custom: &HashMap<Action, Vec<KeySpec>>) -> Self {
        let mut browse: HashMap<Action, Vec<KeySpec>> = preset_browse(nav).into_iter().collect();
        let mut text: HashMap<Action, Vec<KeySpec>> = preset_text().into_iter().collect();
        for (action, keys) in custom {
            browse.insert(*action, keys.clone());
            text.insert(*action, keys.clone());
        }
        Keymap { browse, text }
    }

    fn action(&self, key: &KeyEvent, is_text: bool) -> Option<Action> {
        let map = if is_text { &self.text } else { &self.browse };
        for (action, keys) in map {
            if keys.iter().any(|k| k.matches(key)) {
                return Some(*action);
            }
        }
        None
    }

    fn keys_for(&self, action: Action, is_text: bool) -> &[KeySpec] {
        let map = if is_text { &self.text } else { &self.browse };
        map.get(&action).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

struct App {
    current_dir: PathBuf,
    dir_entries: Vec<PathBuf>,
    files: Vec<PathBuf>,
    current_idx: usize,
    scroll_offset: usize,
    mode: AppMode,
    nav: NavScheme,
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
    show_help: bool,
    show_preview: bool,
    status_msg: String,
    status_is_error: bool,
    should_quit: bool,
    l: L,
    colors: ColorScheme,
    config_colors: ColorScheme,
    keymap: Keymap,
    custom_keys: HashMap<Action, Vec<KeySpec>>,
    border_default: BorderStyle,
    border_overrides: HashMap<String, BorderStyle>,
    last_config_content: String,
    last_written_config: String,
    picker: Option<Picker>,
    cover_protocol: Option<StatefulProtocol>,
    cover_protocol_file: Option<PathBuf>,
    cover_cache: HashMap<PathBuf, StatefulProtocol>,
    local_cover_rx: Option<(PathBuf, mpsc::Receiver<(PathBuf, Option<image::DynamicImage>)>)>,
    mb_state: MbState,
    mb_rx: Option<mpsc::Receiver<MbState>>,
    cover_rx: Option<mpsc::Receiver<CoverState>>,
    detail_rx: Option<mpsc::Receiver<MbSuggestion>>,
    mb_cover_protocol: Option<StatefulProtocol>,
    last_area: Rect,
    last_click: Option<(std::time::Instant, u16, u16)>,
}

impl App {
    fn new(start_dir: PathBuf, l: L, config: Config) -> Self {
        let custom_keys = config.custom_keys;
        let keymap = Keymap::new(config.nav, &custom_keys);
        let startup_config = fs::read_to_string(config_path()).unwrap_or_default();
        let mut app = App {
            current_dir: start_dir,
            dir_entries: Vec::new(),
            files: Vec::new(),
            current_idx: 0,
            scroll_offset: 0,
            mode: AppMode::Browse,
            nav: config.nav,
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
            show_help: true,
            show_preview: config.show_preview,
            status_msg: String::new(),
            status_is_error: false,
            should_quit: false,
            l,
            colors: config.colors.clone(),
            config_colors: config.colors,
            keymap,
            custom_keys,
            border_default: config.border,
            border_overrides: config.border_overrides,
            last_config_content: startup_config.clone(),
            last_written_config: startup_config,
            picker: None,
            cover_protocol: None,
            cover_protocol_file: None,
            cover_cache: HashMap::new(),
            local_cover_rx: None,
            mb_state: MbState::Idle,
            mb_rx: None,
            cover_rx: None,
            detail_rx: None,
            mb_cover_protocol: None,
            last_area: Rect::new(0, 0, 0, 0),
            last_click: None,
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

    fn current_cover_file(&self) -> Option<PathBuf> {
        if self.current_idx < self.dir_entries.len() {
            return None;
        }
        let fidx = self.current_idx - self.dir_entries.len();
        self.files.get(fidx).cloned()
    }

    fn update_cover_state(&mut self) {
        let Some(fp) = self.current_cover_file() else {
            self.cover_protocol = None;
            self.cover_protocol_file = None;
            return;
        };

        // already showing this file's cover
        if self.cover_protocol_file.as_ref() == Some(&fp) {
            return;
        }

        // stash the currently displayed protocol back into the cache
        if let Some(pf) = self.cover_protocol_file.take() {
            if let Some(p) = self.cover_protocol.take() {
                if self.cover_cache.len() >= 48 {
                    if let Some(oldest) = self.cover_cache.keys().next().cloned() {
                        self.cover_cache.remove(&oldest);
                    }
                }
                self.cover_cache.insert(pf, p);
            }
        }

        // cache hit: reuse cached protocol (no re-read, no re-encode)
        if let Some(p) = self.cover_cache.remove(&fp) {
            self.cover_protocol = Some(p);
            self.cover_protocol_file = Some(fp);
            return;
        }

        // no cached protocol: read + decode in a background thread
        // (skip if we already have a pending request for this same file)
        if let Some((pending, _)) = &self.local_cover_rx {
            if pending == &fp {
                return;
            }
        }
        self.cover_protocol = None;
        self.cover_protocol_file = None;
        let (tx, rx) = mpsc::channel();
        self.local_cover_rx = Some((fp.clone(), rx));
        thread::spawn(move || {
            let img = decode_cover_image(&fp);
            let _ = tx.send((fp, img));
        });
    }

    fn check_local_cover(&mut self) {
        if let Some((_, rx)) = &self.local_cover_rx {
            if let Ok((path, img)) = rx.try_recv() {
                self.local_cover_rx = None;
                if let Some(img) = img {
                    if let Some(picker) = &self.picker {
                        let protocol = picker.new_resize_protocol(img);
                        let is_current = self.current_cover_file().as_ref() == Some(&path);
                        if is_current && self.cover_protocol.is_none() {
                            self.cover_protocol = Some(protocol);
                            self.cover_protocol_file = Some(path);
                        } else {
                            if self.cover_cache.len() >= 48 {
                                if let Some(oldest) = self.cover_cache.keys().next().cloned() {
                                    self.cover_cache.remove(&oldest);
                                }
                            }
                            self.cover_cache.insert(path, protocol);
                        }
                    }
                }
            }
        }
    }

    fn check_mb_results(&mut self) {
        if let Some(rx) = &self.mb_rx {
            if let Ok(state) = rx.try_recv() {
                self.mb_state = state;
                self.mb_rx = None;
                if let MbState::Suggestions { results, .. } = &self.mb_state {
                    if !results.is_empty() {
                        self.download_mb_cover(0);
                        self.load_mb_detail(0);
                    }
                }
            }
        }
        if let Some(rx) = &self.cover_rx {
            if let Ok(cover) = rx.try_recv() {
                self.cover_rx = None;
                let index = if let MbState::Suggestions { index, .. } = &self.mb_state {
                    *index
                } else {
                    return;
                };
                if let MbState::Suggestions { cover_state, .. } = &mut self.mb_state {
                    *cover_state = cover;
                }
                self.decode_mb_cover(index);
            }
        }
        if let Some(rx) = &self.detail_rx {
            if let Ok(detail) = rx.try_recv() {
                self.detail_rx = None;
                if let MbState::Suggestions { results, index, .. } = &mut self.mb_state {
                    if *index < results.len() {
                        if !detail.albumartist.is_empty() {
                            results[*index].albumartist = detail.albumartist;
                        }
                        if !detail.disc.is_empty() {
                            results[*index].disc = detail.disc;
                        }
                        if !detail.genre.is_empty() {
                            results[*index].genre = detail.genre;
                        }
                    }
                }
            }
        }
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
        self.mb_state = MbState::Idle;
        self.mb_rx = None;
        self.cover_rx = None;
        self.detail_rx = None;
        self.mb_cover_protocol = None;

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
                    self.edit_cursor = self.edit_vals[0].chars().count();
                    self.mode = AppMode::Edit;
                    self.status_msg.clear();
                    self.query_mb();
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
                    self.edit_cursor = self.edit_vals[0].chars().count();
                    self.mode = AppMode::Edit;
                    self.status_msg.clear();
                    self.query_mb();
                }
                None => {
                    let name = filepath.file_name().unwrap_or_default().to_string_lossy();
                    self.status_msg = self.l.err_could_not_read_file(&name);
                    self.status_is_error = true;
                }
            }
        }
    }

    fn query_mb(&mut self) {
        let title = self.edit_vals.first().map(|s| s.trim()).unwrap_or("");
        let artist = self.edit_vals.get(1).map(|s| s.trim()).unwrap_or("");
        let album = self.edit_vals.get(2).map(|s| s.trim()).unwrap_or("");
        if title.is_empty() && artist.is_empty() && album.is_empty() {
            self.mb_state = MbState::Idle;
            return;
        }
        self.mb_state = MbState::LoadingSuggestions;
        let (tx, rx) = mpsc::channel();
        self.mb_rx = Some(rx);
        let title = title.to_string();
        let artist = artist.to_string();
        let album = album.to_string();
        thread::spawn(move || {
            let result = query_musicbrainz(&title, &artist, &album);
            let _ = tx.send(result);
        });
    }

    fn download_mb_cover(&mut self, index: usize) {
        if let MbState::Suggestions { results, cover_state, .. } = &mut self.mb_state {
            if index >= results.len() {
                return;
            }
            let rid = results[index].release_id.clone();
            if rid.is_empty() {
                *cover_state = CoverState::None;
                return;
            }
            *cover_state = CoverState::Loading;
            let (tx, rx) = mpsc::channel();
            self.cover_rx = Some(rx);
            thread::spawn(move || {
                let url = format!("https://coverartarchive.org/release/{}/front", rid);
                let config = ureq::Agent::config_builder()
                    .user_agent("musictag/0.1.0 ( jpablo@example.com )")
                    .build();
                let agent = ureq::Agent::new_with_config(config);
                let resp = match agent.get(&url).call() {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(CoverState::Error(format!("Cover download: {}", e)));
                        return;
                    }
                };
                let mut reader = resp.into_body().into_reader();
                let mut buf = Vec::new();
                match std::io::Read::read_to_end(&mut reader, &mut buf) {
                    Ok(_) => { let _ = tx.send(CoverState::Loaded(buf)); }
                    Err(e) => { let _ = tx.send(CoverState::Error(format!("Cover read: {}", e))); }
                }
            });
        }
    }

    fn decode_mb_cover(&mut self, _index: usize) {
        if let MbState::Suggestions { cover_state, .. } = &self.mb_state {
            if let CoverState::Loaded(data) = cover_state {
                if let Ok(img) = image::load_from_memory(data) {
                    if let Some(picker) = &self.picker {
                        self.mb_cover_protocol = Some(picker.new_resize_protocol(img));
                    }
                }
            }
        }
    }

    fn load_mb_detail(&mut self, index: usize) {
        if let MbState::Suggestions { results, .. } = &self.mb_state {
            if index >= results.len() {
                return;
            }
            let rid = results[index].release_id.clone();
            if rid.is_empty() {
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.detail_rx = Some(rx);
            thread::spawn(move || {
                let url = format!(
                    "https://musicbrainz.org/ws/2/release/{}?fmt=json&inc=artist-credits+media+tags",
                    rid
                );
                let config = ureq::Agent::config_builder()
                    .user_agent("musictag/0.1.0 ( jpablo@example.com )")
                    .build();
                let agent = ureq::Agent::new_with_config(config);
                let mut resp = match agent.get(&url).call() {
                    Ok(r) => r,
                    Err(_) => return,
                };
                let body: String = match resp.body_mut().read_to_string() {
                    Ok(b) => b,
                    Err(_) => return,
                };
                let detail: MbReleaseDetail = match serde_json::from_str(&body) {
                    Ok(d) => d,
                    Err(_) => return,
                };
                let albumartist = detail
                    .artist_credit
                    .map(|ac| {
                        ac.into_iter()
                            .map(|a| a.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let disc = detail
                    .media
                    .map(|m| {
                        m.iter()
                            .filter_map(|md| md.position)
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let genre = detail
                    .tags
                    .map(|t| {
                        t.into_iter()
                            .map(|tag| tag.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let sug = MbSuggestion {
                    source: MbSource::Recording,
                    title: String::new(),
                    artist: String::new(),
                    album: String::new(),
                    albumartist,
                    year: String::new(),
                    disc,
                    genre,
                    comment: String::new(),
                    release_id: String::new(),
                    release_status: String::new(),
                };
                let _ = tx.send(sug);
            });
        }
    }

    fn navigate_mb(&mut self, delta: isize) {
        if let MbState::Suggestions { results, index, cover_state } = &mut self.mb_state {
            let len = results.len();
            if len == 0 {
                return;
            }
            let new_index = if delta < 0 {
                index.saturating_sub(1)
            } else {
                (*index + 1).min(len.saturating_sub(1))
            };
            if new_index != *index {
                *index = new_index;
                *cover_state = CoverState::None;
                self.mb_cover_protocol = None;
                self.download_mb_cover(new_index);
                self.load_mb_detail(new_index);
            }
        }
    }

    fn open_mb_browser(&mut self) {
        let (title, artist, album) = match &self.mb_state {
            MbState::Suggestions { results, index, .. } => {
                if results.is_empty() {
                    return;
                }
                let s = &results[*index];
                (s.title.clone(), s.artist.clone(), s.album.clone())
            }
            _ => return,
        };
        let mut parts: Vec<String> = Vec::new();
        if !title.is_empty() {
            parts.push(title);
        }
        if !artist.is_empty() {
            parts.push(artist);
        }
        if !album.is_empty() {
            parts.push(album);
        }
        if parts.is_empty() {
            return;
        }
        let url = format!(
            "https://musicbrainz.org/search?query={}&type=release",
            url_encode(&parts.join(" ")),
        );
        match spawn_in_new_terminal(&url) {
            Ok(_) => {
                self.status_msg = format!("{} Abierto en el navegador", icon::GLOBE);
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_msg = format!("{} No se pudo abrir: {}", icon::CLOSE, e);
                self.status_is_error = true;
            }
        }
    }

    fn apply_mb_suggestion(&mut self) {
        if let MbState::Suggestions { results, index, .. } = &self.mb_state {
            if results.is_empty() {
                return;
            }
            let s = &results[*index];
            if self.edit_vals.len() < 9 {
                return;
            }
            if !s.title.is_empty() {
                self.edit_vals[0] = s.title.clone();
            }
            if !s.artist.is_empty() {
                self.edit_vals[1] = s.artist.clone();
            }
            if !s.album.is_empty() {
                self.edit_vals[2] = s.album.clone();
            }
            if !s.albumartist.is_empty() {
                self.edit_vals[3] = s.albumartist.clone();
            }
            if !s.year.is_empty() {
                self.edit_vals[4] = s.year.clone();
            }
            if !s.disc.is_empty() {
                self.edit_vals[6] = s.disc.clone();
            }
            if !s.genre.is_empty() {
                self.edit_vals[7] = s.genre.clone();
            }
            if !s.comment.is_empty() {
                self.edit_vals[8] = s.comment.clone();
            }
            self.status_msg = format!("{} Sugerencia aplicada", icon::CHECK);
            self.status_is_error = false;
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

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.status_msg = if self.show_help {
            self.l.help_shown()
        } else {
            self.l.help_hidden()
        };
        self.status_is_error = false;
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
        if !self.selected.is_empty() {
            let has_files = self.selected.iter().any(|&idx| idx >= self.dir_entries.len());
            if !has_files {
                return;
            }
        } else if self.current_idx < self.dir_entries.len() {
            return;
        }
        self.cover_path.clear();
        self.cover_cursor = 0;
        self.mode = AppMode::SetCoverArt;
    }

    fn apply_cover_art(&mut self) -> bool {
        let img_path = Path::new(&self.cover_path);
        if !img_path.exists() {
            self.status_msg = match self.l.lang {
                Lang::Es => format!("{} La imagen no existe", icon::CLOSE),
                Lang::En => format!("{} Image does not exist", icon::CLOSE),
            };
            self.status_is_error = true;
            return false;
        }

        let indices: Vec<usize> = if !self.selected.is_empty() {
            let mut v: Vec<usize> = self.selected.iter().copied().collect();
            v.sort();
            v
        } else {
            vec![self.current_idx]
        };

        let mut successes = 0u32;
        let mut errors = 0u32;
        let mut first_error = String::new();

        for &idx in &indices {
            let fidx = match idx.checked_sub(self.dir_entries.len()) {
                Some(f) => f,
                None => continue,
            };
            if fidx >= self.files.len() {
                continue;
            }
            let filepath = &self.files[fidx];
            match set_cover_art(filepath, img_path) {
                Ok(()) => successes += 1,
                Err(e) => {
                    errors += 1;
                    if first_error.is_empty() {
                        first_error = e;
                    }
                }
            }
        }

        if successes > 0 && errors == 0 {
            self.cover_protocol = None;
            self.cover_protocol_file = None;
            self.cover_cache.clear();
            if indices.len() > 1 {
                self.status_msg = match self.l.lang {
                    Lang::Es => format!("{} Portada asignada a {} archivos", icon::CHECK, successes),
                    Lang::En => format!("{} Cover art set for {} files", icon::CHECK, successes),
                };
                self.selected.clear();
            } else {
                let idx = indices[0];
                let fidx = idx - self.dir_entries.len();
                let name = self.files[fidx]
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                self.status_msg = match self.l.lang {
                    Lang::Es => format!("{} Portada asignada: {}", icon::CHECK, name),
                    Lang::En => format!("{} Cover art set: {}", icon::CHECK, name),
                };
            }
            self.status_is_error = false;
            true
        } else if errors > 0 {
            self.status_msg = match self.l.lang {
                Lang::Es => format!("{} {} errores: {}", icon::CLOSE, errors, first_error),
                Lang::En => format!("{} {} errors: {}", icon::CLOSE, errors, first_error),
            };
            self.status_is_error = true;
            false
        } else {
            self.status_msg = match self.l.lang {
                Lang::Es => format!("{} No hay archivos de audio seleccionados", icon::CLOSE),
                Lang::En => format!("{} No audio files selected", icon::CLOSE),
            };
            self.status_is_error = true;
            false
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

    fn content_area(&self) -> Rect {
        Rect {
            x: 0,
            y: 3,
            width: self.last_area.width,
            height: self.last_area.height.saturating_sub(5),
        }
    }

    fn border_for(&self, widget: &str) -> BorderStyle {
        self.border_overrides
            .get(widget)
            .copied()
            .unwrap_or(self.border_default)
    }

    fn block(&self, widget: &str, color: Color) -> Block<'static> {
        let style = self.border_for(widget);
        let mut b = Block::default()
            .borders(if style == BorderStyle::None {
                Borders::NONE
            } else {
                Borders::ALL
            })
            .border_style(Style::default().fg(color));
        if style != BorderStyle::None {
            b = b.border_type(style.border_type());
        }
        b
    }

    fn reload_config_if_changed(&mut self) {
        let Ok(content) = fs::read_to_string(config_path()) else {
            return;
        };
        if content == self.last_config_content {
            return;
        }
        self.last_config_content = content.clone();
        let cfg = load_config();
        self.colors = cfg.colors.clone();
        self.config_colors = cfg.colors;
        self.border_default = cfg.border;
        self.border_overrides = cfg.border_overrides;
        self.custom_keys = cfg.custom_keys;
        self.nav = cfg.nav;
        self.keymap = Keymap::new(self.nav, &self.custom_keys);
        self.show_preview = cfg.show_preview;
        if content != self.last_written_config {
            self.status_msg = self.l.config_reloaded();
            self.status_is_error = false;
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) {
        match self.mode {
            AppMode::Browse => self.handle_browse_mouse(m),
            AppMode::Edit => self.handle_edit_mouse(m),
            AppMode::Filter => self.handle_filter_mouse(m),
            AppMode::DeleteConfirm => self.handle_delete_mouse(m),
            AppMode::SetCoverArt => self.handle_cover_mouse(m),
        }
    }

    fn handle_browse_mouse(&mut self, m: MouseEvent) {
        let MouseEvent { kind, column, row, .. } = m;
        let content = self.content_area();
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if row >= content.y && row < content.y + content.height {
                    let now = std::time::Instant::now();
                    let is_double = match self.last_click {
                        Some((t, c, r)) => {
                            c == column
                                && r == row
                                && now.duration_since(t).as_millis() < 500
                        }
                        None => false,
                    };
                    self.last_click = Some((now, column, row));
                    if is_double {
                        self.open_item();
                        return;
                    }
                    let visible = (row - content.y) as usize;
                    let list_height = content.height as usize;
                    let mut adjusted = self.scroll_offset;
                    if self.current_idx >= adjusted + list_height {
                        adjusted = self.current_idx.saturating_sub(list_height - 1);
                    }
                    if self.current_idx < adjusted {
                        adjusted = self.current_idx;
                    }
                    let idx = adjusted + visible;
                    if idx < self.total_items() {
                        self.current_idx = idx;
                        self.update_cover_state();
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if row >= content.y && row < content.y + content.height {
                    let visible = (row - content.y) as usize;
                    let list_height = content.height as usize;
                    let mut adjusted = self.scroll_offset;
                    if self.current_idx >= adjusted + list_height {
                        adjusted = self.current_idx.saturating_sub(list_height - 1);
                    }
                    if self.current_idx < adjusted {
                        adjusted = self.current_idx;
                    }
                    let idx = adjusted + visible;
                    if idx < self.total_items() {
                        self.current_idx = idx;
                        self.update_cover_state();
                        self.toggle_select();
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if self.current_idx < self.total_items().saturating_sub(1) {
                    self.current_idx += 1;
                    self.update_cover_state();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.current_idx > 0 {
                    self.current_idx -= 1;
                    self.update_cover_state();
                }
            }
            _ => {}
        }
    }

    fn handle_edit_mouse(&mut self, m: MouseEvent) {
        let MouseEvent { kind, column, row, .. } = m;
        let field_count = self.get_fields().len();
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if row >= 3 && row < 3 + field_count as u16 {
                    let f = (row - 3) as usize;
                    if f < field_count {
                        self.edit_idx = f;
                        let val = &self.edit_vals[f];
                        let char_idx = column.saturating_sub(26) as usize;
                        let char_idx = char_idx.min(val.chars().count());
                        self.edit_cursor = val
                            .char_indices()
                            .nth(char_idx)
                            .map(|(i, _)| i)
                            .unwrap_or(val.len());
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if self.edit_idx < field_count - 1 {
                    self.edit_idx += 1;
                    self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.edit_idx > 0 {
                    self.edit_idx -= 1;
                    self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
                }
            }
            _ => {}
        }
    }

    fn handle_filter_mouse(&mut self, m: MouseEvent) {
        let MouseEvent { kind, column, row, .. } = m;
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let area = self.last_area;
                let popup_width = 50.min(area.width.saturating_sub(4));
                let popup_height = 3;
                let x = area.width.saturating_sub(popup_width) / 2;
                let y = area.height.saturating_sub(popup_height) / 2;
                let inner = Rect::new(x + 1, y + 1, popup_width.saturating_sub(2), 1);
                if row >= inner.y && row <= inner.y && column >= inner.x {
                    let char_idx = (column - inner.x).saturating_sub(2) as usize;
                    let char_idx = char_idx.min(self.filter_text.chars().count());
                    self.filter_cursor = self
                        .filter_text
                        .char_indices()
                        .nth(char_idx)
                        .map(|(i, _)| i)
                        .unwrap_or(self.filter_text.len());
                } else {
                    self.mode = AppMode::Browse;
                    self.status_msg = self.l.cancelled();
                    self.status_is_error = false;
                }
            }
            _ => {}
        }
    }

    fn handle_delete_mouse(&mut self, m: MouseEvent) {
        let MouseEvent { kind, column, row, .. } = m;
        if let MouseEventKind::Down(MouseButton::Left) = kind {
            let area = self.last_area;
            let count = self.delete_indices.len();
            let filename = if count == 1 {
                self.get_item(self.delete_indices[0])
                    .map(|(p, _)| {
                        p.file_name().unwrap_or_default().to_string_lossy().to_string()
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let msg = if count == 1 {
                self.l.confirm_delete_one(&filename)
            } else {
                self.l.confirm_delete_many(count)
            };
            let popup_width = (msg.len() as u16 + 6).min(area.width.saturating_sub(2));
            let popup_height = 3;
            let x = area.width.saturating_sub(popup_width) / 2;
            let y = area.height.saturating_sub(popup_height) / 2;
            let popup = Rect::new(x, y, popup_width, popup_height);
            if !(column >= popup.x
                && column < popup.x + popup.width
                && row >= popup.y
                && row < popup.y + popup.height)
            {
                self.mode = AppMode::Browse;
                self.status_msg = self.l.cancelled();
                self.status_is_error = false;
            }
        }
    }

    fn handle_cover_mouse(&mut self, m: MouseEvent) {
        let MouseEvent { kind, column, row, .. } = m;
        if let MouseEventKind::Down(MouseButton::Left) = kind {
            let area = self.last_area;
            let popup_width = 60.min(area.width.saturating_sub(4));
            let popup_height = 5;
            let x = area.width.saturating_sub(popup_width) / 2;
            let y = area.height.saturating_sub(popup_height) / 2;
            let popup = Rect::new(x, y, popup_width, popup_height);
            let inner = Rect::new(x + 1, y + 1, popup_width.saturating_sub(2), popup_height.saturating_sub(2));
            if column >= inner.x
                && column < inner.x + inner.width
                && row >= inner.y
                && row < inner.y + 1
            {
                let char_idx = (column - inner.x) as usize;
                let char_idx = char_idx.min(self.cover_char_count());
                self.cover_cursor = char_idx;
            } else if !(column >= popup.x
                && column < popup.x + popup.width
                && row >= popup.y
                && row < popup.y + popup.height)
            {
                self.mode = AppMode::Browse;
                self.status_msg = self.l.cancelled();
                self.status_is_error = false;
            }
        }
    }

    fn handle_browse_key(&mut self, key: KeyEvent) {
        let total = self.total_items();

        match self.keymap.action(&key, false) {
            Some(Action::Quit) => {
                self.should_quit = true;
            }
            Some(Action::ToggleNav) => {
                self.nav = match self.nav {
                    NavScheme::Vim => NavScheme::Arrows,
                    NavScheme::Arrows => NavScheme::Vim,
                };
                self.keymap = Keymap::new(self.nav, &self.custom_keys);
                if let Ok(c) = save_config(&self.current_dir, self.show_preview, self.nav) {
                    self.last_written_config = c;
                }
                self.status_msg = if self.nav == NavScheme::Vim {
                    self.l.nav_vim()
                } else {
                    self.l.nav_arrows()
                };
                self.status_is_error = false;
            }
            Some(Action::ToggleHelp) => self.toggle_help(),
            Some(Action::Down) => {
                if self.current_idx < total.saturating_sub(1) {
                    self.current_idx += 1;
                }
            }
            Some(Action::Up) => {
                if self.current_idx > 0 {
                    self.current_idx -= 1;
                }
            }
            Some(Action::Left) | Some(Action::Escape) => {
                self.go_parent();
            }
            Some(Action::Right) | Some(Action::Enter) => {
                self.open_item();
            }
            Some(Action::Home) => {
                self.current_idx = 0;
            }
            Some(Action::End) => {
                self.current_idx = total.saturating_sub(1);
            }
            Some(Action::ToggleSelect) => {
                self.toggle_select();
            }
            Some(Action::Filter) => {
                self.filter_text.clear();
                self.filter_cursor = 0;
                self.mode = AppMode::Filter;
            }
            Some(Action::Reload) => {
                self.load_dir();
                self.status_msg = self.l.status_reloaded();
                self.status_is_error = false;
            }
            Some(Action::ApplyAll) => {
                self.enter_edit_mode(true);
            }
            Some(Action::ApplySelected) => {
                self.select_all();
                let count = self.selected.len();
                self.status_msg = self.l.status_selected_all(count);
                self.status_is_error = false;
            }
            Some(Action::ClearSelection) => {
                self.selected.clear();
                self.status_msg = self.l.status_selection_cleared();
                self.status_is_error = false;
            }
            Some(Action::Delete) => {
                self.enter_delete_confirm();
            }
            Some(Action::SaveConfig) => {
                match save_config(&self.current_dir, self.show_preview, self.nav) {
                    Ok(content) => {
                        self.last_written_config = content;
                        self.status_msg = self.l.config_saved();
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_msg = self.l.config_error(&e);
                        self.status_is_error = true;
                    }
                }
            }
            Some(Action::SetCoverArt) => {
                self.enter_set_cover_art();
            }
            Some(Action::ResetColors) => {
                self.colors = self.config_colors.clone();
                self.status_msg = self.l.reload_colors();
                self.status_is_error = false;
            }
            Some(Action::TogglePreview) => {
                self.show_preview = !self.show_preview;
                self.status_msg = if self.show_preview {
                    self.l.preview_enabled()
                } else {
                    self.l.preview_disabled()
                };
                self.status_is_error = false;
            }
            Some(Action::ExtractCover) => {
                if self.current_idx < self.dir_entries.len() {
                    // directory, skip
                } else {
                    let fidx = self.current_idx - self.dir_entries.len();
                    if let Some(fp) = self.files.get(fidx) {
                        match extract_cover_from(fp) {
                            Ok(path) => {
                                self.status_msg = format!("{} Extraída: {}", icon::CHECK, path);
                                self.status_is_error = false;
                            }
                            Err(e) => {
                                self.status_msg = format!("{} {}", icon::CLOSE, e);
                                self.status_is_error = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        self.update_cover_state();
    }

    fn edit_byte_pos(&self) -> usize {
        let val = &self.edit_vals[self.edit_idx];
        val.char_indices()
            .nth(self.edit_cursor)
            .map(|(i, _)| i)
            .unwrap_or(val.len())
    }

    fn edit_char_count(&self) -> usize {
        self.edit_vals[self.edit_idx].chars().count()
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        let field_count = self.get_fields().len();

        match self.keymap.action(&key, true) {
            Some(Action::Escape) => {
                self.mode = AppMode::Browse;
                self.batch_mode = false;
                self.batch_indices.clear();
                self.status_msg.clear();
            }
            Some(Action::Enter) => {
                self.save_current();
                if self.batch_mode {
                    self.batch_mode = false;
                    self.batch_indices.clear();
                    self.selected.clear();
                }
                self.mode = AppMode::Browse;
            }
            Some(Action::NextField) => {
                self.edit_idx = (self.edit_idx + 1) % field_count;
                self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
            }
            Some(Action::PrevField) => {
                if self.edit_idx == 0 {
                    self.edit_idx = field_count - 1;
                } else {
                    self.edit_idx -= 1;
                }
                self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
            }
            Some(Action::SaveNext) => {
                self.save_and_next();
            }
            Some(Action::Down) => {
                if self.edit_idx < field_count - 1 {
                    self.edit_idx += 1;
                    self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
                }
            }
            Some(Action::Up) => {
                if self.edit_idx > 0 {
                    self.edit_idx -= 1;
                    self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
                }
            }
            Some(Action::Backspace) => {
                if self.edit_cursor > 0 {
                    self.edit_cursor -= 1;
                    let pos = self.edit_byte_pos();
                    self.edit_vals[self.edit_idx].remove(pos);
                }
            }
            Some(Action::DeleteChar) => {
                if self.edit_cursor < self.edit_char_count() {
                    let pos = self.edit_byte_pos();
                    self.edit_vals[self.edit_idx].remove(pos);
                }
            }
            Some(Action::Left) => {
                if self.edit_cursor > 0 {
                    self.edit_cursor -= 1;
                }
            }
            Some(Action::Right) => {
                if self.edit_cursor < self.edit_char_count() {
                    self.edit_cursor += 1;
                }
            }
            Some(Action::Home) => {
                self.edit_cursor = 0;
            }
            Some(Action::End) => {
                self.edit_cursor = self.edit_vals[self.edit_idx].chars().count();
            }
            Some(Action::ClearField) => {
                self.edit_vals[self.edit_idx].clear();
                self.edit_cursor = 0;
            }
            Some(Action::ApplyToAll) => {
                self.enter_edit_mode(true);
            }
            Some(Action::ApplyMb) => {
                self.apply_mb_suggestion();
            }
            Some(Action::MbPrev) => self.navigate_mb(-1),
            Some(Action::MbNext) => self.navigate_mb(1),
            Some(Action::MbBrowser) => {
                self.open_mb_browser();
            }
            _ => {
                if let KeyCode::Char(c) = key.code {
                    let pos = self.edit_byte_pos();
                    self.edit_vals[self.edit_idx].insert(pos, c);
                    self.edit_cursor += 1;
                }
            }
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        match self.keymap.action(&key, true) {
            Some(Action::Escape) => {
                self.mode = AppMode::Browse;
                self.filter_text.clear();
                self.load_dir();
            }
            Some(Action::Enter) => {
                self.mode = AppMode::Browse;
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            Some(Action::ClearField) => {
                self.filter_text.clear();
                self.filter_cursor = 0;
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            Some(Action::Backspace) => {
                if self.filter_cursor > 0 {
                    self.filter_cursor -= 1;
                    let pos = self.filter_text
                        .char_indices()
                        .nth(self.filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.filter_text.remove(pos);
                }
                self.load_dir();
                self.current_idx = 0;
                self.scroll_offset = 0;
            }
            Some(Action::Left) => {
                if self.filter_cursor > 0 {
                    self.filter_cursor -= 1;
                }
            }
            Some(Action::Right) => {
                if self.filter_cursor < self.filter_text.chars().count() {
                    self.filter_cursor += 1;
                }
            }
            Some(Action::Home) => {
                self.filter_cursor = 0;
            }
            Some(Action::End) => {
                self.filter_cursor = self.filter_text.chars().count();
            }
            _ => {
                if let KeyCode::Char(c) = key.code {
                    let pos = self.filter_text
                        .char_indices()
                        .nth(self.filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(self.filter_text.len());
                    self.filter_text.insert(pos, c);
                    self.filter_cursor += 1;
                    self.load_dir();
                    self.current_idx = 0;
                    self.scroll_offset = 0;
                }
            }
        }
    }

    fn handle_delete_confirm_key(&mut self, key: KeyEvent) {
        match self.keymap.action(&key, true) {
            Some(Action::Enter) | Some(Action::ConfirmYes) => {
                self.confirm_delete();
                self.mode = AppMode::Browse;
            }
            Some(Action::Escape) | Some(Action::ConfirmNo) => {
                self.mode = AppMode::Browse;
                self.status_msg = self.l.cancelled();
                self.status_is_error = false;
            }
            _ => {}
        }
    }

    fn cover_byte_idx(&self) -> usize {
        self.cover_path
            .char_indices()
            .nth(self.cover_cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.cover_path.len())
    }

    fn cover_char_count(&self) -> usize {
        self.cover_path.chars().count()
    }

    fn handle_cover_art_key(&mut self, key: KeyEvent) {
        match self.keymap.action(&key, true) {
            Some(Action::Escape) => {
                self.mode = AppMode::Browse;
                self.status_msg = self.l.cancelled();
                self.status_is_error = false;
            }
            Some(Action::Enter) => {
                if self.cover_path.is_empty() {
                    return;
                }
                if self.apply_cover_art() {
                    self.mode = AppMode::Browse;
                    self.update_cover_state();
                }
            }
            Some(Action::Backspace) => {
                if self.cover_cursor > 0 {
                    self.cover_cursor -= 1;
                    let byte_pos = self.cover_byte_idx();
                    self.cover_path.remove(byte_pos);
                }
            }
            Some(Action::DeleteChar) => {
                if self.cover_cursor < self.cover_char_count() {
                    let byte_pos = self.cover_byte_idx();
                    self.cover_path.remove(byte_pos);
                }
            }
            Some(Action::Left) => {
                if self.cover_cursor > 0 {
                    self.cover_cursor -= 1;
                }
            }
            Some(Action::Right) => {
                if self.cover_cursor < self.cover_char_count() {
                    self.cover_cursor += 1;
                }
            }
            Some(Action::Home) => {
                self.cover_cursor = 0;
            }
            Some(Action::End) => {
                self.cover_cursor = self.cover_char_count();
            }
            _ => {
                if let KeyCode::Char(c) = key.code {
                    let byte_pos = self.cover_byte_idx();
                    self.cover_path.insert(byte_pos, c);
                    self.cover_cursor += 1;
                }
            }
        }
    }
}

fn decode_cover_image(filepath: &Path) -> Option<image::DynamicImage> {
    let tagged_file = read_from_path(filepath).ok()?;
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
    let picture = tag.pictures().first()?;
    let data = picture.data();
    if data.is_empty() {
        return None;
    }
    image::load_from_memory(data).ok()
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

fn command_exists(name: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", name))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn spawn_in_new_terminal(url: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let inner = format!("xdg-open \"{}\"", url);
    let candidates: &[&[&str]] = &[
        &["foot", "--", "sh", "-c", inner.as_str()],
        &["kitty", "--", "sh", "-c", inner.as_str()],
        &["alacritty", "-e", "sh", "-c", inner.as_str()],
        &["wezterm", "start", "--", "sh", "-c", inner.as_str()],
        &["xterm", "-e", "sh", "-c", inner.as_str()],
    ];

    for args in candidates {
        if !command_exists(args[0]) {
            continue;
        }
        let mut c = Command::new(args[0]);
        for a in &args[1..] {
            c.arg(a);
        }
        if c
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }

    Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{}", e))
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn extract_cover_from(filepath: &Path) -> Result<String, String> {
    use std::fs;

    let tagged_file =
        read_from_path(filepath).map_err(|e| format!("Could not read: {}", e))?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
        .ok_or_else(|| "No tag".to_string())?;
    let picture = tag
        .pictures()
        .first()
        .ok_or_else(|| "Sin portada".to_string())?;
    let data = picture.data();
    if data.is_empty() {
        return Err("Portada vacía".to_string());
    }
    let fmt = image::guess_format(data).map_err(|e| format!("Formato: {}", e))?;
    let ext = fmt.extensions_str()[0];
    let stem = filepath.file_stem().unwrap_or_default();
    let out_dir = covers_dir();
    fs::create_dir_all(&out_dir).map_err(|e| format!("No se pudo crear la carpeta: {}", e))?;
    let out_path = out_dir.join(format!("{}.{}", stem.to_string_lossy(), ext));
    fs::write(&out_path, data).map_err(|e| format!("No se pudo escribir: {}", e))?;
    Ok(out_path.to_string_lossy().to_string())
}

fn query_musicbrainz(title: &str, artist: &str, album: &str) -> MbState {
    let enc = |s: &str| s.replace(' ', "%20").replace('&', "%26");

    let config = ureq::Agent::config_builder()
        .user_agent("musictag/0.1.0 ( jpablo@example.com )")
        .build();

    let mut results: Vec<MbSuggestion> = Vec::new();

    // 1) Search recordings by title + artist + album
    if !title.is_empty() {
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!("recording:{}", enc(title)));
        if !artist.is_empty() {
            parts.push(format!("artist:{}", enc(artist)));
        }
        if !album.is_empty() {
            parts.push(format!("release:{}", enc(album)));
        }
        let rec_url = format!(
            "https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit=10",
            parts.join("%20AND%20"),
        );
        let agent = ureq::Agent::new_with_config(config.clone());
        if let Ok(mut resp) = agent.get(&rec_url).call() {
            if let Ok(body) = resp.body_mut().read_to_string() {
                if let Ok(mb) = serde_json::from_str::<MbResponse>(&body) {
                    for rec in mb.recordings {
                        let artist_str = rec
                            .artist_credit
                            .into_iter()
                            .map(|a| a.name)
                            .collect::<Vec<_>>()
                            .join(", ");
                        let releases = rec.releases.unwrap_or_default();
                        if releases.is_empty() {
                            results.push(MbSuggestion {
                                source: MbSource::Recording,
                                title: rec.title,
                                artist: artist_str.clone(),
                                album: String::new(),
                                albumartist: String::new(),
                                year: String::new(),
                                disc: String::new(),
                                genre: String::new(),
                                comment: String::new(),
                                release_id: String::new(),
                                release_status: String::new(),
                            });
                        } else {
                            for rel in releases {
                                results.push(MbSuggestion {
                                    source: MbSource::Recording,
                                    title: rec.title.clone(),
                                    artist: artist_str.clone(),
                                    album: rel.title,
                                    albumartist: String::new(),
                                    year: rel.date.unwrap_or_default(),
                                    disc: String::new(),
                                    genre: String::new(),
                                    comment: String::new(),
                                    release_id: rel.id,
                                    release_status: rel.status.unwrap_or_default(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 2) Search releases by album (+ artist if available)
    if !album.is_empty() {
        let mut parts: Vec<String> = vec![format!("release:{}", enc(album))];
        if !artist.is_empty() {
            parts.push(format!("artist:{}", enc(artist)));
        }
        let rel_url = format!(
            "https://musicbrainz.org/ws/2/release/?query={}&fmt=json&limit=10",
            parts.join("%20AND%20"),
        );
        let agent = ureq::Agent::new_with_config(config);
        if let Ok(mut resp) = agent.get(&rel_url).call() {
            if let Ok(body) = resp.body_mut().read_to_string() {
                if let Ok(mb) = serde_json::from_str::<MbReleaseSearchResponse>(&body) {
                    for rel in mb.releases {
                        let artist_str = rel
                            .artist_credit
                            .map(|ac| {
                                ac.into_iter()
                                    .map(|a| a.name)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        results.push(MbSuggestion {
                            source: MbSource::Release,
                            title: String::new(),
                            artist: artist_str,
                            album: rel.title,
                            albumartist: String::new(),
                            year: rel.date.unwrap_or_default(),
                            disc: String::new(),
                            genre: String::new(),
                            comment: String::new(),
                            release_id: rel.id,
                            release_status: rel.status.unwrap_or_default(),
                        });
                    }
                }
            }
        }
    }

    if results.is_empty() {
        return MbState::Error("Sin resultados".into());
    }

    MbState::Suggestions {
        results,
        index: 0,
        cover_state: CoverState::None,
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    app.last_area = frame.area();
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

    if app.show_help {
        let help_para = Paragraph::new(app.l.help_browse(&app.keymap))
            .style(Style::default().fg(app.colors.help_text))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(help_para, chunks[1]);
    }

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

fn render_edit(frame: &mut Frame, app: &mut App, chunks: &[Rect]) {
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

    if app.show_help {
        let help_para = Paragraph::new(app.l.help_edit(&app.keymap))
            .style(Style::default().fg(app.colors.help_text))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(help_para, chunks[1]);
    }

    let field_count = app.get_fields().len();
    let form_widget = {
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
                format!("  {:<1} {:<22}", arrow, label),
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

        Paragraph::new(lines).wrap(Wrap { trim: false })
    };

    let form_line_count = field_count as u16 + 1;

    let edit_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[3]);

    // Left panel: local form + local cover
    let (left_top, left_bottom) = {
        let c = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(form_line_count), Constraint::Min(3)])
            .split(edit_chunks[0]);
        (c[0], c[1])
    };

    let left_block = Block::default()
        .borders(Borders::NONE);
    let left_area = left_block.inner(left_top);
    frame.render_widget(left_block, left_top);
    frame.render_widget(form_widget, left_area);

    // Local cover image in left bottom
    if left_bottom.height >= 2 && left_bottom.width >= 4 {
        if let Some(protocol) = &mut app.cover_protocol {
            frame.render_widget(Clear, left_bottom);
            let image: StatefulImage<StatefulProtocol> = StatefulImage::default().resize(Resize::Fit(None));
            frame.render_stateful_widget(image, left_bottom, protocol);
        }
    }

    // Right panel: MB metadata + MB cover
    let (right_top, right_bottom) = {
        let c = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(3)])
            .split(edit_chunks[1]);
        (c[0], c[1])
    };

    let mb_lines: Vec<Line> = match &app.mb_state {
        MbState::Idle | MbState::LoadingSuggestions => {
            vec![Line::from(Span::styled(
                "  Buscando...",
                Style::default().fg(app.colors.empty_value),
            ))]
        }
        MbState::Suggestions { results, index, cover_state } => {
            let mut v = Vec::new();
            let s = &results[*index];

            let source_label = match s.source {
                MbSource::Recording => "Tema",
                MbSource::Release => "Álbum",
            };
            let year_short: String = s.year.chars().take(4).collect();
            let tag = if !s.release_status.is_empty() && !year_short.is_empty() {
                format!("{} · {} · {}", source_label, s.release_status, year_short)
            } else if !s.release_status.is_empty() {
                format!("{} · {}", source_label, s.release_status)
            } else if !year_short.is_empty() {
                format!("{} · {}", source_label, year_short)
            } else {
                source_label.to_string()
            };
            let mb_label = |a: Action| {
                app.keymap
                    .keys_for(a, true)
                    .first()
                    .map(|k| k.label())
                    .unwrap_or_default()
            };
            let nav = format!(
                " {}/{}  [{}]  {}:aplicar",
                index + 1,
                results.len(),
                tag,
                mb_label(Action::ApplyMb)
            );
            v.push(Line::from(Span::styled(
                nav,
                Style::default().fg(app.colors.metadata_label).add_modifier(Modifier::BOLD),
            )));
            v.push(Line::from(""));

            let fields: Vec<(&str, &str)> = vec![
                ("  Título: ", &s.title),
                ("  Artista: ", &s.artist),
                ("  Álbum: ", &s.album),
                ("  Artista Álbum: ", &s.albumartist),
                ("  Año: ", &s.year),
                ("  Disco: ", &s.disc),
                ("  Género: ", &s.genre),
                ("  Comentario: ", &s.comment),
            ];

            for (label, val) in &fields {
                if !val.is_empty() {
                    v.push(Line::from(vec![
                        Span::styled(*label, Style::default().fg(app.colors.metadata_label).add_modifier(Modifier::BOLD)),
                        Span::styled(val.to_string(), Style::default().fg(app.colors.metadata_value)),
                    ]));
                } else {
                    v.push(Line::from(vec![
                        Span::styled(*label, Style::default().fg(app.colors.metadata_label).add_modifier(Modifier::BOLD)),
                        Span::styled("—", Style::default().fg(app.colors.empty_value)),
                    ]));
                }
            }

            match cover_state {
                CoverState::Loading => {
                    v.push(Line::from(""));
                    v.push(Line::from(Span::styled(
                        "  Descargando carátula...",
                        Style::default().fg(app.colors.empty_value),
                    )));
                }
                CoverState::Error(e) => {
                    v.push(Line::from(""));
                    v.push(Line::from(Span::styled(
                        format!("  {}", e),
                        Style::default().fg(app.colors.error),
                    )));
                    v.push(Line::from(Span::styled(
                        format!(
                            "  [{}] Buscar en MusicBrainz",
                            mb_label(Action::MbBrowser)
                        ),
                        Style::default().fg(app.colors.metadata_label),
                    )));
                }
                _ => {}
            }

            if v.len() <= 2 {
                v.push(Line::from(Span::styled(
                    "  Sin datos",
                    Style::default().fg(app.colors.empty_value),
                )));
            }

            v
        }
        MbState::Error(e) => {
            vec![Line::from(Span::styled(
                format!("  Error: {}", e),
                Style::default().fg(app.colors.error),
            ))]
        }
    };

    let mb_para = Paragraph::new(mb_lines).wrap(Wrap { trim: false });
    frame.render_widget(mb_para, right_top);

    // MB cover image in right bottom
    if right_bottom.height >= 2 && right_bottom.width >= 4 {
        if let MbState::Suggestions { cover_state, .. } = &app.mb_state {
            if matches!(cover_state, CoverState::Loaded(_)) {
                if let Some(protocol) = &mut app.mb_cover_protocol {
                    frame.render_widget(Clear, right_bottom);
                    let image: StatefulImage<StatefulProtocol> = StatefulImage::default().resize(Resize::Fit(None));
                    frame.render_stateful_widget(image, right_bottom, protocol);
                }
            }
        }
    }

    let status_style = Style::default()
        .fg(app.colors.info)
        .add_modifier(Modifier::DIM);
    let status = Paragraph::new(app.l.status_field(
        app.edit_idx + 1,
        field_count,
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
    let block = app
        .block("preview", app.colors.preview_border)
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

    let block = app
        .block("filter", app.colors.filter_border)
        .title(app.l.filter_title());

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

    let block = app
        .block("delete", app.colors.delete_border)
        .title(app.l.confirm_delete_title());

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

    let title = if app.selected.is_empty() {
        app.l.cover_title()
    } else {
        app.l.cover_batch_title(app.selected.len())
    };
    let hint = app.l.cover_hint();

    let popup_width = (60).min(area.width.saturating_sub(4));
    let popup_height = 5;
    let x = area.width.saturating_sub(popup_width) / 2;
    let y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let block = app.block("cover", app.colors.cover_border).title(title);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let input_width = inner.width as usize;
    let display_path = if app.cover_path.len() > input_width.saturating_sub(2) {
        let target = app.cover_path.len().saturating_sub(input_width.saturating_sub(2));
        let start = app.cover_path
            .char_indices()
            .map(|(i, _)| i)
            .filter(|&i| i >= target)
            .next()
            .unwrap_or(app.cover_path.len());
        format!("...{}", &app.cover_path[start..])
    } else {
        app.cover_path.clone()
    };

    let input_block = app.block("input", app.colors.input_border);

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

    let config = load_config();

    let start_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or(config.dir.clone())
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));

    let start_dir = if start_dir.exists() {
        start_dir.canonicalize()?
    } else {
        eprintln!("{}", l.err_invalid_dir(&start_dir));
        std::process::exit(1);
    };

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(start_dir, l, config);

    eprintln!("{}", app.l.lang_detected());

    app.picker = Some(Picker::from_query_stdio()?);
    app.update_cover_state();

    let mut last_status_msg = String::new();
    let mut last_status_change = std::time::Instant::now();

    loop {
        app.check_local_cover();
        app.check_mb_results();
        app.reload_config_if_changed();

        // expire status messages after a few seconds
        if app.status_msg != last_status_msg {
            last_status_msg = app.status_msg.clone();
            last_status_change = std::time::Instant::now();
        } else if !app.status_msg.is_empty()
            && last_status_change.elapsed().as_secs() >= 4
        {
            app.status_msg.clear();
            last_status_msg.clear();
        }

        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
