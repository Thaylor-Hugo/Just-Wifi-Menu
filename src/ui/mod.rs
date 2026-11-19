mod network_row;
mod saved_row;

use std::cell::Cell;
use std::rc::Rc;

use futures::StreamExt;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::gtk::gdk;
use relm4::gtk::gio;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tokio::sync::mpsc;

use crate::config::{self, AppConfig};
use crate::dbus;
use crate::dbus::proxies::{DeviceProxy, NetworkManagerProxy, WirelessDeviceProxy};
use crate::dbus::wifi::{
    NetworkDetails, SavedNetworkInfo, WifiManager, WifiNetwork, WifiSecurity,
};
use crate::i18n::{self, Language};
use network_row::{NetworkRow, NetworkRowInit, NetworkRowMsg, NetworkRowOutput};
use saved_row::{SavedRow, SavedRowInit, SavedRowMsg, SavedRowOutput};

#[derive(Debug)]
enum BackendCmd {
    Scan,
    ToggleWifi(bool),
    Connect {
        network: WifiNetwork,
        password: Option<String>,
    },
    ConnectHidden {
        ssid: String,
        security: WifiSecurity,
        password: Option<String>,
        hidden: bool,
    },
    Disconnect,
    Forget(String),
    FetchDetails(WifiNetwork),
    FetchPassword(String),
    SetAutoconnect {
        ssid: String,
        enabled: bool,
    },
    ListSaved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePage {
    Main,
    AddNetwork,
    SavedNetworks,
    Settings,
    About,
}

impl ActivePage {
    fn stack_name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::AddNetwork => "add-network",
            Self::SavedNetworks => "saved",
            Self::Settings => "settings",
            Self::About => "about",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreset {
    Catppuccin,
    TokyoNight,
    Nord,
    Gruvbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTarget {
    Primary,
    Secondary,
}

#[derive(Debug)]
pub enum AppMsg {
    Navigate(ActivePage),
    SetLanguage(Language),
    ApplyThemePreset(ThemePreset),
    SetPrimaryColor,
    SetSecondaryColor,
    ToggleColorPicker(ColorTarget),
    ColorPickerChanged,
    SetBorderRadius(i32),
    SetSpacing(i32),
    SetRowPadding(i32),
    SetAnchor(String),
    SetMarginTop(i32),
    SetMarginRight(i32),
    SetMarginBottom(i32),
    SetMarginLeft(i32),
    SetCloseOnConnect(bool),
    SetCloseOnFocusLoss(bool),
    SetMinSignal(u8),
    OpenConfigFile,
    OpenStyleFile,
    LaunchWithApp(usize),
    CloseAppChooser,
    ReloadCss,
    ToggleWifi(bool),
    Scan,
    SelectNetwork(WifiNetwork),
    ConnectWithPassword,
    CancelPassword,
    SetHiddenSecurity(WifiSecurity),
    SetHiddenFlag(bool),
    HiddenFormChanged,
    SubmitHiddenNetwork,
    EscapePressed,
    Disconnect,
    Forget(String),
    ToggleExpandRow(WifiNetwork),
    FetchPassword {
        ssid: String,
        visible: bool,
    },
    CopyPassword(String),
    SetAutoconnect {
        ssid: String,
        enabled: bool,
    },
    DetailsLoaded(NetworkDetails),
    PasswordLoaded {
        ssid: String,
        password: Option<String>,
    },
    ToggleExpandSaved(String),
    SavedNetworksLoaded(Vec<SavedNetworkInfo>),
    BackendState {
        interface: String,
        wifi_enabled: bool,
        networks: Vec<WifiNetwork>,
    },
    BackendStatus(String),
    BackendError(String),
    ScanFinished,
}

pub struct WifiApp {
    window: gtk::ApplicationWindow,
    config: AppConfig,
    close_on_focus_loss_flag: Rc<Cell<bool>>,
    active_page: ActivePage,
    interface: String,
    wifi_enabled: bool,
    scanning: bool,
    status_text: String,
    pending_network: Option<WifiNetwork>,
    expanded_ssid: Option<String>,
    expanded_details: Option<NetworkDetails>,
    expanded_password: Option<String>,
    expanded_password_visible: bool,
    pending_copy_password: bool,
    networks: FactoryVecDeque<NetworkRow>,
    saved_networks: FactoryVecDeque<SavedRow>,
    expanded_saved_ssid: Option<String>,
    password_entry: gtk::PasswordEntry,
    hidden_ssid_entry: gtk::Entry,
    hidden_password_entry: gtk::PasswordEntry,
    primary_color_entry: gtk::Entry,
    secondary_color_entry: gtk::Entry,
    #[allow(deprecated)]
    color_chooser: gtk::ColorChooserWidget,
    active_color_target: Option<ColorTarget>,
    app_chooser_list: gtk::ListBox,
    app_chooser_apps: Vec<gio::AppInfo>,
    app_chooser_path: Option<std::path::PathBuf>,
    hidden_security: WifiSecurity,
    hidden_is_hidden: bool,
    hidden_can_submit: bool,
    stack: gtk::Stack,
    menu_popover: gtk::Popover,
    menu_buttons: [gtk::Button; 3],
    cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    connect_requested: bool,
}

#[allow(deprecated)]
#[relm4::component(pub)]
impl SimpleComponent for WifiApp {
    type Init = AppConfig;
    type Input = AppMsg;
    type Output = ();

    view! {
        gtk::ApplicationWindow {
            set_title: Some("Just Wifi Menu"),
            set_default_width: model.config.window_width,
            set_default_height: model.config.window_height,
            add_css_class: "wifi-menu-window",

            #[local_ref]
            stack -> gtk::Stack {
                set_transition_type: gtk::StackTransitionType::SlideLeftRight,
                set_transition_duration: 180,

                // Página principal: lista de redes Wi-Fi
                add_named[Some("main")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    #[watch]
                    set_spacing: model.config.spacing,
                    set_margin_all: 12,
                    add_css_class: "wifi-menu-container",

                    // Cabeçalho: título + spinner + botão de scan + switch Wi-Fi + menu 3 pontinhos
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,
                        add_css_class: "wifi-header",

                        gtk::Label {
                            #[watch]
                            set_label: &if model.interface.is_empty() {
                                "Wi-Fi".to_string()
                            } else {
                                format!("Wi-Fi ({})", model.interface)
                            },
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-title",
                        },

                        gtk::Spinner {
                            #[watch]
                            set_spinning: model.scanning,
                            #[watch]
                            set_visible: model.scanning,
                        },

                        gtk::Button {
                            set_icon_name: "list-add-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).add_hidden_tooltip),
                            add_css_class: "wifi-action-btn",
                            #[watch]
                            set_sensitive: model.wifi_enabled,
                            connect_clicked => AppMsg::Navigate(ActivePage::AddNetwork),
                        },

                        gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).scan_tooltip),
                            add_css_class: "wifi-scan-btn",
                            #[watch]
                            set_sensitive: model.wifi_enabled && !model.scanning,
                            connect_clicked => AppMsg::Scan,
                        },

                        gtk::Switch {
                            #[watch]
                            set_active: model.wifi_enabled,
                            connect_state_set[sender] => move |_, enabled| {
                                sender.input(AppMsg::ToggleWifi(enabled));
                                gtk::glib::Propagation::Proceed
                            }
                        },

                        gtk::MenuButton {
                            set_icon_name: "view-more-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).options_tooltip),
                            add_css_class: "wifi-action-btn",
                            set_popover: Some(&model.menu_popover),
                        },
                    },

                    // Barra de status
                    gtk::Label {
                        #[watch]
                        set_label: &model.status_text,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        add_css_class: "wifi-status",
                    },

                    // Lista rolável gerenciada por FactoryVecDeque
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                        #[local_ref]
                        networks_box -> gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::None,
                            add_css_class: "wifi-list",
                        }
                    },

