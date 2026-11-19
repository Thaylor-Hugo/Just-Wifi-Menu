use std::fs;
use std::path::PathBuf;

use relm4::gtk;
use relm4::gtk::gdk;
use serde::{Deserialize, Serialize};

pub const DEFAULT_CSS: &str = include_str!("default_style.css");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub window_width: i32,
    pub window_height: i32,
    pub close_on_connect: bool,
    pub close_on_focus_loss: bool,
    pub min_signal_strength: u8,
    pub anchor: String,
    pub margin_top: i32,
    pub margin_right: i32,
    pub margin_bottom: i32,
    pub margin_left: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window_width: 400,
            window_height: 500,
            close_on_connect: false,
            close_on_focus_loss: true,
            min_signal_strength: 0,
            anchor: "top-right".to_string(),
            margin_top: 8,
            margin_right: 8,
            margin_bottom: 8,
            margin_left: 8,
        }
    }
}

impl AppConfig {
    /// Carrega `~/.config/just-wifi-menu/config.toml`, criando os arquivos padrão se ainda não existirem.
    pub fn load() -> Self {
        let Some(dir) = config_dir() else {
            return Self::default();
        };

        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("Aviso: não foi possível criar {}: {e}", dir.display());
            return Self::default();
        }

        let config_path = dir.join("config.toml");
        if !config_path.exists() {
            let default_cfg = Self::default();
            if let Ok(serialized) = toml::to_string_pretty(&default_cfg) {
                let _ = fs::write(&config_path, serialized);
            }
        }

        let style_path = dir.join("style.css");
        if !style_path.exists() {
            let _ = fs::write(&style_path, DEFAULT_CSS);
        }

        match fs::read_to_string(&config_path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Aviso: erro ao ler {}: {e}", config_path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// Aplica o CSS embutido como base e sobrepõe com `~/.config/just-wifi-menu/style.css` se existir.
pub fn apply_css() {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let base_provider = gtk::CssProvider::new();
    base_provider.load_from_string(DEFAULT_CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &base_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    if let Some(style_path) = config_dir().map(|d| d.join("style.css"))
        && style_path.exists()
    {
        let user_provider = gtk::CssProvider::new();
        user_provider.load_from_path(&style_path);
        gtk::style_context_add_provider_for_display(
            &display,
            &user_provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}

fn config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("just-wifi-menu"));
    }

    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("just-wifi-menu"))
}
