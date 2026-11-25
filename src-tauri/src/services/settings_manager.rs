use super::interface_preferences::InterfacePreferences;
use super::notification_preferences::NotificationPreferences;
use crate::core::{AppError, CoreError, FsError, PathManager, emit};
use compact_str::CompactString;
use launchwerk::auth::MinecraftUser;
use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::OnceLock;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

// ── Static global ─────────────────────────────────────────────────────────────

static SETTINGS: LazyLock<RwLock<SettingsManager>> =
    LazyLock::new(|| RwLock::new(SettingsManager::load()));

static SAVE_TX: OnceLock<mpsc::UnboundedSender<()>> = OnceLock::new();
static SAVE_LOCK: Mutex<()> = Mutex::new(());

pub fn init_auto_save() {
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();
    SAVE_TX.set(tx).ok();
    tokio::spawn(async move {
        loop {
            if rx.recv().await.is_none() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            while rx.try_recv().is_ok() {}
            if let Err(e) = SettingsManager::save().await {
                warn!("Auto-save error: {}", e);
            }
        }
    });
}

// ── Defaults (serde) ──────────────────────────────────────────────────────────

fn default_min_mem() -> u32 {
    1024
}
fn default_max_mem() -> u32 {
    2048
}
fn default_lang() -> CompactString {
    CompactString::from("en")
}
fn default_true() -> bool {
    true
}
fn default_update_channel() -> CompactString {
    CompactString::from("stable")
}
fn default_theme() -> CompactString {
    CompactString::from("lima")
}
fn default_console_history_limit() -> u32 {
    3000
}
fn default_tile_slider_size() -> u32 {
    80
}
fn default_tile_slider_size_y() -> u32 {
    80
}
fn default_tile_slider_edge_inset() -> u32 {
    8
}
fn default_tile_slider_grid_gap() -> u32 {
    8
}
fn default_tile_slider_gap() -> u32 {
    8
}
fn default_tile_slider_height() -> u32 {
    0
}
fn default_tile_slider_speed() -> u32 {
    80
}
fn default_tile_slider_layout() -> String {
    "size".to_string()
}
fn default_tile_slider_rows() -> u32 {
    2
}
fn default_tile_slider_border() -> u32 {
    50
}
fn default_screenshot_interval() -> u32 {
    5
}
fn default_tile_style() -> String {
    "standard".to_string()
}
fn default_tile_side() -> String {
    "left".to_string()
}
fn default_tile_centered_blur() -> u32 {
    80
}
fn default_tile_text_pos() -> String {
    "auto".to_string()
}
fn default_active_user_idx() -> usize {
    0
}
/// Proveedor de JRE elegido por el usuario. `auto` usa la cadena por defecto.
fn default_jre_vendor() -> CompactString {
    CompactString::from("auto")
}
/// Proveedor de descargas de archivos de juego. `official` usa Mojang;
/// `bmclapi` usa el espejo para China con fallback al oficial.
fn default_download_provider() -> CompactString {
    CompactString::from("official")
}
fn default_user() -> Vec<MinecraftUser> {
    let vec = vec![MinecraftUser::cracked("Steve")];
    vec
}
// ── SettingsManager ───────────────────────────────────────────────────────────