                    // Painel expansível para digitar senha
                    gtk::Revealer {
                        #[watch]
                        set_reveal_child: model.pending_network.is_some(),

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            add_css_class: "wifi-password-box",

                            gtk::Label {
                                #[watch]
                                set_label: &match &model.pending_network {
                                    Some(net) => i18n::password_prompt_for(model.config.language, &net.ssid),
                                    None => String::new(),
                                },
                                set_halign: gtk::Align::Start,
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,

                                #[local_ref]
                                password_entry -> gtk::PasswordEntry {
                                    set_hexpand: true,
                                    set_show_peek_icon: true,
                                    #[watch]
                                    set_placeholder_text: Some(i18n::strings(model.config.language).password_placeholder),
                                    connect_activate => AppMsg::ConnectWithPassword,
                                },

                                gtk::Button {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).connect,
                                    add_css_class: "wifi-connect-btn",
                                    connect_clicked => AppMsg::ConnectWithPassword,
                                },

                                gtk::Button {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).cancel,
                                    add_css_class: "wifi-action-btn",
                                    connect_clicked => AppMsg::CancelPassword,
                                },
                            }
                        }
                    }
                },

                // Página: Adicionar rede (oculta / manual)
                add_named[Some("add-network")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).back_tooltip),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).add_network_title,
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        add_css_class: "wifi-about-card",

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).ssid_label,
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                        },

                        #[local_ref]
                        hidden_ssid_entry -> gtk::Entry {
                            #[watch]
                            set_placeholder_text: Some(i18n::strings(model.config.language).ssid_placeholder),
                            connect_changed => AppMsg::HiddenFormChanged,
                            connect_activate => AppMsg::SubmitHiddenNetwork,
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).security_label,
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 6,
                            set_homogeneous: true,

                            gtk::ToggleButton {
                                #[watch]
                                set_label: i18n::strings(model.config.language).sec_open,
                                #[watch]
                                set_active: model.hidden_security == WifiSecurity::Open,
                                connect_clicked => AppMsg::SetHiddenSecurity(WifiSecurity::Open),
                            },

                            gtk::ToggleButton {
                                #[watch]
                                set_label: i18n::strings(model.config.language).sec_wpa,
                                #[watch]
                                set_active: model.hidden_security == WifiSecurity::WpaPsk,
                                connect_clicked => AppMsg::SetHiddenSecurity(WifiSecurity::WpaPsk),
                            },

                            gtk::ToggleButton {
                                #[watch]
                                set_label: i18n::strings(model.config.language).sec_wpa3,
                                #[watch]
                                set_active: model.hidden_security == WifiSecurity::Sae,
                                connect_clicked => AppMsg::SetHiddenSecurity(WifiSecurity::Sae),
                            },
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).password_label,
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                            #[watch]
                            set_visible: model.hidden_security != WifiSecurity::Open,
                        },

                        #[local_ref]
                        hidden_password_entry -> gtk::PasswordEntry {
                            set_show_peek_icon: true,
                            #[watch]
                            set_placeholder_text: Some(i18n::strings(model.config.language).password_placeholder),
                            #[watch]
                            set_visible: model.hidden_security != WifiSecurity::Open,
                            connect_changed => AppMsg::HiddenFormChanged,
                            connect_activate => AppMsg::SubmitHiddenNetwork,
                        },

                        gtk::CheckButton {
                            #[watch]
                            set_label: Some(i18n::strings(model.config.language).hidden_checkbox),
                            #[watch]
                            set_active: model.hidden_is_hidden,
                            connect_toggled[sender] => move |btn| {
                                sender.input(AppMsg::SetHiddenFlag(btn.is_active()));
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_halign: gtk::Align::End,

                            gtk::Button {
                                #[watch]
                                set_label: i18n::strings(model.config.language).cancel,
                                add_css_class: "wifi-action-btn",
                                connect_clicked => AppMsg::Navigate(ActivePage::Main),
                            },

                            gtk::Button {
                                #[watch]
                                set_label: i18n::strings(model.config.language).connect,
                                add_css_class: "wifi-connect-btn",
                                #[watch]
                                set_sensitive: model.hidden_can_submit,
                                connect_clicked => AppMsg::SubmitHiddenNetwork,
                            },
                        },
                    },
                },

                // Página: Redes Salvas
                add_named[Some("saved")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).back_tooltip),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).saved_networks_title,
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                        #[local_ref]
                        saved_box -> gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::None,
                            add_css_class: "wifi-list",
                        }
                    },
                },

                // Página: Personalização
                add_named[Some("settings")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).back_tooltip),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).settings_title,
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 10,

                            // Card 1: Arquivos de configuração
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 8,
                                add_css_class: "wifi-about-card",

                                gtk::Label {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).files_section,
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "wifi-ssid",
                                },

                                gtk::Label {
                                    set_label: &config_dir_display(),
                                    set_halign: gtk::Align::Start,
                                    set_ellipsize: gtk::pango::EllipsizeMode::Middle,
                                    add_css_class: "wifi-subtitle",
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 6,
                                    set_homogeneous: true,

                                    gtk::Button {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).open_config,
                                        add_css_class: "wifi-action-btn",
                                        connect_clicked => AppMsg::OpenConfigFile,
                                    },

                                    gtk::Button {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).open_style,
                                        add_css_class: "wifi-action-btn",
                                        connect_clicked => AppMsg::OpenStyleFile,
                                    },
                                },

                                gtk::Button {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).reload_css,
                                    add_css_class: "wifi-action-btn",
                                    connect_clicked => AppMsg::ReloadCss,
                                },

                                // Seletor inline de aplicativo compatível (sem diálogo externo)
                                gtk::Revealer {
                                    #[watch]
                                    set_reveal_child: model.app_chooser_path.is_some(),

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 6,
                                        add_css_class: "wifi-details-box",

                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Horizontal,
                                            set_spacing: 6,

                                            gtk::Label {
                                                #[watch]
                                                set_label: &match &model.app_chooser_path {
                                                    Some(p) => p
                                                        .file_name()
                                                        .and_then(|n| n.to_str())
                                                        .unwrap_or("Arquivo")
                                                        .to_string(),
                                                    None => String::new(),
                                                },
                                                set_hexpand: true,
                                                set_halign: gtk::Align::Start,
                                                add_css_class: "wifi-subtitle",
                                            },

                                            gtk::Button {
                                                set_icon_name: "window-close-symbolic",
                                                set_has_frame: false,
                                                connect_clicked => AppMsg::CloseAppChooser,
                                            },
                                        },

                                        #[local_ref]
                                        app_chooser_list -> gtk::ListBox {
                                            set_selection_mode: gtk::SelectionMode::None,
                                            add_css_class: "wifi-list",
                                        },
                                    },
                                },
                            },

                            // Card 2: Aparência e posição
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 8,
                                add_css_class: "wifi-about-card",

                                gtk::Label {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).appearance_section,
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "wifi-ssid",
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).language_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::ToggleButton {
                                        set_label: "PT",
                                        #[watch]
                                        set_active: model.config.language == Language::Pt,
                                        connect_clicked => AppMsg::SetLanguage(Language::Pt),
                                    },

                                    gtk::ToggleButton {
                                        set_label: "EN",
                                        #[watch]
                                        set_active: model.config.language == Language::En,
                                        connect_clicked => AppMsg::SetLanguage(Language::En),
                                    },
                                },

                                gtk::Label {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).theme_presets_label,
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "wifi-subtitle",
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 4,
                                    set_homogeneous: true,

                                    gtk::Button {
                                        set_label: "Catppuccin",
                                        add_css_class: "wifi-action-btn",
                                        connect_clicked => AppMsg::ApplyThemePreset(ThemePreset::Catppuccin),
                                    },
                                    gtk::Button {
                                        set_label: "Tokyo",
                                        add_css_class: "wifi-action-btn",
                                        connect_clicked => AppMsg::ApplyThemePreset(ThemePreset::TokyoNight),
                                    },
                                    gtk::Button {
                                        set_label: "Nord",
                                        add_css_class: "wifi-action-btn",
                                        connect_clicked => AppMsg::ApplyThemePreset(ThemePreset::Nord),
                                    },
                                    gtk::Button {
                                        set_label: "Gruvbox",
                                        add_css_class: "wifi-action-btn",
                                        connect_clicked => AppMsg::ApplyThemePreset(ThemePreset::Gruvbox),
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).primary_color_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::ToggleButton {
                                        set_icon_name: "color-select-symbolic",
                                        #[watch]
                                        set_active: model.active_color_target == Some(ColorTarget::Primary),
                                        connect_clicked => AppMsg::ToggleColorPicker(ColorTarget::Primary),
                                    },

                                    #[local_ref]
                                    primary_color_entry -> gtk::Entry {
                                        set_width_chars: 9,
                                        connect_changed => AppMsg::SetPrimaryColor,
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).secondary_color_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::ToggleButton {
                                        set_icon_name: "color-select-symbolic",
                                        #[watch]
                                        set_active: model.active_color_target == Some(ColorTarget::Secondary),
                                        connect_clicked => AppMsg::ToggleColorPicker(ColorTarget::Secondary),
                                    },

                                    #[local_ref]
                                    secondary_color_entry -> gtk::Entry {
                                        set_width_chars: 9,
                                        connect_changed => AppMsg::SetSecondaryColor,
                                    },
                                },

                                // Color picker inline (não abre janela separada no Hyprland)
                                gtk::Revealer {
                                    #[watch]
                                    set_reveal_child: model.active_color_target.is_some(),

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 4,
                                        add_css_class: "wifi-details-box",

                                        #[local_ref]
                                        color_chooser -> gtk::ColorChooserWidget {
                                            set_use_alpha: false,
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).border_radius_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(0.0, 24.0, 2.0) {
                                        set_value: model.config.border_radius as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetBorderRadius(spin.value_as_int()));
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).spacing_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(2.0, 24.0, 2.0) {
                                        set_value: model.config.spacing as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetSpacing(spin.value_as_int()));
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).row_padding_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(2.0, 20.0, 2.0) {
                                        set_value: model.config.row_padding as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetRowPadding(spin.value_as_int()));
                                        },
                                    },
                                },

                                gtk::Label {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).position_label,
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "wifi-subtitle",
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 4,
                                    set_homogeneous: true,

                                    gtk::ToggleButton {
                                        set_label: "↖",
                                        set_tooltip_text: Some("top-left"),
                                        #[watch]
                                        set_active: model.config.anchor == "top-left",
                                        connect_clicked => AppMsg::SetAnchor("top-left".to_string()),
                                    },
                                    gtk::ToggleButton {
                                        set_label: "↗",
                                        set_tooltip_text: Some("top-right"),
                                        #[watch]
                                        set_active: model.config.anchor == "top-right",
                                        connect_clicked => AppMsg::SetAnchor("top-right".to_string()),
                                    },
                                    gtk::ToggleButton {
                                        set_label: "•",
                                        set_tooltip_text: Some("center"),
                                        #[watch]
                                        set_active: model.config.anchor == "center",
                                        connect_clicked => AppMsg::SetAnchor("center".to_string()),
                                    },
                                    gtk::ToggleButton {
                                        set_label: "↙",
                                        set_tooltip_text: Some("bottom-left"),
                                        #[watch]
                                        set_active: model.config.anchor == "bottom-left",
                                        connect_clicked => AppMsg::SetAnchor("bottom-left".to_string()),
                                    },
                                    gtk::ToggleButton {
                                        set_label: "↘",
                                        set_tooltip_text: Some("bottom-right"),
                                        #[watch]
                                        set_active: model.config.anchor == "bottom-right",
                                        connect_clicked => AppMsg::SetAnchor("bottom-right".to_string()),
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).margin_top_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(0.0, 120.0, 2.0) {
                                        set_value: model.config.margin_top as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetMarginTop(spin.value_as_int()));
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).margin_right_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(0.0, 120.0, 2.0) {
                                        set_value: model.config.margin_right as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetMarginRight(spin.value_as_int()));
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).margin_bottom_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(0.0, 120.0, 2.0) {
                                        set_value: model.config.margin_bottom as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetMarginBottom(spin.value_as_int()));
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).margin_left_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(0.0, 120.0, 2.0) {
                                        set_value: model.config.margin_left as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetMarginLeft(spin.value_as_int()));
                                        },
                                    },
                                },
                            },

                            // Card 3: Comportamento
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 8,
                                add_css_class: "wifi-about-card",

                                gtk::Label {
                                    #[watch]
                                    set_label: i18n::strings(model.config.language).behavior_section,
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "wifi-ssid",
                                },

                                gtk::CheckButton {
                                    #[watch]
                                    set_label: Some(i18n::strings(model.config.language).close_on_connect_label),
                                    #[watch]
                                    set_active: model.config.close_on_connect,
                                    connect_toggled[sender] => move |btn| {
                                        sender.input(AppMsg::SetCloseOnConnect(btn.is_active()));
                                    },
                                },

                                gtk::CheckButton {
                                    #[watch]
                                    set_label: Some(i18n::strings(model.config.language).close_on_focus_loss_label),
                                    #[watch]
                                    set_active: model.config.close_on_focus_loss,
                                    connect_toggled[sender] => move |btn| {
                                        sender.input(AppMsg::SetCloseOnFocusLoss(btn.is_active()));
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: i18n::strings(model.config.language).min_signal_label,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "wifi-subtitle",
                                    },

                                    gtk::SpinButton::with_range(0.0, 90.0, 5.0) {
                                        set_value: model.config.min_signal_strength as f64,
                                        connect_value_changed[sender] => move |spin| {
                                            sender.input(AppMsg::SetMinSignal(spin.value_as_int().clamp(0, 100) as u8));
                                        },
                                    },
                                },
                            },
                        },
                    },
                },

                // Página: Sobre (About)
                add_named[Some("about")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(model.config.language).back_tooltip),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).about_title,
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        add_css_class: "wifi-about-card",

                        gtk::Image {
                            set_icon_name: Some("network-wireless-symbolic"),
                            set_pixel_size: 48,
                        },

                        gtk::Label {
                            set_label: env!("CARGO_PKG_NAME"),
                            add_css_class: "wifi-title",
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &i18n::version_label(model.config.language, env!("CARGO_PKG_VERSION")),
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::Label {
                            #[watch]
                            set_label: i18n::strings(model.config.language).about_description,
                            set_wrap: true,
                            set_justify: gtk::Justification::Center,
                            add_css_class: "wifi-status",
                        },

                        gtk::Label {
                            set_label: "Rust • GTK4 • Relm4 • zbus • Hyprland",
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::LinkButton {
                            set_uri: env!("CARGO_PKG_REPOSITORY"),
                            #[watch]
                            set_label: i18n::strings(model.config.language).about_github,
                            connect_activate_link => |_| {
                                if let Err(e) = std::process::Command::new("xdg-open")
                                    .arg(env!("CARGO_PKG_REPOSITORY"))
                                    .spawn()
                                {
                                    tracing::warn!("Falha ao abrir link com xdg-open: {e}");
                                }
                                gtk::glib::Propagation::Stop
                            },
                        },
                    },
                },
            }
        }
    }

    fn init(
        config: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let password_entry = gtk::PasswordEntry::new();
        let hidden_ssid_entry = gtk::Entry::new();
        let hidden_password_entry = gtk::PasswordEntry::new();
        let primary_color_entry = gtk::Entry::new();
        primary_color_entry.set_text(&config.primary_color);
        let secondary_color_entry = gtk::Entry::new();
        secondary_color_entry.set_text(&config.secondary_color);

        #[allow(deprecated)]
        let color_chooser = gtk::ColorChooserWidget::new();
        {
            let sender_clone = sender.clone();
            #[allow(deprecated)]
            color_chooser.connect_rgba_notify(move |_| {
                sender_clone.input(AppMsg::ColorPickerChanged);
            });
        }

        let app_chooser_list = gtk::ListBox::new();

        let stack = gtk::Stack::new();
        let (menu_popover, menu_buttons) = build_menu_popover(&sender, config.language);

        let close_on_focus_loss_flag = Rc::new(Cell::new(config.close_on_focus_loss));

        config::apply_css(&config);
        setup_layer_shell(&root, &config);
        setup_close_handlers(&root, &menu_popover, &close_on_focus_loss_flag, &sender);

        let networks = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                NetworkRowOutput::ToggleExpand(net) => AppMsg::ToggleExpandRow(net),
                NetworkRowOutput::Connect(net) => AppMsg::SelectNetwork(net),
                NetworkRowOutput::Disconnect => AppMsg::Disconnect,
                NetworkRowOutput::Forget(ssid) => AppMsg::Forget(ssid),
                NetworkRowOutput::FetchPassword { ssid, visible } => {
                    AppMsg::FetchPassword { ssid, visible }
                }
                NetworkRowOutput::CopyPassword(ssid) => AppMsg::CopyPassword(ssid),
                NetworkRowOutput::SetAutoconnect { ssid, enabled } => {
                    AppMsg::SetAutoconnect { ssid, enabled }
                }
                NetworkRowOutput::CopiedFeedback(msg) => AppMsg::BackendStatus(msg),
            });

        let saved_networks = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                SavedRowOutput::ToggleExpand(ssid) => AppMsg::ToggleExpandSaved(ssid),
                SavedRowOutput::SetAutoconnect { ssid, enabled } => {
                    AppMsg::SetAutoconnect { ssid, enabled }
                }
                SavedRowOutput::Forget(ssid) => AppMsg::Forget(ssid),
            });

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        spawn_dbus_worker(sender.clone(), cmd_rx);

        let initial_status = i18n::strings(config.language).connecting_nm.to_string();

        let model = Self {
            window: root.clone(),
            config,
            close_on_focus_loss_flag,
            active_page: ActivePage::Main,
            interface: String::new(),
            wifi_enabled: false,
            scanning: false,
            status_text: initial_status,
            pending_network: None,
            expanded_ssid: None,
            expanded_details: None,
            expanded_password: None,
            expanded_password_visible: false,
            pending_copy_password: false,
            networks,
            saved_networks,
            expanded_saved_ssid: None,
            password_entry: password_entry.clone(),
            hidden_ssid_entry: hidden_ssid_entry.clone(),
            hidden_password_entry: hidden_password_entry.clone(),
            primary_color_entry: primary_color_entry.clone(),
            secondary_color_entry: secondary_color_entry.clone(),
            color_chooser: color_chooser.clone(),
            active_color_target: None,
            app_chooser_list: app_chooser_list.clone(),
            app_chooser_apps: Vec::new(),
            app_chooser_path: None,
            hidden_security: WifiSecurity::WpaPsk,
            hidden_is_hidden: true,
            hidden_can_submit: false,
            stack: stack.clone(),
            menu_popover,
            menu_buttons,
            cmd_tx,
            connect_requested: false,
        };

        let stack = &model.stack;
        let networks_box = model.networks.widget();
        let saved_box = model.saved_networks.widget();
        let password_entry = &model.password_entry;
        let hidden_ssid_entry = &model.hidden_ssid_entry;
        let hidden_password_entry = &model.hidden_password_entry;
        let primary_color_entry = &model.primary_color_entry;
        let secondary_color_entry = &model.secondary_color_entry;
        let color_chooser = &model.color_chooser;
        let app_chooser_list = &model.app_chooser_list;
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Navigate(page) => {
                self.menu_popover.popdown();
                self.active_page = page;
                self.stack.set_visible_child_name(page.stack_name());
                if page == ActivePage::SavedNetworks {
                    let _ = self.cmd_tx.send(BackendCmd::ListSaved);
                }
            }
            AppMsg::SetLanguage(lang) => {
                if self.config.language == lang {
                    return;
                }
                self.config.language = lang;
                self.config.save();
                let s = i18n::strings(lang);
                self.menu_buttons[0].set_label(s.menu_saved_networks);
                self.menu_buttons[1].set_label(s.menu_settings);
                self.menu_buttons[2].set_label(s.menu_about);

                let guard = self.networks.guard();
                for idx in 0..guard.len() {
                    guard.send(idx, NetworkRowMsg::SetLanguage(lang));
                }
                let saved_guard = self.saved_networks.guard();
                for idx in 0..saved_guard.len() {
                    saved_guard.send(idx, SavedRowMsg::SetLanguage(lang));
                }
            }
            AppMsg::SetAnchor(anchor) => {
                if self.config.anchor == anchor {
                    return;
                }
                self.config.anchor = anchor;
                self.config.save();
                apply_layer_shell_position(&self.window, &self.config);
            }
            AppMsg::SetMarginTop(val) => {
                if self.config.margin_top == val {
                    return;
                }
                self.config.margin_top = val;
                self.config.save();
                apply_layer_shell_position(&self.window, &self.config);
            }
            AppMsg::SetMarginRight(val) => {
                if self.config.margin_right == val {
                    return;
                }
                self.config.margin_right = val;
                self.config.save();
                apply_layer_shell_position(&self.window, &self.config);
            }
            AppMsg::ApplyThemePreset(preset) => {
                let (primary, secondary, bg, text) = match preset {
                    ThemePreset::Catppuccin => ("#89b4fa", "#313244", "#181825", "#cdd6f4"),
                    ThemePreset::TokyoNight => ("#7aa2f7", "#24283b", "#1a1b26", "#c0caf5"),
                    ThemePreset::Nord => ("#88c0d0", "#3b4252", "#2e3440", "#eceff4"),
                    ThemePreset::Gruvbox => ("#fabd2f", "#3c3836", "#282828", "#ebdbb2"),
                };
                self.config.primary_color = primary.to_string();
                self.config.secondary_color = secondary.to_string();
                self.config.bg_color = bg.to_string();
                self.config.text_color = text.to_string();
                self.primary_color_entry.set_text(primary);
                self.secondary_color_entry.set_text(secondary);
                if let Some(target) = self.active_color_target {
                    let hex = match target {
                        ColorTarget::Primary => primary,
                        ColorTarget::Secondary => secondary,
                    };
                    if let Ok(rgba) = gdk::RGBA::parse(hex) {
                        #[allow(deprecated)]
                        self.color_chooser.set_rgba(&rgba);
                    }
                }
                self.config.save();
                config::apply_css(&self.config);
            }
            AppMsg::SetPrimaryColor => {
                let color = self.primary_color_entry.text().trim().to_string();
                if self.config.primary_color == color {
                    return;
                }
                self.config.primary_color = color.clone();
                if self.active_color_target == Some(ColorTarget::Primary)
                    && let Ok(rgba) = gdk::RGBA::parse(&color)
                {
                    #[allow(deprecated)]
                    self.color_chooser.set_rgba(&rgba);
                }
                self.config.save();
                config::apply_css(&self.config);
            }
            AppMsg::SetSecondaryColor => {
                let color = self.secondary_color_entry.text().trim().to_string();
                if self.config.secondary_color == color {
                    return;
                }
                self.config.secondary_color = color.clone();
                if self.active_color_target == Some(ColorTarget::Secondary)
                    && let Ok(rgba) = gdk::RGBA::parse(&color)
                {
                    #[allow(deprecated)]
                    self.color_chooser.set_rgba(&rgba);
                }
                self.config.save();
                config::apply_css(&self.config);
            }
            AppMsg::ToggleColorPicker(target) => {
                if self.active_color_target == Some(target) {
                    self.active_color_target = None;
                    return;
                }
                self.active_color_target = Some(target);
                let current = match target {
                    ColorTarget::Primary => &self.config.primary_color,
                    ColorTarget::Secondary => &self.config.secondary_color,
                };
                if let Ok(rgba) = gdk::RGBA::parse(current) {
                    #[allow(deprecated)]
                    self.color_chooser.set_rgba(&rgba);
                }
            }
            AppMsg::ColorPickerChanged => {
                let Some(target) = self.active_color_target else {
                    return;
                };
                #[allow(deprecated)]
                let hex = rgba_to_hex(&self.color_chooser.rgba());
                match target {
                    ColorTarget::Primary => {
                        if self.config.primary_color == hex {
                            return;
                        }
                        self.config.primary_color = hex.clone();
                        self.primary_color_entry.set_text(&hex);
                    }
                    ColorTarget::Secondary => {
                        if self.config.secondary_color == hex {
                            return;
                        }
                        self.config.secondary_color = hex.clone();
                        self.secondary_color_entry.set_text(&hex);
                    }
                }
                self.config.save();
                config::apply_css(&self.config);
            }
            AppMsg::SetBorderRadius(val) => {
                if self.config.border_radius == val {
                    return;
                }
                self.config.border_radius = val;
                self.config.save();
                config::apply_css(&self.config);
            }
            AppMsg::SetSpacing(val) => {
                if self.config.spacing == val {
                    return;
                }
                self.config.spacing = val;
                self.config.save();
            }
            AppMsg::SetRowPadding(val) => {
                if self.config.row_padding == val {
                    return;
                }
                self.config.row_padding = val;
                self.config.save();
                config::apply_css(&self.config);
            }
            AppMsg::SetMarginBottom(val) => {
                if self.config.margin_bottom == val {
                    return;
                }
                self.config.margin_bottom = val;
                self.config.save();
                apply_layer_shell_position(&self.window, &self.config);
            }
            AppMsg::SetMarginLeft(val) => {
                if self.config.margin_left == val {
                    return;
                }
                self.config.margin_left = val;
                self.config.save();
                apply_layer_shell_position(&self.window, &self.config);
            }
            AppMsg::SetCloseOnConnect(enabled) => {
                if self.config.close_on_connect == enabled {
                    return;
                }
                self.config.close_on_connect = enabled;
                self.config.save();
            }
            AppMsg::SetCloseOnFocusLoss(enabled) => {
                if self.config.close_on_focus_loss == enabled {
                    return;
                }
                self.config.close_on_focus_loss = enabled;
                self.close_on_focus_loss_flag.set(enabled);
                self.config.save();
            }
            AppMsg::SetMinSignal(val) => {
                if self.config.min_signal_strength == val {
                    return;
                }
                self.config.min_signal_strength = val;
                self.config.save();
            }
            AppMsg::OpenConfigFile => {
                if let Some((_, config_path, _)) = config::paths() {
                    self.open_app_chooser(config_path, &sender);
                }
            }
            AppMsg::OpenStyleFile => {
                if let Some((_, _, style_path)) = config::paths() {
                    self.open_app_chooser(style_path, &sender);
                }
            }
            AppMsg::LaunchWithApp(idx) => {
                if let (Some(app), Some(path)) =
                    (self.app_chooser_apps.get(idx), self.app_chooser_path.as_ref())
                {
                    let file = gio::File::for_path(path);
                    if let Err(e) = app.launch(&[file], gio::AppLaunchContext::NONE) {
                        tracing::warn!("Falha ao abrir com {}: {e}", app.display_name());
                    }
                }
                self.app_chooser_path = None;
                self.app_chooser_apps.clear();
            }
            AppMsg::CloseAppChooser => {
                self.app_chooser_path = None;
                self.app_chooser_apps.clear();
            }
            AppMsg::ReloadCss => {
                config::apply_css(&self.config);
                self.status_text = i18n::strings(self.config.language).css_reloaded.to_string();
            }
            AppMsg::ToggleWifi(enabled) => {
                if self.wifi_enabled == enabled {
                    return;
                }
                self.wifi_enabled = enabled;
                self.pending_network = None;
                let s = i18n::strings(self.config.language);
                self.status_text = if enabled {
                    s.turning_on.to_string()
                } else {
                    s.turning_off.to_string()
                };
                let _ = self.cmd_tx.send(BackendCmd::ToggleWifi(enabled));
            }
            AppMsg::Scan => {
                self.scanning = true;
                self.status_text = i18n::strings(self.config.language).scanning.to_string();
                let _ = self.cmd_tx.send(BackendCmd::Scan);
            }
            AppMsg::SelectNetwork(net) => {
                if net.secured && !net.saved {
                    self.password_entry.set_text("");
                    self.status_text = i18n::enter_password_for(self.config.language, &net.ssid);
                    self.pending_network = Some(net);
                } else {
                    self.connect_requested = true;
                    self.status_text = i18n::connecting_to(self.config.language, &net.ssid);
                    let _ = self.cmd_tx.send(BackendCmd::Connect {
                        network: net,
                        password: None,
                    });
                }
            }
            AppMsg::ConnectWithPassword => {
                let Some(net) = self.pending_network.take() else {
                    return;
                };
                let password = self.password_entry.text().to_string();
                self.password_entry.set_text("");
                self.connect_requested = true;
                self.status_text = i18n::connecting_to(self.config.language, &net.ssid);
                let _ = self.cmd_tx.send(BackendCmd::Connect {
                    network: net,
                    password: Some(password),
                });
            }
            AppMsg::CancelPassword => {
                self.pending_network = None;
                self.password_entry.set_text("");
                self.status_text = i18n::strings(self.config.language)
                    .connection_cancelled
                    .to_string();
            }
            AppMsg::SetHiddenSecurity(sec) => {
                self.hidden_security = sec;
                self.refresh_hidden_can_submit();
            }
            AppMsg::SetHiddenFlag(hidden) => {
                self.hidden_is_hidden = hidden;
            }
            AppMsg::HiddenFormChanged => {
                self.refresh_hidden_can_submit();
            }
            AppMsg::SubmitHiddenNetwork => {
                if !self.hidden_can_submit {
                    return;
                }
                let ssid = self.hidden_ssid_entry.text().trim().to_string();
                let password = if self.hidden_security == WifiSecurity::Open {
                    None
                } else {
                    Some(self.hidden_password_entry.text().to_string())
                };
                let security = self.hidden_security;
                let hidden = self.hidden_is_hidden;

                self.hidden_ssid_entry.set_text("");
                self.hidden_password_entry.set_text("");
                self.hidden_can_submit = false;
                self.connect_requested = true;
                self.active_page = ActivePage::Main;
                self.stack.set_visible_child_name(ActivePage::Main.stack_name());
                self.status_text = i18n::connecting_to(self.config.language, &ssid);

                let _ = self.cmd_tx.send(BackendCmd::ConnectHidden {
                    ssid,
                    security,
                    password,
                    hidden,
                });
            }
            AppMsg::EscapePressed => {
                if self.active_page != ActivePage::Main {
                    self.active_page = ActivePage::Main;
                    self.stack.set_visible_child_name(ActivePage::Main.stack_name());
                } else if self.pending_network.is_some() {
                    self.pending_network = None;
                    self.password_entry.set_text("");
                    self.status_text = "Conexão cancelada".to_string();
                } else {
                    relm4::main_application().quit();
                }
            }
            AppMsg::Disconnect => {
                self.connect_requested = false;
                self.status_text = i18n::strings(self.config.language).disconnecting.to_string();
                let _ = self.cmd_tx.send(BackendCmd::Disconnect);
            }
            AppMsg::Forget(ssid) => {
                self.status_text = i18n::removing_profile(self.config.language, &ssid);
                let _ = self.cmd_tx.send(BackendCmd::Forget(ssid));
            }
            AppMsg::ToggleExpandRow(net) => {
                let collapsing = self.expanded_ssid.as_deref() == Some(&net.ssid);
                if collapsing {
                    self.expanded_ssid = None;
                    self.expanded_details = None;
                    self.expanded_password = None;
                    self.expanded_password_visible = false;
                    self.pending_copy_password = false;
                } else {
                    self.expanded_ssid = Some(net.ssid.clone());
                    self.expanded_details = None;
                    self.expanded_password = None;
                    self.expanded_password_visible = false;
                    self.pending_copy_password = false;
                    let _ = self.cmd_tx.send(BackendCmd::FetchDetails(net.clone()));
                }

                let guard = self.networks.guard();
                for idx in 0..guard.len() {
                    let is_target = self
                        .expanded_ssid
                        .as_deref()
                        .is_some_and(|ssid| guard[idx].network.ssid == ssid);
                    guard.send(
                        idx,
                        NetworkRowMsg::SetExpanded {
                            expanded: is_target,
                            details: None,
                            password: None,
                        },
                    );
                }
            }
            AppMsg::FetchPassword { ssid, visible } => {
                self.expanded_password_visible = visible;
                if visible && self.expanded_password.is_none() {
                    let _ = self.cmd_tx.send(BackendCmd::FetchPassword(ssid));
                }
            }
            AppMsg::CopyPassword(ssid) => {
                if let Some(pw) = &self.expanded_password {
                    if let Some(display) = gdk::Display::default() {
                        display.clipboard().set_text(pw);
                        self.status_text = "Senha copiada".to_string();
                    }
                } else {
                    self.pending_copy_password = true;
                    let _ = self.cmd_tx.send(BackendCmd::FetchPassword(ssid));
                }
            }
            AppMsg::SetAutoconnect { ssid, enabled } => {
                let _ = self
                    .cmd_tx
                    .send(BackendCmd::SetAutoconnect { ssid, enabled });
            }
            AppMsg::DetailsLoaded(details) => {
                if self.expanded_ssid.as_deref() != Some(&details.ssid) {
                    return;
                }
                self.expanded_details = Some(details.clone());
                let guard = self.networks.guard();
                for idx in 0..guard.len() {
                    if guard[idx].network.ssid == details.ssid {
                        guard.send(idx, NetworkRowMsg::SetDetails(details));
                        break;
                    }
                }
            }
            AppMsg::PasswordLoaded { ssid, password } => {
                if self.expanded_ssid.as_deref() != Some(&ssid) {
                    return;
                }
                self.expanded_password = password.clone();
                if self.pending_copy_password {
                    self.pending_copy_password = false;
                    if let Some(pw) = &password
                        && let Some(display) = gdk::Display::default()
                    {
                        display.clipboard().set_text(pw);
                        self.status_text = "Senha copiada".to_string();
                    }
                }
                let guard = self.networks.guard();
                for idx in 0..guard.len() {
                    if guard[idx].network.ssid == ssid {
                        guard.send(idx, NetworkRowMsg::SetPassword(password));
                        break;
                    }
                }
            }
            AppMsg::ToggleExpandSaved(ssid) => {
                if self.expanded_saved_ssid.as_deref() == Some(&ssid) {
                    self.expanded_saved_ssid = None;
                } else {
                    self.expanded_saved_ssid = Some(ssid);
                }

                let guard = self.saved_networks.guard();
                for idx in 0..guard.len() {
                    let is_target = self
                        .expanded_saved_ssid
                        .as_deref()
                        .is_some_and(|s| guard[idx].info.ssid == s);
                    guard.send(idx, SavedRowMsg::SetExpanded(is_target));
                }
            }
            AppMsg::SavedNetworksLoaded(list) => {
                let mut guard = self.saved_networks.guard();
                guard.clear();
                let lang = self.config.language;
                for info in list {
                    let expanded = self.expanded_saved_ssid.as_deref() == Some(&info.ssid);
                    guard.push_back(SavedRowInit {
                        info,
                        lang,
                        expanded,
                    });
                }
            }
            AppMsg::BackendState {
                interface,
                wifi_enabled,
                networks,
            } => {
                self.interface = interface;
                self.wifi_enabled = wifi_enabled;

                let has_connected = networks.iter().any(|n| n.connected);
                if self.connect_requested && has_connected && self.config.close_on_connect {
                    relm4::main_application().quit();
                    return;
                }

                let mut guard = self.networks.guard();
                guard.clear();
                let lang = self.config.language;
                if wifi_enabled {
                    let min_strength = self.config.min_signal_strength;
                    for net in networks
                        .into_iter()
                        .filter(|n| n.connected || n.strength >= min_strength)
                    {
                        let is_expanded = self.expanded_ssid.as_deref() == Some(&net.ssid);
                        let details = if is_expanded {
                            self.expanded_details.clone()
                        } else {
                            None
                        };
                        let password = if is_expanded {
                            self.expanded_password.clone()
                        } else {
                            None
                        };
                        guard.push_back(NetworkRowInit {
                            network: net,
                            lang,
                            expanded: is_expanded,
                            details,
                            password,
                            password_visible: is_expanded && self.expanded_password_visible,
                        });
                    }
                    self.status_text = i18n::networks_found(lang, guard.len());
                } else {
                    self.pending_network = None;
                    self.status_text = i18n::strings(lang).wifi_off.to_string();
                }
            }
            AppMsg::BackendStatus(text) => {
                self.status_text = text;
            }
            AppMsg::BackendError(err) => {
                self.scanning = false;
                self.status_text = format!("Erro: {err}");
            }
            AppMsg::ScanFinished => {
                self.scanning = false;
            }
        }
    }
}

