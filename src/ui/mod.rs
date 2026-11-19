mod network_row;

use std::cell::Cell;

use futures::StreamExt;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::gtk::gdk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tokio::sync::mpsc;

use crate::config::{self, AppConfig};
use crate::dbus;
use crate::dbus::proxies::{DeviceProxy, NetworkManagerProxy, WirelessDeviceProxy};
use crate::dbus::wifi::{NetworkDetails, WifiManager, WifiNetwork, WifiSecurity};
use network_row::{NetworkRow, NetworkRowInit, NetworkRowMsg, NetworkRowOutput};

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

#[derive(Debug)]
pub enum AppMsg {
    Navigate(ActivePage),
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
    config: AppConfig,
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
    password_entry: gtk::PasswordEntry,
    hidden_ssid_entry: gtk::Entry,
    hidden_password_entry: gtk::PasswordEntry,
    hidden_security: WifiSecurity,
    hidden_is_hidden: bool,
    hidden_can_submit: bool,
    stack: gtk::Stack,
    menu_popover: gtk::Popover,
    cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    connect_requested: bool,
}

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
                    set_spacing: 10,
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
                            set_tooltip_text: Some("Adicionar rede oculta"),
                            add_css_class: "wifi-action-btn",
                            #[watch]
                            set_sensitive: model.wifi_enabled,
                            connect_clicked => AppMsg::Navigate(ActivePage::AddNetwork),
                        },

                        gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            set_tooltip_text: Some("Escanear redes"),
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
                            set_tooltip_text: Some("Opções"),
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
                                    Some(net) => format!("Senha para \"{}\":", net.ssid),
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
                                    set_placeholder_text: Some("Digite a senha Wi-Fi..."),
                                    connect_activate => AppMsg::ConnectWithPassword,
                                },

                                gtk::Button {
                                    set_label: "Conectar",
                                    add_css_class: "wifi-connect-btn",
                                    connect_clicked => AppMsg::ConnectWithPassword,
                                },

                                gtk::Button {
                                    set_label: "Cancelar",
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
                            set_tooltip_text: Some("Voltar"),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            set_label: "Adicionar rede",
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        add_css_class: "wifi-about-card",

                        gtk::Label {
                            set_label: "Nome da rede (SSID)",
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                        },

                        #[local_ref]
                        hidden_ssid_entry -> gtk::Entry {
                            set_placeholder_text: Some("Ex: MinhaRedeOculta"),
                            connect_changed => AppMsg::HiddenFormChanged,
                            connect_activate => AppMsg::SubmitHiddenNetwork,
                        },

                        gtk::Label {
                            set_label: "Segurança",
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 6,
                            set_homogeneous: true,

                            gtk::ToggleButton {
                                set_label: "Aberta",
                                #[watch]
                                set_active: model.hidden_security == WifiSecurity::Open,
                                connect_clicked => AppMsg::SetHiddenSecurity(WifiSecurity::Open),
                            },

                            gtk::ToggleButton {
                                set_label: "WPA/WPA2",
                                #[watch]
                                set_active: model.hidden_security == WifiSecurity::WpaPsk,
                                connect_clicked => AppMsg::SetHiddenSecurity(WifiSecurity::WpaPsk),
                            },

                            gtk::ToggleButton {
                                set_label: "WPA3",
                                #[watch]
                                set_active: model.hidden_security == WifiSecurity::Sae,
                                connect_clicked => AppMsg::SetHiddenSecurity(WifiSecurity::Sae),
                            },
                        },

                        gtk::Label {
                            set_label: "Senha",
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                            #[watch]
                            set_visible: model.hidden_security != WifiSecurity::Open,
                        },

                        #[local_ref]
                        hidden_password_entry -> gtk::PasswordEntry {
                            set_show_peek_icon: true,
                            set_placeholder_text: Some("Digite a senha Wi-Fi..."),
                            #[watch]
                            set_visible: model.hidden_security != WifiSecurity::Open,
                            connect_changed => AppMsg::HiddenFormChanged,
                            connect_activate => AppMsg::SubmitHiddenNetwork,
                        },

                        gtk::CheckButton {
                            set_label: Some("Rede oculta (não transmite SSID)"),
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
                                set_label: "Cancelar",
                                add_css_class: "wifi-action-btn",
                                connect_clicked => AppMsg::Navigate(ActivePage::Main),
                            },

                            gtk::Button {
                                set_label: "Conectar",
                                add_css_class: "wifi-connect-btn",
                                #[watch]
                                set_sensitive: model.hidden_can_submit,
                                connect_clicked => AppMsg::SubmitHiddenNetwork,
                            },
                        },
                    },
                },

                // Página: Redes Salvas (placeholder para Etapa F4)
                add_named[Some("saved")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            set_tooltip_text: Some("Voltar"),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            set_label: "Redes salvas",
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::Label {
                        set_label: "Em construção (Etapa F4).",
                        set_vexpand: true,
                        add_css_class: "wifi-status",
                    },
                },

                // Página: Personalização (placeholder para Etapa F6)
                add_named[Some("settings")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            set_tooltip_text: Some("Voltar"),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            set_label: "Personalização",
                            add_css_class: "wifi-title",
                        },
                    },

                    gtk::Label {
                        set_label: "Em construção (Etapa F6).",
                        set_vexpand: true,
                        add_css_class: "wifi-status",
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
                            set_tooltip_text: Some("Voltar"),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => AppMsg::Navigate(ActivePage::Main),
                        },

                        gtk::Label {
                            set_label: "Sobre",
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
                            set_label: &format!("Versão {}", env!("CARGO_PKG_VERSION")),
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::Label {
                            set_label: env!("CARGO_PKG_DESCRIPTION"),
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
                            set_label: "Repositório no GitHub",
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
        let stack = gtk::Stack::new();
        let menu_popover = build_menu_popover(&sender);

        config::apply_css();
        setup_layer_shell(&root, &config);
        setup_close_handlers(&root, &menu_popover, &config, &sender);

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

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        spawn_dbus_worker(sender.clone(), cmd_rx);

        let model = Self {
            config,
            active_page: ActivePage::Main,
            interface: String::new(),
            wifi_enabled: false,
            scanning: false,
            status_text: "Conectando ao NetworkManager...".to_string(),
            pending_network: None,
            expanded_ssid: None,
            expanded_details: None,
            expanded_password: None,
            expanded_password_visible: false,
            pending_copy_password: false,
            networks,
            password_entry: password_entry.clone(),
            hidden_ssid_entry: hidden_ssid_entry.clone(),
            hidden_password_entry: hidden_password_entry.clone(),
            hidden_security: WifiSecurity::WpaPsk,
            hidden_is_hidden: true,
            hidden_can_submit: false,
            stack: stack.clone(),
            menu_popover,
            cmd_tx,
            connect_requested: false,
        };

        let stack = &model.stack;
        let networks_box = model.networks.widget();
        let password_entry = &model.password_entry;
        let hidden_ssid_entry = &model.hidden_ssid_entry;
        let hidden_password_entry = &model.hidden_password_entry;
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Navigate(page) => {
                self.menu_popover.popdown();
                self.active_page = page;
                self.stack.set_visible_child_name(page.stack_name());
            }
            AppMsg::ToggleWifi(enabled) => {
                if self.wifi_enabled == enabled {
                    return;
                }
                self.wifi_enabled = enabled;
                self.pending_network = None;
                self.status_text = if enabled {
                    "Ligando Wi-Fi...".to_string()
                } else {
                    "Desligando Wi-Fi...".to_string()
                };
                let _ = self.cmd_tx.send(BackendCmd::ToggleWifi(enabled));
            }
            AppMsg::Scan => {
                self.scanning = true;
                self.status_text = "Escaneando redes...".to_string();
                let _ = self.cmd_tx.send(BackendCmd::Scan);
            }
            AppMsg::SelectNetwork(net) => {
                if net.secured && !net.saved {
                    self.password_entry.set_text("");
                    self.status_text = format!("Digite a senha para \"{}\"", net.ssid);
                    self.pending_network = Some(net);
                } else {
                    self.connect_requested = true;
                    self.status_text = format!("Conectando em \"{}\"...", net.ssid);
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
                self.status_text = format!("Conectando em \"{}\"...", net.ssid);
                let _ = self.cmd_tx.send(BackendCmd::Connect {
                    network: net,
                    password: Some(password),
                });
            }
            AppMsg::CancelPassword => {
                self.pending_network = None;
                self.password_entry.set_text("");
                self.status_text = "Conexão cancelada".to_string();
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
                self.status_text = format!("Conectando em \"{ssid}\"...");

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
                self.status_text = "Desconectando...".to_string();
                let _ = self.cmd_tx.send(BackendCmd::Disconnect);
            }
            AppMsg::Forget(ssid) => {
                self.status_text = format!("Removendo perfil \"{ssid}\"...");
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
                            expanded: is_expanded,
                            details,
                            password,
                            password_visible: is_expanded && self.expanded_password_visible,
                        });
                    }
                    self.status_text = format!("{} rede(s) encontrada(s)", guard.len());
                } else {
                    self.pending_network = None;
                    self.status_text = "Wi-Fi desligado".to_string();
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
                Ok(true) => sender.input(AppMsg::BackendStatus(format!(
                    "Perfil \"{ssid}\" removido"
                ))),
                Ok(false) => sender.input(AppMsg::BackendStatus(format!(
                    "Perfil \"{ssid}\" não encontrado"
                ))),
                Err(e) => sender.input(AppMsg::BackendError(format!("Esquecer: {e}"))),
            }
        }
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

