use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Pt,
    En,
}

pub struct Strings {
    pub add_hidden_tooltip: &'static str,
    pub scan_tooltip: &'static str,
    pub options_tooltip: &'static str,
    pub back_tooltip: &'static str,
    pub menu_saved_networks: &'static str,
    pub menu_settings: &'static str,
    pub menu_about: &'static str,
    pub connecting_nm: &'static str,
    pub wifi_off: &'static str,
    pub scanning: &'static str,
    pub turning_on: &'static str,
    pub turning_off: &'static str,
    pub disconnecting: &'static str,
    pub connection_cancelled: &'static str,
    pub connect: &'static str,
    pub disconnect: &'static str,
    pub cancel: &'static str,
    pub forget: &'static str,
    pub password_placeholder: &'static str,
    pub add_network_title: &'static str,
    pub ssid_label: &'static str,
    pub ssid_placeholder: &'static str,
    pub security_label: &'static str,
    pub sec_open: &'static str,
    pub sec_wpa: &'static str,
    pub sec_wpa3: &'static str,
    pub password_label: &'static str,
    pub hidden_checkbox: &'static str,
    pub saved_networks_title: &'static str,
    pub settings_title: &'static str,
    pub files_section: &'static str,
    pub open_config: &'static str,
    pub open_style: &'static str,
    pub reload_css: &'static str,
    pub css_reloaded: &'static str,
    pub appearance_section: &'static str,
    pub language_tooltip: &'static str,
    pub theme_presets_label: &'static str,
    pub primary_color_label: &'static str,
    pub secondary_color_label: &'static str,
    pub border_radius_label: &'static str,
    pub spacing_label: &'static str,
    pub row_padding_label: &'static str,
    pub position_label: &'static str,
    pub margin_top_label: &'static str,
    pub margin_right_label: &'static str,
    pub margin_bottom_label: &'static str,
    pub margin_left_label: &'static str,
    pub behavior_section: &'static str,
    pub close_on_connect_label: &'static str,
    pub close_on_focus_loss_label: &'static str,
    pub min_signal_label: &'static str,
    pub auto_scan_label: &'static str,
    pub about_title: &'static str,
    pub about_description: &'static str,
    pub about_github: &'static str,
    pub connected: &'static str,
    pub saved: &'static str,
    pub secured: &'static str,
    pub open: &'static str,
    pub show_password: &'static str,
    pub hide_password: &'static str,
    pub copy_ip_tooltip: &'static str,
    pub copy_password_tooltip: &'static str,
    pub ip_copied: &'static str,
    pub password_copied: &'static str,
    pub password_loading: &'static str,
    pub autoconnect: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub last_connected_label: &'static str,
    pub hidden_label: &'static str,
}