impl WifiApp {
    fn open_app_chooser(&mut self, path: std::path::PathBuf, sender: &ComponentSender<Self>) {
        if self.app_chooser_path.as_ref() == Some(&path) {
            self.app_chooser_path = None;
            self.app_chooser_apps.clear();
            return;
        }

        while let Some(child) = self.app_chooser_list.first_child() {
            self.app_chooser_list.remove(&child);
        }

        let (content_type, _) = gio::content_type_guess(Some(&path), None);
        let mut apps = gio::AppInfo::all_for_type(&content_type);
        if apps.is_empty() {
            apps = gio::AppInfo::all_for_type("text/plain");
        }
        apps.retain(|a| a.should_show());

        for (idx, app) in apps.iter().enumerate() {
            let row_btn = gtk::Button::new();
            row_btn.set_has_frame(false);
            row_btn.add_css_class("wifi-popover-btn");

            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if let Some(icon) = app.icon() {
                let img = gtk::Image::from_gicon(&icon);
                hbox.append(&img);
            }
            let lbl = gtk::Label::new(Some(&app.display_name()));
            lbl.set_halign(gtk::Align::Start);
            lbl.set_hexpand(true);
            hbox.append(&lbl);

            row_btn.set_child(Some(&hbox));
            let sender_clone = sender.clone();
            row_btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::LaunchWithApp(idx));
            });
            self.app_chooser_list.append(&row_btn);
        }

        self.app_chooser_apps = apps;
        self.app_chooser_path = Some(path);
    }

    fn refresh_hidden_can_submit(&mut self) {
        let has_ssid = !self.hidden_ssid_entry.text().trim().is_empty();
        let has_pass = self.hidden_security == WifiSecurity::Open
            || !self.hidden_password_entry.text().is_empty();
        self.hidden_can_submit = has_ssid && has_pass;
    }
}