fn build_menu_popover(sender: &ComponentSender<WifiApp>) -> gtk::Popover {
    let popover = gtk::Popover::new();
    popover.add_css_class("wifi-menu-popover");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 4);

    let items = [
        ("Redes salvas", ActivePage::SavedNetworks),
        ("Personalização", ActivePage::Settings),
        ("Sobre", ActivePage::About),
    ];

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
    }

    popover.set_child(Some(&vbox));
    popover
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

    window.set_margin(Edge::Top, config.margin_top);
    window.set_margin(Edge::Right, config.margin_right);
    window.set_margin(Edge::Bottom, config.margin_bottom);
    window.set_margin(Edge::Left, config.margin_left);

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
    config: &AppConfig,
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

    if config.close_on_focus_loss {
        let was_active = Cell::new(false);
        let popover_ref = menu_popover.clone();
        window.connect_is_active_notify(move |win| {
            if win.is_active() {
                was_active.set(true);
            } else if was_active.get() && !popover_ref.is_visible() {
                relm4::main_application().quit();
            }
        });

        let win_weak = window.downgrade();
        menu_popover.connect_closed(move |_| {
            let win_weak = win_weak.clone();
            gtk::glib::idle_add_local_once(move || {
                if let Some(win) = win_weak.upgrade()
                    && !win.is_active()
                {
                    relm4::main_application().quit();
                }
            });
        });
    }
}

pub fn run_gui(config: AppConfig) {
    let app = RelmApp::new("com.just.wifi.menu").with_args(vec![]);
    app.run::<WifiApp>(config);
}