const PT: Strings = Strings {
    add_hidden_tooltip: "Adicionar rede oculta",
    scan_tooltip: "Escanear redes",
    options_tooltip: "Opções",
    back_tooltip: "Voltar",
    menu_saved_networks: "Redes salvas",
    menu_settings: "Personalização",
    menu_about: "Sobre",
    connecting_nm: "Conectando ao NetworkManager...",
    wifi_off: "Wi-Fi desligado",
    scanning: "Escaneando redes...",
    turning_on: "Ligando Wi-Fi...",
    turning_off: "Desligando Wi-Fi...",
    disconnecting: "Desconectando...",
    connection_cancelled: "Conexão cancelada",
    connect: "Conectar",
    disconnect: "Desconectar",
    cancel: "Cancelar",
    forget: "Esquecer",
    password_placeholder: "Digite a senha Wi-Fi...",
    add_network_title: "Adicionar rede",
    ssid_label: "Nome da rede (SSID)",
    ssid_placeholder: "Ex: MinhaRedeOculta",
    security_label: "Segurança",
    sec_open: "Aberta",
    sec_wpa: "WPA/WPA2",
    sec_wpa3: "WPA3",
    password_label: "Senha",
    hidden_checkbox: "Rede oculta (não transmite SSID)",
    saved_networks_title: "Redes salvas",
    settings_title: "Personalização",
    files_section: "Arquivos de configuração",
    open_config: "Abrir config.toml",
    open_style: "Abrir style.css",
    reload_css: "Recarregar CSS",
    css_reloaded: "CSS recarregado",
    appearance_section: "Aparência e posição",
    language_tooltip: "Alternar idioma (PT/EN)",
    theme_presets_label: "Paleta rápida",
    primary_color_label: "Cor principal",
    secondary_color_label: "Cor secundária",
    border_radius_label: "Raio das bordas",
    spacing_label: "Espaçamento",
    row_padding_label: "Padding das linhas",
    position_label: "Posição na tela",
    margin_top_label: "Margem topo",
    margin_right_label: "Margem direita",
    margin_bottom_label: "Margem base",
    margin_left_label: "Margem esquerda",
    behavior_section: "Comportamento",
    close_on_connect_label: "Fechar ao conectar",
    close_on_focus_loss_label: "Fechar ao clicar fora",
    min_signal_label: "Sinal mínimo (%)",
    auto_scan_label: "Auto-scan (seg)",
    about_title: "Sobre",
    about_description: "Menu Wi-Fi customizável para Hyprland/Wayland em Rust, GTK4, Relm4 e zbus",
    about_github: "Repositório no GitHub",
    connected: "Conectado",
    saved: "Salvo",
    secured: "Protegida",
    open: "Aberta",
    show_password: "Ver senha",
    hide_password: "Ocultar",
    copy_ip_tooltip: "Copiar IP",
    copy_password_tooltip: "Copiar senha",
    ip_copied: "IP copiado",
    password_copied: "Senha copiada",
    password_loading: "Senha: carregando...",
    autoconnect: "Auto-reconectar",
    yes: "Sim",
    no: "Não",
    last_connected_label: "Última conexão",
    hidden_label: "Rede oculta",
};

const EN: Strings = Strings {
    add_hidden_tooltip: "Add hidden network",
    scan_tooltip: "Scan networks",
    options_tooltip: "Options",
    back_tooltip: "Back",
    menu_saved_networks: "Saved networks",
    menu_settings: "Customization",
    menu_about: "About",
    connecting_nm: "Connecting to NetworkManager...",
    wifi_off: "Wi-Fi off",
    scanning: "Scanning networks...",
    turning_on: "Turning Wi-Fi on...",
    turning_off: "Turning Wi-Fi off...",
    disconnecting: "Disconnecting...",
    connection_cancelled: "Connection cancelled",
    connect: "Connect",
    disconnect: "Disconnect",
    cancel: "Cancel",
    forget: "Forget",
    password_placeholder: "Enter Wi-Fi password...",
    add_network_title: "Add network",
    ssid_label: "Network name (SSID)",
    ssid_placeholder: "e.g. MyHiddenNetwork",
    security_label: "Security",
    sec_open: "Open",
    sec_wpa: "WPA/WPA2",
    sec_wpa3: "WPA3",
    password_label: "Password",
    hidden_checkbox: "Hidden network (does not broadcast SSID)",
    saved_networks_title: "Saved networks",
    settings_title: "Customization",
    files_section: "Configuration files",
    open_config: "Open config.toml",
    open_style: "Open style.css",
    reload_css: "Reload CSS",
    css_reloaded: "CSS reloaded",
    appearance_section: "Appearance & position",
    language_tooltip: "Switch language (PT/EN)",
    theme_presets_label: "Quick palette",
    primary_color_label: "Primary color",
    secondary_color_label: "Secondary color",
    border_radius_label: "Border radius",
    spacing_label: "Spacing",
    row_padding_label: "Row padding",
    position_label: "Screen position",
    margin_top_label: "Top margin",
    margin_right_label: "Right margin",
    margin_bottom_label: "Bottom margin",
    margin_left_label: "Left margin",
    behavior_section: "Behavior",
    close_on_connect_label: "Close on connect",
    close_on_focus_loss_label: "Close when clicking outside",
    min_signal_label: "Minimum signal (%)",
    auto_scan_label: "Auto-scan (sec)",
    about_title: "About",
    about_description: "Customizable Wi-Fi menu for Hyprland/Wayland in Rust, GTK4, Relm4 and zbus",
    about_github: "GitHub Repository",
    connected: "Connected",
    saved: "Saved",
    secured: "Secured",
    open: "Open",
    show_password: "Show password",
    hide_password: "Hide",
    copy_ip_tooltip: "Copy IP",
    copy_password_tooltip: "Copy password",
    ip_copied: "IP copied",
    password_copied: "Password copied",
    password_loading: "Password: loading...",
    autoconnect: "Auto-connect",
    yes: "Yes",
    no: "No",
    last_connected_label: "Last connected",
    hidden_label: "Hidden network",
};