fn spawn_dbus_worker(
    sender: ComponentSender<WifiApp>,
    mut cmd_rx: mpsc::UnboundedReceiver<BackendCmd>,
) {
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            sender.input(AppMsg::BackendError(
                "Falha ao iniciar runtime tokio".to_string(),
            ));
            return;
        };

        rt.block_on(async move {
            let connection = match dbus::connect_system().await {
                Ok(conn) => conn,
                Err(e) => {
                    sender.input(AppMsg::BackendError(format!("D-Bus: {e}")));
                    return;
                }
            };

            let manager = match WifiManager::new(connection.clone()).await {
                Ok(Some(mgr)) => mgr,
                Ok(None) => {
                    sender.input(AppMsg::BackendError(
                        "Nenhuma interface Wi-Fi encontrada".to_string(),
                    ));
                    return;
                }
                Err(e) => {
                    sender.input(AppMsg::BackendError(format!("NetworkManager: {e}")));
                    return;
                }
            };

            emit_snapshot(&manager, &sender).await;

            let Ok(nm) = NetworkManagerProxy::new(&connection).await else {
                return;
            };
            let Ok(wireless) = WirelessDeviceProxy::builder(&connection)
                .path(manager.device_path.as_str())
            else {
                return;
            };
            let Ok(wireless) = wireless.build().await else {
                return;
            };
            let Ok(device) = DeviceProxy::builder(&connection)
                .path(manager.device_path.as_str())
            else {
                return;
            };
            let Ok(device) = device.build().await else {
                return;
            };

            let Ok(mut ap_added) = wireless.receive_access_point_added().await else {
                return;
            };
            let Ok(mut ap_removed) = wireless.receive_access_point_removed().await else {
                return;
            };
            let mut wireless_changed = nm.receive_wireless_enabled_changed().await;
            let mut active_ap_changed = wireless.receive_active_access_point_changed().await;
            let mut device_state_changed = device.receive_state_changed().await;

            loop {
                tokio::select! {
                    maybe_cmd = cmd_rx.recv() => {
                        let Some(cmd) = maybe_cmd else {
                            break;
                        };
                        handle_backend_cmd(&manager, cmd, &sender).await;
                        emit_snapshot(&manager, &sender).await;
                    }
                    Some(_) = ap_added.next() => {
                        emit_snapshot(&manager, &sender).await;
                    }
                    Some(_) = ap_removed.next() => {
                        emit_snapshot(&manager, &sender).await;
                    }
                    Some(_) = wireless_changed.next() => {
                        emit_snapshot(&manager, &sender).await;
                    }
                    Some(_) = active_ap_changed.next() => {
                        emit_snapshot(&manager, &sender).await;
                    }
                    Some(_) = device_state_changed.next() => {
                        emit_snapshot(&manager, &sender).await;
                    }
                }
            }
        });
    });
}