/// Configuración persistida del launcher.
/// La memoria se almacena en MB.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SettingsManager {
    #[serde(default = "default_user")]
    pub user: Vec<MinecraftUser>,
    #[serde(default = "default_active_user_idx")]
    pub active_user_idx: usize,
    #[serde(default = "default_min_mem")]
    pub min_memory: u32,
    #[serde(default = "default_max_mem")]
    pub max_memory: u32,
    #[serde(default = "default_true")]
    pub use_recommended_ram: bool,
    #[serde(default)]
    pub jre8_path: PathBuf,
    #[serde(default = "default_true")]
    pub jre8_managed: bool,
    #[serde(default = "default_jre_vendor")]
    pub jre8_vendor: CompactString,
    #[serde(default)]
    pub jre17_path: PathBuf,
    #[serde(default = "default_true")]
    pub jre17_managed: bool,
    #[serde(default = "default_jre_vendor")]
    pub jre17_vendor: CompactString,
    #[serde(default)]
    pub jre21_path: PathBuf,
    #[serde(default = "default_true")]
    pub jre21_managed: bool,
    #[serde(default = "default_jre_vendor")]
    pub jre21_vendor: CompactString,
    #[serde(default)]
    pub jre25_path: PathBuf,
    #[serde(default = "default_true")]
    pub jre25_managed: bool,
    #[serde(default = "default_jre_vendor")]
    pub jre25_vendor: CompactString,
    #[serde(default = "default_lang")]
    pub language: CompactString,
    /// Check for launcher updates automatically and notify when one is available.
    #[serde(default)]
    pub auto_updates: bool,
    #[serde(default = "default_update_channel")]
    pub update_channel: CompactString,
    #[serde(default)]
    pub show_error_console: bool,
    #[serde(default = "default_true")]
    pub close_launcher_on_play: bool,
    /// Close the main WebView once Minecraft initializes; restore it on exit.
    #[serde(default)]
    pub hide_on_launch: bool,
    #[serde(default)]
    pub restore_last_instance: bool,
    #[serde(default)]
    pub open_console_on_launch: bool,
    #[serde(default = "default_console_history_limit")]
    pub console_history_limit: u32,
    #[serde(default = "default_true")]
    pub console_show_level_tags: bool,
    #[serde(default)]
    pub show_snapshots: bool,
    #[serde(default)]
    pub show_alpha: bool,
    #[serde(default)]
    pub show_unstable_loaders: bool,
    #[serde(default)]
    pub jvm_args: CompactString,
    #[serde(default)]
    pub env_vars: HashMap<CompactString, String>,
    #[serde(default = "default_theme")]
    pub theme: CompactString,
    /// Legacy flag, kept for old settings files. Discord RPC is disabled in the launcher.
    #[serde(default)]
    pub discord_presence: bool,
    #[serde(default = "default_true")]
    pub show_tutorial: bool,
    #[serde(default)]
    pub license_accepted: bool,
    #[serde(default)]
    pub market_filter_collapsed: bool,
    #[serde(default)]
    pub prominent_notifications: bool,
    // None distinguishes legacy settings from explicitly customized preferences.
    #[serde(default)]
    pub notification_preferences: Option<NotificationPreferences>,
    #[serde(default)]
    pub interface_preferences: InterfacePreferences,
    #[serde(default)]
    pub reduce_animations: bool,
    #[serde(default)]
    pub disable_blur_effects: bool,
    #[serde(default)]
    pub disable_infinite_animations: bool,
    #[serde(default)]
    pub disable_skin3d_animations: bool,
    #[serde(default)]
    pub reduce_log_animations: bool,
    #[serde(default)]
    pub disable_background_image: bool,
    #[serde(default)]
    pub reduce_skin3d_quality: bool,
    /// Directorio personalizado para las instancias. Vacío = ubicación por defecto.
    #[serde(default)]
    pub custom_instances_dir: PathBuf,
    /// Directorio personalizado para shared (versions/libraries/assets).
    /// Vacío = ubicación por defecto.
    #[serde(default)]
    pub custom_shared_dir: PathBuf,
    /// Proveedor de descargas: `official` (Mojang) o `bmclapi` (espejo China).
    #[serde(default = "default_download_provider")]
    pub download_provider: CompactString,
    /// Tamaño del campo de cintas de los mosaicos, en % (30–100; 80 = aspecto original).
    #[serde(default = "default_tile_slider_size")]
    pub tile_slider_size: u32,
    /// Escala vertical del área de cintas, % (30–100; 80 = original).
    #[serde(default = "default_tile_slider_size_y")]
    pub tile_slider_size_y: u32,
    /// Inset from the outer edges of animated tile sliders, in pixels.
    #[serde(default = "default_tile_slider_edge_inset")]
    pub tile_slider_edge_inset: u32,
    /// Velocidad de las cintas de los mosaicos, en % (10–200; 80 = velocidad original).
    #[serde(default = "default_tile_slider_speed")]
    pub tile_slider_speed: u32,
    /// Extensión vertical del campo de cintas (arriba y abajo), en % (0–100; 0 = sin extender).
    #[serde(default = "default_tile_slider_height")]
    pub tile_slider_height: u32,
    /// Режим высоты поля слайдеров: size | stretch | custom.
    #[serde(default = "default_tile_slider_layout")]
    pub tile_slider_layout: String,
    /// Количество рядов в режиме custom (1–10).
    #[serde(default = "default_tile_slider_rows")]
    pub tile_slider_rows: u32,
    /// Граница затухания краёв полосы, % (30–100; 50 = как раньше).
    #[serde(default = "default_tile_slider_border")]
    pub tile_slider_border: u32,
    /// Separación entre elementos de la cuadrícula estática, px.
    #[serde(default = "default_tile_slider_grid_gap")]
    pub tile_slider_grid_gap: u32,
    /// Separación entre elementos de la cinta, px.
    #[serde(default = "default_tile_slider_gap")]
    pub tile_slider_gap: u32,
    /// Ocultar etiquetas de los deslizadores animados.
    #[serde(default)]
    pub tile_slider_hide_text: bool,
    /// Segundos entre capturas en la galería (0 = desactivado).
    #[serde(default = "default_screenshot_interval")]
    pub screenshot_interval: u32,
    /// Стиль плиток: standard | stylish | glass.
    #[serde(default = "default_tile_style")]
    pub tile_style: String,
    /// Сторона текста плитки: left | right.
    #[serde(default = "default_tile_side")]
    pub tile_side: String,
    /// Размытие стеклянной подложки, % (0–80); legacy serialized field name.
    #[serde(default = "default_tile_centered_blur")]
    pub tile_centered_blur: u32,
    /// Положение текста в стиле «Стильный»: auto | bottom-left | bottom-center |
    /// bottom-right | top-left | top-center | top-right.
    #[serde(default = "default_tile_text_pos")]
    pub tile_text_pos: String,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    pub(crate) revision: u64,
}