pub const fn strings(lang: Language) -> &'static Strings {
    match lang {
        Language::Pt => &PT,
        Language::En => &EN,
    }
}

pub fn networks_found(lang: Language, count: usize) -> String {
    match lang {
        Language::Pt => format!("{count} rede(s) encontrada(s)"),
        Language::En => format!("{count} network(s) found"),
    }
}

pub fn password_prompt_for(lang: Language, ssid: &str) -> String {
    match lang {
        Language::Pt => format!("Senha para \"{ssid}\":"),
        Language::En => format!("Password for \"{ssid}\":"),
    }
}

pub fn enter_password_for(lang: Language, ssid: &str) -> String {
    match lang {
        Language::Pt => format!("Digite a senha para \"{ssid}\""),
        Language::En => format!("Enter password for \"{ssid}\""),
    }
}

pub fn connecting_to(lang: Language, ssid: &str) -> String {
    match lang {
        Language::Pt => format!("Conectando em \"{ssid}\"..."),
        Language::En => format!("Connecting to \"{ssid}\"..."),
    }
}

pub fn removing_profile(lang: Language, ssid: &str) -> String {
    match lang {
        Language::Pt => format!("Removendo perfil \"{ssid}\"..."),
        Language::En => format!("Removing profile \"{ssid}\"..."),
    }
}

pub fn version_label(lang: Language, version: &str) -> String {
    match lang {
        Language::Pt => format!("Versão {version}"),
        Language::En => format!("Version {version}"),
    }
}

pub fn password_masked(lang: Language) -> &'static str {
    match lang {
        Language::Pt => "Senha: ••••••••",
        Language::En => "Password: ••••••••",
    }
}

pub fn password_revealed(lang: Language, pw: &str) -> String {
    match lang {
        Language::Pt => format!("Senha: {pw}"),
        Language::En => format!("Password: {pw}"),
    }
}

pub fn format_last_connected_i18n(lang: Language, timestamp: u64, now_secs: u64) -> String {
    if timestamp == 0 {
        return match lang {
            Language::Pt => "Nunca".to_string(),
            Language::En => "Never".to_string(),
        };
    }

    let diff = now_secs.saturating_sub(timestamp);
    match lang {
        Language::Pt => {
            if diff < 60 {
                "Agora mesmo".to_string()
            } else if diff < 3600 {
                format!("Há {} min", diff / 60)
            } else if diff < 86_400 {
                format!("Há {} h", diff / 3600)
            } else {
                format!("Há {} dia(s)", diff / 86_400)
            }
        }
        Language::En => {
            if diff < 60 {
                "Just now".to_string()
            } else if diff < 3600 {
                format!("{} min ago", diff / 60)
            } else if diff < 86_400 {
                format!("{} h ago", diff / 3600)
            } else {
                format!("{} day(s) ago", diff / 86_400)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_last_connected_supports_pt_and_en() {
        assert_eq!(format_last_connected_i18n(Language::Pt, 0, 10_000), "Nunca");
        assert_eq!(format_last_connected_i18n(Language::En, 0, 10_000), "Never");
        assert_eq!(
            format_last_connected_i18n(Language::En, 9_400, 10_000),
            "10 min ago"
        );
        assert_eq!(
            format_last_connected_i18n(Language::Pt, 100_000, 359_200),
            "Há 3 dia(s)"
        );
    }
}