async fn handle_backend_cmd(
    manager: &WifiManager,
    cmd: BackendCmd,
    sender: &ComponentSender<WifiApp>,
) {
    match cmd {
        BackendCmd::Scan => {
            if let Err(e) = manager.request_scan().await {
                sender.input(AppMsg::BackendError(format!("Scan: {e}")));
            }
            sender.input(AppMsg::ScanFinished);
        }
        BackendCmd::ToggleWifi(enabled) => {
            if let Err(e) = manager.set_wireless_enabled(enabled).await {
                sender.input(AppMsg::BackendError(format!("Toggle Wi-Fi: {e}")));
            }
        }
        BackendCmd::Connect { network, password } => {
            let ssid = network.ssid.clone();
            match manager.connect(&network, password.as_deref()).await {
                Ok(()) => sender.input(AppMsg::BackendStatus(format!(
                    "Conexão solicitada para \"{ssid}\""
                ))),
                Err(e) => sender.input(AppMsg::BackendError(format!("Conectar: {e}"))),
            }
        }
        BackendCmd::ConnectHidden {
            ssid,
            security,
            password,
            hidden,
        } => {
            match manager
                .connect_hidden(&ssid, security, password.as_deref(), hidden)
                .await
            {
                Ok(()) => sender.input(AppMsg::BackendStatus(format!(
                    "Conexão solicitada para \"{ssid}\""
                ))),
                Err(e) => sender.input(AppMsg::BackendError(format!("Rede oculta: {e}"))),
            }
        }
        BackendCmd::Disconnect => {
            if let Err(e) = manager.disconnect().await {
                sender.input(AppMsg::BackendError(format!("Desconectar: {e}")));
            }
        }
        BackendCmd::Forget(ssid) => {
            match manager.forget_network(&ssid).await {
                Ok(true) => {
                    sender.input(AppMsg::BackendStatus(format!("Perfil \"{ssid}\" removido")));
                    if let Ok(saved) = manager.list_saved_networks().await {
                        sender.input(AppMsg::SavedNetworksLoaded(saved));
                    }
                }
                Ok(false) => sender.input(AppMsg::BackendStatus(format!(
                    "Perfil \"{ssid}\" não encontrado"
                ))),
                Err(e) => sender.input(AppMsg::BackendError(format!("Esquecer: {e}"))),
            }
        }
        BackendCmd::ListSaved => match manager.list_saved_networks().await {
            Ok(list) => sender.input(AppMsg::SavedNetworksLoaded(list)),
            Err(e) => sender.input(AppMsg::BackendError(format!("Redes salvas: {e}"))),
        },
        BackendCmd::FetchDetails(net) => {
            match manager.network_details(&net).await {
                Ok(details) => sender.input(AppMsg::DetailsLoaded(details)),
                Err(e) => sender.input(AppMsg::BackendError(format!("Detalhes: {e}"))),
            }
        }
        BackendCmd::FetchPassword(ssid) => {
            match manager.get_saved_password(&ssid).await {
                Ok(password) => sender.input(AppMsg::PasswordLoaded { ssid, password }),
                Err(e) => sender.input(AppMsg::BackendError(format!("Senha: {e}"))),
            }
        }
        BackendCmd::SetAutoconnect { ssid, enabled } => {
            if let Err(e) = manager.set_autoconnect(&ssid, enabled).await {
                sender.input(AppMsg::BackendError(format!("Auto-reconectar: {e}")));
            }
        }
    }
}