// ── SettingsSnapshot ──────────────────────────────────────────────────────────

/// Snapshot liviano para el hot path (launch).
/// Solo contiene los campos necesarios para lanzar una instancia.
pub struct SettingsSnapshot {
    pub min_memory: u32,
    pub max_memory: u32,
    pub env_vars: HashMap<CompactString, String>,
    pub jvm_args: CompactString,
    pub jre8_managed: bool,
    pub jre17_managed: bool,
    pub jre21_managed: bool,
    pub jre25_managed: bool,
    pub jre8_path: Box<Path>,
    pub jre17_path: Box<Path>,
    pub jre21_path: Box<Path>,
    pub jre25_path: Box<Path>,
    pub open_console_on_launch: bool,
}

impl SettingsSnapshot {
    pub fn get_jre8_path(&self) -> &Path {
        &self.jre8_path
    }
    pub fn get_jre17_path(&self) -> &Path {
        &self.jre17_path
    }
    pub fn get_jre21_path(&self) -> &Path {
        &self.jre21_path
    }
    pub fn get_jre25_path(&self) -> &Path {
        &self.jre25_path
    }
}

impl Default for SettingsManager {
    fn default() -> Self {
        Self {
            user: {
                let vec = vec![MinecraftUser::cracked("Steve")];
                vec
            },
            active_user_idx: 0,
            min_memory: 1024,
            max_memory: 2048,
            use_recommended_ram: true,
            jre8_path: PathBuf::new(),
            jre8_managed: true,
            jre8_vendor: default_jre_vendor(),
            jre17_path: PathBuf::new(),
            jre17_managed: true,
            jre17_vendor: default_jre_vendor(),
            jre21_path: PathBuf::new(),
            jre21_managed: true,
            jre21_vendor: default_jre_vendor(),
            jre25_path: PathBuf::new(),
            jre25_managed: true,
            jre25_vendor: default_jre_vendor(),
            language: CompactString::from("en"),
            auto_updates: false,
            update_channel: CompactString::from("stable"),
            show_error_console: false,
            close_launcher_on_play: true,
            hide_on_launch: false,
            restore_last_instance: false,
            open_console_on_launch: false,
            console_history_limit: 3000,
            console_show_level_tags: true,
            show_snapshots: false,
            show_alpha: false,
            show_unstable_loaders: false,
            jvm_args: CompactString::default(),
            env_vars: HashMap::new(),
            theme: CompactString::from("lima"),
            discord_presence: false,
            show_tutorial: true,
            license_accepted: false,
            market_filter_collapsed: true,
            prominent_notifications: false,
            notification_preferences: Some(NotificationPreferences::default()),
            interface_preferences: InterfacePreferences::default(),
            reduce_animations: false,
            disable_blur_effects: false,
            disable_infinite_animations: false,
            disable_skin3d_animations: false,
            reduce_log_animations: false,
            disable_background_image: false,
            reduce_skin3d_quality: false,
            custom_instances_dir: PathBuf::new(),
            custom_shared_dir: PathBuf::new(),
            download_provider: default_download_provider(),
            tile_slider_size: default_tile_slider_size(),
            tile_slider_size_y: default_tile_slider_size_y(),
            tile_slider_edge_inset: default_tile_slider_edge_inset(),
            tile_slider_speed: default_tile_slider_speed(),
            tile_slider_height: default_tile_slider_height(),
            tile_slider_layout: default_tile_slider_layout(),
            tile_slider_rows: default_tile_slider_rows(),
            tile_slider_border: default_tile_slider_border(),
            tile_slider_grid_gap: default_tile_slider_grid_gap(),
            tile_slider_gap: default_tile_slider_gap(),
            tile_slider_hide_text: false,
            screenshot_interval: default_screenshot_interval(),
            tile_style: default_tile_style(),
            tile_side: default_tile_side(),
            tile_centered_blur: default_tile_centered_blur(),
            tile_text_pos: default_tile_text_pos(),
            dirty: true,
            revision: 0,
        }
    }
}

