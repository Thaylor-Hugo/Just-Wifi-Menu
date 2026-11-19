use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

use relm4::gtk;
use relm4::gtk::gdk;
use serde::{Deserialize, Serialize};

use crate::i18n::Language;

pub const DEFAULT_CSS: &str = include_str!("default_style.css");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub language: Language,
    pub editor: String,
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
    pub primary_color: String,
    pub secondary_color: String,
    pub bg_color: String,
    pub text_color: String,
    pub border_radius: i32,
    pub spacing: i32,
    pub row_padding: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::Pt,
            editor: String::new(),
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
            primary_color: "#89b4fa".to_string(),
            secondary_color: "#313244".to_string(),
            bg_color: "#181825".to_string(),
            text_color: "#cdd6f4".to_string(),
            border_radius: 10,
            spacing: 10,
            row_padding: 6,
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
            tracing::warn!("Não foi possível criar {}: {e}", dir.display());
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
                tracing::warn!("Erro ao ler {}: {e}", config_path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Salva a configuração atual em `~/.config/just-wifi-menu/config.toml`.
    pub fn save(&self) {
        let Some((dir, config_path, _)) = paths() else {
            return;
        };
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("Não foi possível criar {}: {e}", dir.display());
            return;
        }
        match toml::to_string_pretty(self) {
            Ok(serialized) => {
                if let Err(e) = fs::write(&config_path, serialized) {
                    tracing::warn!("Erro ao salvar {}: {e}", config_path.display());
                }
            }
            Err(e) => tracing::warn!("Erro ao serializar config: {e}"),
        }
    }
}

/// Retorna `(diretorio, config.toml, style.css)`.
pub fn paths() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let dir = config_dir()?;
    let config_path = dir.join("config.toml");
    let style_path = dir.join("style.css");
    Some((dir, config_path, style_path))
}

thread_local! {
    static BASE_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static USER_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static DYNAMIC_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Aplica o CSS embutido, o `style.css` do usuário e os overrides visuais do `AppConfig`.
/// Pode ser chamada novamente para recarregar o tema em tempo real.
pub fn apply_css(config: &AppConfig) {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    BASE_PROVIDER.with(|slot| {
        if slot.borrow().is_none() {
            let base_provider = gtk::CssProvider::new();
            base_provider.load_from_string(DEFAULT_CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &base_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            *slot.borrow_mut() = Some(base_provider);
        }
    });

    USER_PROVIDER.with(|slot| {
        if let Some(old) = slot.borrow_mut().take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }

        if let Some((_, _, style_path)) = paths()
            && style_path.exists()
        {
            let user_provider = gtk::CssProvider::new();
            user_provider.load_from_path(&style_path);
            gtk::style_context_add_provider_for_display(
                &display,
                &user_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 50,
            );
            *slot.borrow_mut() = Some(user_provider);
        }
    });

    DYNAMIC_PROVIDER.with(|slot| {
        if let Some(old) = slot.borrow_mut().take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }

        let dynamic_css = build_dynamic_css(config);
        let dyn_provider = gtk::CssProvider::new();
        dyn_provider.load_from_string(&dynamic_css);
        gtk::style_context_add_provider_for_display(
            &display,
            &dyn_provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
        *slot.borrow_mut() = Some(dyn_provider);
    });
}

fn build_dynamic_css(config: &AppConfig) -> String {
    let radius = config.border_radius.clamp(0, 32);
    let shell_radius = (radius + 4).clamp(4, 36);
    let pad = config.row_padding.clamp(0, 24);
    let gap = config.spacing.clamp(2, 24);
    let primary = sanitize_color(&config.primary_color, "#89b4fa");
    let secondary = sanitize_color(&config.secondary_color, "#313244");
    let bg = sanitize_color(&config.bg_color, "#181825");
    let text = sanitize_color(&config.text_color, "#cdd6f4");

    format!(
        r#"
@define-color menu_bg alpha({bg}, 0.96);
@define-color menu_surface alpha({secondary}, 0.78);
@define-color menu_surface_hover alpha({secondary}, 0.98);
@define-color menu_connected alpha({primary}, 0.18);
@define-color menu_border alpha({primary}, 0.42);
@define-color menu_accent {primary};
@define-color menu_text {text};

.wifi-menu-shell {{
    border-radius: {shell_radius}px;
}}

.wifi-list > row {{
    margin-bottom: {gap}px;
}}

.wifi-row,
.wifi-about-card,
.wifi-password-box {{
    border-radius: {radius}px;
}}

.wifi-row {{
    padding: {pad}px 10px;
}}

.wifi-icon-btn,
.wifi-action-btn,
.wifi-scan-btn,
.wifi-connect-btn,
.wifi-forget-btn {{
    border-radius: {radius}px;
}}
"#
    )
}

fn sanitize_color<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.starts_with('#') && (trimmed.len() == 7 || trimmed.len() == 9 || trimmed.len() == 4) {
        trimmed
    } else {
        fallback
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_toml_uses_defaults_for_missing_fields() {
        let parsed: AppConfig = toml::from_str(
            r#"
            language = "en"
            window_width = 450
            close_on_connect = true
            anchor = "bottom-right"
            "#,
        )
        .expect("TOML parcial deve desserializar");

        assert_eq!(parsed.language, Language::En);
        assert_eq!(parsed.window_width, 450);
        assert_eq!(parsed.window_height, 500);
        assert!(parsed.close_on_connect);
        assert!(parsed.close_on_focus_loss);
        assert_eq!(parsed.anchor, "bottom-right");
        assert_eq!(parsed.margin_top, 8);
    }
}