async fn emit_snapshot(manager: &WifiManager, sender: &ComponentSender<WifiApp>) {
    let wifi_enabled = match manager.is_wireless_enabled().await {
        Ok(val) => val,
        Err(e) => {
            sender.input(AppMsg::BackendError(format!("Status Wi-Fi: {e}")));
            return;
        }
    };

    let networks = if wifi_enabled {
        match manager.list_networks().await {
            Ok(list) => list,
            Err(e) => {
                sender.input(AppMsg::BackendError(format!("Listar redes: {e}")));
                return;
            }
        }
    } else {
        Vec::new()
    };

    sender.input(AppMsg::BackendState {
        interface: manager.interface.clone(),
        wifi_enabled,
        networks,
    });
}

fn build_menu_popover(
    sender: &ComponentSender<WifiApp>,
    lang: Language,
) -> (gtk::Popover, [gtk::Button; 3]) {
    let popover = gtk::Popover::new();
    popover.add_css_class("wifi-menu-popover");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let s = i18n::strings(lang);

    let items = [
        (s.menu_saved_networks, ActivePage::SavedNetworks),
        (s.menu_settings, ActivePage::Settings),
        (s.menu_about, ActivePage::About),
    ];

    let mut buttons = Vec::with_capacity(3);
    for (label, page) in items {
        let btn = gtk::Button::with_label(label);
        btn.set_has_frame(false);
        btn.add_css_class("wifi-popover-btn");
        if let Some(child) = btn.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
            child.set_halign(gtk::Align::Start);
        }

        let sender_clone = sender.clone();
        btn.connect_clicked(move |_| {
            sender_clone.input(AppMsg::Navigate(page));
        });
        vbox.append(&btn);
        buttons.push(btn);
    }

    popover.set_child(Some(&vbox));
    let buttons: [gtk::Button; 3] = buttons.try_into().expect("3 botões no menu");
    (popover, buttons)
}