impl SettingsManager {
    pub fn read() -> RwLockReadGuard<'static, SettingsManager> {
        SETTINGS.read()
    }

    pub fn write(f: impl FnOnce(&mut SettingsManager)) -> Result<(), CoreError> {
        let mut settings = SETTINGS.write();
        let revision = settings.revision.wrapping_add(1);
        let previous_theme = settings.theme.clone();
        f(&mut settings);
        settings.normalize_offline_users();
        if settings.theme != previous_theme {
            // Follow write order, not the completion order of asynchronous saves.
            crate::theme_watcher::ThemeWatcher::watch(
                settings.theme.strip_prefix("user:").map(str::to_owned),
            );
        }
        settings.dirty = true;
        settings.revision = revision;
        if let Some(tx) = SAVE_TX.get() {
            let _ = tx.send(());
        }
        Ok(())
    }

    pub fn snapshot() -> SettingsManager {
        SETTINGS.read().clone()
    }

    pub fn launch_snapshot() -> SettingsSnapshot {
        let s = SETTINGS.read();
        let (min_memory, max_memory) = s.effective_memory();
        SettingsSnapshot {
            min_memory,
            max_memory,
            env_vars: s.env_vars.clone(),
            jvm_args: s.jvm_args.clone(),
            jre8_managed: s.jre8_managed,
            jre17_managed: s.jre17_managed,
            jre21_managed: s.jre21_managed,
            jre25_managed: s.jre25_managed,
            jre8_path: s.jre8_path.clone().into_boxed_path(),
            jre17_path: s.jre17_path.clone().into_boxed_path(),
            jre21_path: s.jre21_path.clone().into_boxed_path(),
            jre25_path: s.jre25_path.clone().into_boxed_path(),
            open_console_on_launch: s.open_console_on_launch,
        }
    }

    // ── Getters ───────────────────────────────────────────────────────────────

    pub fn get_min_memory(&self) -> u32 {
        self.effective_memory().0
    }
    pub fn get_max_memory(&self) -> u32 {
        self.effective_memory().1
    }

    fn recommended_ram_limits(total_gb: u32) -> (u32, u32) {
        // Reserve roughly a quarter for the OS, bounded to 3–6 GiB when possible.
        let reserve_gb = total_gb.div_ceil(4).clamp(3, 6);
        let max_gb = total_gb.saturating_sub(reserve_gb).max(1);
        // Keep the initial heap modest so Java can grow it as needed.
        let min_gb = (max_gb / 4).clamp(1, 4);
        (min_gb, max_gb)
    }

    /// Returns detected RAM, recommended minimum and recommended maximum in GiB.
    pub fn recommended_ram() -> (u32, u32, u32) {
        static RECOMMENDED_RAM: OnceLock<(u32, u32, u32)> = OnceLock::new();
        *RECOMMENDED_RAM.get_or_init(|| {
            let sys = System::new_with_specifics(
                RefreshKind::new().with_memory(MemoryRefreshKind::new().with_ram()),
            );
            let total_gb = (sys.total_memory() / 1024_u64.pow(3)).max(1) as u32;
            let (min_gb, max_gb) = Self::recommended_ram_limits(total_gb);
            info!("RAM detected: {total_gb} GB, min: {min_gb} GB, max: {max_gb} GB");
            (total_gb, min_gb, max_gb)
        })
    }

    fn effective_memory(&self) -> (u32, u32) {
        if self.use_recommended_ram {
            let (_, min_gb, max_gb) = Self::recommended_ram();
            (min_gb * 1024, max_gb * 1024)
        } else {
            (self.min_memory, self.max_memory)
        }
    }

    /// Proveedor de JRE configurado para una versión de Java. Devuelve `auto`
    /// para versiones sin preferencia explícita.
    pub fn get_jre_vendor(&self, version: u8) -> &str {
        match version {
            8 => &self.jre8_vendor,
            17 => &self.jre17_vendor,
            21 => &self.jre21_vendor,
            25 => &self.jre25_vendor,
            _ => "auto",
        }
    }
    pub fn add_user(&mut self, user: MinecraftUser) {
        self.user.push(user);
    }

    #[allow(dead_code)]
    pub fn rem_user(&mut self, user_name: &str) {
        let idx = self.user.iter().position(|u| u.username == user_name);
        self.user.retain(|s| s.username != user_name);
        if let Some(i) = idx {
            if self.user.is_empty() {
                self.user.push(MinecraftUser::cracked("Steve"));
                self.active_user_idx = 0;
            } else if i < self.active_user_idx || self.active_user_idx >= self.user.len() {
                self.active_user_idx = self.active_user_idx.saturating_sub(1);
            }
        }
    }

    pub fn rem_user_by_uuid(&mut self, uuid: &str) {
        let idx = self.user.iter().position(|u| u.uuid == uuid);
        self.user.retain(|u| u.uuid != uuid);
        if let Some(i) = idx {
            if self.user.is_empty() {
                self.user.push(MinecraftUser::cracked("Steve"));
                self.active_user_idx = 0;
            } else if i < self.active_user_idx || self.active_user_idx >= self.user.len() {
                self.active_user_idx = self.active_user_idx.saturating_sub(1);
            }
        }
    }

    pub fn get_user(&self) -> MinecraftUser {
        if self.active_user_idx < self.user.len() {
            return self.user[self.active_user_idx].clone();
        }
        warn!("tried to get the current user but it does not exist, returning default.");
        MinecraftUser::cracked("Steve")
    }
    /// Reemplaza el usuario activo por otro
    pub fn set_user(&mut self, user: MinecraftUser) {
        if self.active_user_idx < self.user.len() {
            self.user[self.active_user_idx] = user;
        } else {
            warn!("tried to replace a user but it does not exist.");
        }
    }

    /// Reemplaza un usuario por su UUID
    pub fn set_user_by_uuid(&mut self, user: &MinecraftUser) {
        if let Some(existing) = self.user.iter_mut().find(|u| u.uuid == user.uuid) {
            *existing = user.clone();
        } else {
            warn!(
                "tried to replace a user by UUID but it was not found: {}",
                user.uuid
            );
        }
    }
    // ── Persistencia ──────────────────────────────────────────────────────────

    /// Serializa y escribe a disco.
    /// Clona fuera del lock para minimizar la contención.
    pub async fn save() -> Result<(), AppError> {
        // Keep serialization and publication owned by the worker if the IPC
        // caller disappears. A newer save cannot overtake an older write.
        tokio::task::spawn_blocking(|| {
            let path = PathManager::get().get_settings_dir().join("settings.cub");
            if persist_settings(
                &SETTINGS,
                &SAVE_LOCK,
                &path,
                crate::core::atomic_file::write,
            )? {
                info!("Settings saved to {:?}", path);
                emit(crate::core::AppEvent::STChanged);
            }
            Ok(())
        })
        .await
        .map_err(|e| AppError::CoreError(CoreError::Other(e.to_string())))?
    }

    pub fn load() -> Self {
        let path = PathManager::get().get_settings_dir().join("settings.cub");

        if !path.exists() {
            info!("No settings file found, using default values");
            return Self::default();
        }

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to read settings from {:?}: {}", path, e);
                return Self::default();
            }
        };

        let reader = BufReader::new(file);
        match serde_json::from_reader::<_, Self>(reader) {
            Ok(mut settings) => {
                settings.migrate();
                info!("Settings loaded from {:?}", path);
                settings
            }
            Err(e) => {
                error!(
                    "Invalid settings in {:?} ({}), creating backup",
                    path, e
                );
                if let Err(e) = std::fs::copy(&path, path.with_extension("cub.bak")) {
                    warn!("Failed to create settings backup {:?}: {}", path, e);
                }
                Self::default()
            }
        }
    }

    /// Migraciones de versiones anteriores del formato.
    fn migrate(&mut self) {
        if self.language != "en" && self.language != "ru" {
            self.language = CompactString::from("en");
            self.dirty = true;
        }
        self.normalize_offline_users();
        self.normalize_notification_preferences();
        let previous_scale = self.interface_preferences.scale;
        let previous_font = self.interface_preferences.font.clone();
        self.interface_preferences.normalize();
        if self.interface_preferences.scale != previous_scale
            || self.interface_preferences.font != previous_font
        {
            self.dirty = true;
        }
        // v1 → v2: memoria en MB a GB
        if self.min_memory > 128 {
            self.min_memory = (self.min_memory / 1024).max(1);
            self.dirty = true;
        }
        if self.max_memory > 128 {
            self.max_memory = (self.max_memory / 1024).max(1);
            self.dirty = true;
        }
        // v2 → v3: GB a MB
        if self.min_memory <= 64 {
            self.min_memory *= 1024;
            self.dirty = true;
        }
        if self.max_memory <= 64 {
            self.max_memory *= 1024;
            self.dirty = true;
        }
        // Asegurar que el límite del historial de consola esté en rango válido.
        let sanitized = self.console_history_limit.clamp(100, 5000);
        if sanitized != self.console_history_limit {
            self.console_history_limit = sanitized;
            self.dirty = true;
        }
        self.tile_slider_size = self.tile_slider_size.clamp(30, 100);
        self.tile_slider_edge_inset = self.tile_slider_edge_inset.min(24);
        self.tile_slider_speed = self.tile_slider_speed.clamp(10, 200);
        self.tile_slider_height = self.tile_slider_height.clamp(0, 100);
        self.tile_slider_rows = self.tile_slider_rows.clamp(1, 10);
        self.tile_slider_border = self.tile_slider_border.clamp(30, 100);
        if !matches!(self.tile_slider_layout.as_str(), "size" | "stretch" | "custom") {
            self.tile_slider_layout = default_tile_slider_layout();
        }
        if !matches!(self.screenshot_interval, 0 | 3 | 5 | 10 | 30) {
            self.screenshot_interval = 5;
        }
        self.sanitize_tile_style();
    }

    /// Приводит параметры стиля плиток к допустимым значениям.
    pub(crate) fn sanitize_tile_style(&mut self) {
        if self.tile_style == "centered" {
            self.tile_style = "glass".to_string();
        } else if !matches!(self.tile_style.as_str(), "standard" | "stylish" | "glass") {
            self.tile_style = default_tile_style();
        }
        if !matches!(self.tile_side.as_str(), "" | "auto" | "left" | "right") {
            self.tile_side = String::new();
        }
        if !matches!(
            self.tile_text_pos.as_str(),
            "auto"
                | "bottom-left"
                | "bottom-center"
                | "bottom-right"
                | "top-left"
                | "top-center"
                | "top-right"
        ) {
            self.tile_text_pos = default_tile_text_pos();
        }
    }

    fn normalize_offline_users(&mut self) {
        for user in &mut self.user {
            if user.ensure_offline_uuid() {
                self.dirty = true;
            }
        }
    }

    pub(crate) fn normalize_notification_preferences(&mut self) {
        let mut preferences = self
            .notification_preferences
            .clone()
            .unwrap_or_else(|| NotificationPreferences::from_legacy(self.prominent_notifications));
        preferences.normalize();
        if self.notification_preferences.as_ref() != Some(&preferences) {
            self.notification_preferences = Some(preferences);
            self.dirty = true;
        }
    }
}

fn persist_settings(
    settings: &RwLock<SettingsManager>,
    save_lock: &Mutex<()>,
    path: &Path,
    write: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
) -> Result<bool, AppError> {
    let _save = save_lock.lock();
    let snapshot = {
        let settings = settings.read();
        if !settings.dirty {
            return Ok(false);
        }
        settings.clone()
    };
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::Other("Missing settings directory".into()))?;
    std::fs::create_dir_all(parent).map_err(|source| FsError::CreateDir {
        path: parent.to_string_lossy().into_owned(),
        source,
    })?;
    let bytes = serde_json::to_vec(&snapshot).map_err(|e| CoreError::Serialize(e.to_string()))?;
    write(path, &bytes).map_err(|source| FsError::WriteFile {
        path: path.to_string_lossy().into_owned(),
        source,
    })?;
    let mut current = settings.write();
    if current.revision == snapshot.revision {
        current.dirty = false;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "../tests/services/settings_manager.rs"]
mod tests;