fn config_dir_display() -> String {
    config::paths()
        .map(|(dir, _, _)| dir.display().to_string())
        .unwrap_or_else(|| "~/.config/just-wifi-menu".to_string())
}

fn rgba_to_hex(rgba: &gdk::RGBA) -> String {
    let r = (rgba.red().clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (rgba.green().clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (rgba.blue().clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn setup_layer_shell(window: &gtk::ApplicationWindow, config: &AppConfig) {
    if !gtk4_layer_shell::is_supported() {
        tracing::warn!(
            backend = ?std::env::var("GDK_BACKEND").ok(),
            "gtk4-layer-shell não suportado neste backend"
        );
        return;
    }

    tracing::debug!(anchor = %config.anchor, "Inicializando layer-shell");

    window.init_layer_shell();
    window.set_namespace(Some("just-wifi-menu"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    apply_layer_shell_position(window, config);
}

fn apply_layer_shell_position(window: &gtk::ApplicationWindow, config: &AppConfig) {
    if !gtk4_layer_shell::is_supported() {
        return;
    }

    window.set_margin(Edge::Top, config.margin_top);
    window.set_margin(Edge::Right, config.margin_right);
    window.set_margin(Edge::Bottom, config.margin_bottom);
    window.set_margin(Edge::Left, config.margin_left);

    for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
        window.set_anchor(edge, false);
    }

    match config.anchor.as_str() {
        "top-right" => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
        "top-left" => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
        "bottom-right" => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
        }
        "bottom-left" => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
        }
        _ => {}
    }
}

fn setup_close_handlers(
    window: &gtk::ApplicationWindow,
    menu_popover: &gtk::Popover,
    close_on_focus_loss: &Rc<Cell<bool>>,
    sender: &ComponentSender<WifiApp>,
) {
    let key_controller = gtk::EventControllerKey::new();
    let sender_clone = sender.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            sender_clone.input(AppMsg::EscapePressed);
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);

    let was_active = Cell::new(false);
    let popover_ref = menu_popover.clone();
    let flag_focus = Rc::clone(close_on_focus_loss);
    window.connect_is_active_notify(move |win| {
        if win.is_active() {
            was_active.set(true);
        } else if flag_focus.get() && was_active.get() && !popover_ref.is_visible() {
            relm4::main_application().quit();
        }
    });

    let win_weak = window.downgrade();
    let flag_popover = Rc::clone(close_on_focus_loss);
    menu_popover.connect_closed(move |_| {
        let win_weak = win_weak.clone();
        let flag = Rc::clone(&flag_popover);
        gtk::glib::idle_add_local_once(move || {
            if flag.get()
                && let Some(win) = win_weak.upgrade()
                && !win.is_active()
            {
                relm4::main_application().quit();
            }
        });
    });
}

pub fn run_gui(config: AppConfig) {
    let app = RelmApp::new("com.just.wifi.menu").with_args(vec![]);
    app.run::<WifiApp>(config);
}
