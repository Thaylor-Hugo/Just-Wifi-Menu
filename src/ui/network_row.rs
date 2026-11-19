use relm4::gtk;
use relm4::gtk::gdk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::dbus::wifi::{NetworkDetails, WifiNetwork};
use crate::i18n::{self, Language};

#[derive(Debug, Clone)]
pub struct NetworkRowInit {
    pub network: WifiNetwork,
    pub lang: Language,
    pub expanded: bool,
    pub details: Option<NetworkDetails>,
    pub password: Option<String>,
    pub password_visible: bool,
}

#[derive(Debug, Clone)]
pub struct NetworkRow {
    pub network: WifiNetwork,
    pub lang: Language,
    pub expanded: bool,
    pub details: Option<NetworkDetails>,
    pub password: Option<String>,
    pub password_visible: bool,
}

#[derive(Debug)]
pub enum NetworkRowMsg {
    ToggleExpandClicked,
    SetLanguage(Language),
    SetExpanded {
        expanded: bool,
        details: Option<NetworkDetails>,
        password: Option<String>,
    },
    SetDetails(NetworkDetails),
    SetPassword(Option<String>),
    TogglePasswordVisibility,
    CopyText(String),
    ToggleAutoconnect(bool),
}

#[derive(Debug)]
pub enum NetworkRowOutput {
    ToggleExpand(WifiNetwork),
    Connect(WifiNetwork),
    Disconnect,
    Forget(String),
    FetchPassword { ssid: String, visible: bool },
    CopyPassword(String),
    SetAutoconnect { ssid: String, enabled: bool },
    CopiedFeedback(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for NetworkRow {
    type Init = NetworkRowInit;
    type Input = NetworkRowMsg;
    type Output = NetworkRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 4,
            add_css_class: "wifi-row",
            add_css_class: if self.network.connected {
                "wifi-row-connected"
            } else if self.network.saved {
                "wifi-row-saved"
            } else {
                "wifi-row-normal"
            },

            // Linha principal
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,

                gtk::Button {
                    set_has_frame: false,
                    set_hexpand: true,
                    add_css_class: "wifi-row-expand-btn",
                    connect_clicked => NetworkRowMsg::ToggleExpandClicked,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Image {
                            #[watch]
                            set_icon_name: Some(if self.expanded {
                                "pan-down-symbolic"
                            } else {
                                "pan-end-symbolic"
                            }),
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,
                            set_spacing: 2,

                            gtk::Label {
                                set_label: &self.network.ssid,
                                set_halign: gtk::Align::Start,
                                set_ellipsize: gtk::pango::EllipsizeMode::End,
                                add_css_class: "wifi-ssid",
                            },

                            gtk::Label {
                                #[watch]
                                set_label: &subtitle_text(&self.network, self.lang),
                                set_halign: gtk::Align::Start,
                                add_css_class: "wifi-subtitle",
                            },
                        },

                        gtk::Image {
                            set_icon_name: Some("changes-prevent-symbolic"),
                            set_visible: self.network.secured,
                            set_pixel_size: 13,
                            add_css_class: "wifi-lock-icon",
                        },

                        gtk::Image {
                            set_icon_name: Some(signal_icon_name(self.network.strength)),
                            set_pixel_size: 16,
                            add_css_class: "wifi-signal-icon",
                        },
                    },
                },

                gtk::Button {
                    #[watch]
                    set_label: if self.network.connected {
                        i18n::strings(self.lang).disconnect
                    } else {
                        i18n::strings(self.lang).connect
                    },
                    set_valign: gtk::Align::Center,
                    set_margin_end: 8,
                    add_css_class: "wifi-action-btn",
                    connect_clicked[sender, net = self.network.clone()] => move |_| {
                        let msg = if net.connected {
                            NetworkRowOutput::Disconnect
                        } else {
                            NetworkRowOutput::Connect(net.clone())
                        };
                        let _ = sender.output(msg);
                    },
                },
            },

            // Painel expansível de detalhes e ações contextuais
            gtk::Revealer {
                #[watch]
                set_reveal_child: self.expanded,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 6,
                    add_css_class: "wifi-details-box",

                    // Frequência e Segurança (sempre disponível)
                    gtk::Label {
                        #[watch]
                        set_label: &details_summary(&self.network, self.details.as_ref(), self.lang),
                        set_halign: gtk::Align::Start,
                        set_wrap: true,
                        add_css_class: "wifi-subtitle",
                    },

                    // IPv4 + Gateway + Copiar IP (apenas se conectada)
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        #[watch]
                        set_visible: self.network.connected
                            && self.details.as_ref().and_then(|d| d.ipv4.as_ref()).is_some(),

                        gtk::Label {
                            #[watch]
                            set_label: &ip_summary(self.details.as_ref()),
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::Button {
                            set_icon_name: "edit-copy-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(self.lang).copy_ip_tooltip),
                            add_css_class: "wifi-action-btn",
                            connect_clicked[sender] => move |_| {
                                sender.input(NetworkRowMsg::CopyText("ip".to_string()));
                            },
                        },
                    },

                    // Senha salva (apenas se salva e protegida)
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        #[watch]
                        set_visible: self.network.saved && self.network.secured,

                        gtk::Label {
                            #[watch]
                            set_label: &match (self.password_visible, &self.password) {
                                (true, Some(pw)) => i18n::password_revealed(self.lang, pw),
                                (true, None) => i18n::strings(self.lang).password_loading.to_string(),
                                (false, _) => i18n::password_masked(self.lang).to_string(),
                            },
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "wifi-subtitle",
                        },

                        gtk::Button {
                            #[watch]
                            set_label: if self.password_visible {
                                i18n::strings(self.lang).hide_password
                            } else {
                                i18n::strings(self.lang).show_password
                            },
                            add_css_class: "wifi-action-btn",
                            connect_clicked => NetworkRowMsg::TogglePasswordVisibility,
                        },

                        gtk::Button {
                            set_icon_name: "edit-copy-symbolic",
                            #[watch]
                            set_tooltip_text: Some(i18n::strings(self.lang).copy_password_tooltip),
                            add_css_class: "wifi-action-btn",
                            connect_clicked => NetworkRowMsg::CopyText("password".to_string()),
                        },
                    },

                    // Auto-reconectar + Esquecer (apenas se salva)
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,
                        #[watch]
                        set_visible: self.network.saved,

                        gtk::CheckButton {
                            #[watch]
                            set_label: Some(i18n::strings(self.lang).autoconnect),
                            set_hexpand: true,
                            #[watch]
                            set_active: self
                                .details
                                .as_ref()
                                .and_then(|d| d.autoconnect)
                                .unwrap_or(true),
                            connect_toggled[sender] => move |btn| {
                                sender.input(NetworkRowMsg::ToggleAutoconnect(btn.is_active()));
                            },
                        },

                        gtk::Button {
                            set_icon_name: "user-trash-symbolic",
                            #[watch]
                            set_label: i18n::strings(self.lang).forget,
                            add_css_class: "wifi-forget-btn",
                            connect_clicked[sender, ssid = self.network.ssid.clone()] => move |_| {
                                let _ = sender.output(NetworkRowOutput::Forget(ssid.clone()));
                            },
                        },
                    },
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            network: init.network,
            lang: init.lang,
            expanded: init.expanded,
            details: init.details,
            password: init.password,
            password_visible: init.password_visible,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            NetworkRowMsg::ToggleExpandClicked => {
                let _ = sender.output(NetworkRowOutput::ToggleExpand(self.network.clone()));
            }
            NetworkRowMsg::SetLanguage(lang) => {
                self.lang = lang;
            }
            NetworkRowMsg::SetExpanded {
                expanded,
                details,
                password,
            } => {
                self.expanded = expanded;
                if let Some(d) = details {
                    self.details = Some(d);
                }
                if let Some(pw) = password {
                    self.password = Some(pw);
                }
                if !expanded {
                    self.password_visible = false;
                }
            }
            NetworkRowMsg::SetDetails(details) => {
                self.details = Some(details);
            }
            NetworkRowMsg::SetPassword(password) => {
                self.password = password;
            }
            NetworkRowMsg::TogglePasswordVisibility => {
                self.password_visible = !self.password_visible;
                let _ = sender.output(NetworkRowOutput::FetchPassword {
                    ssid: self.network.ssid.clone(),
                    visible: self.password_visible,
                });
            }
            NetworkRowMsg::CopyText(kind) => {
                if kind == "password" && self.password.is_none() {
                    let _ = sender.output(NetworkRowOutput::CopyPassword(self.network.ssid.clone()));
                    return;
                }
                let text = match kind.as_str() {
                    "ip" => self
                        .details
                        .as_ref()
                        .and_then(|d| d.ipv4.as_deref())
                        .map(|s| s.split('/').next().unwrap_or(s).to_string()),
                    "password" => self.password.clone(),
                    _ => None,
                };
                if let Some(val) = text
                    && let Some(display) = gdk::Display::default()
                {
                    display.clipboard().set_text(&val);
                    let s = i18n::strings(self.lang);
                    let label = if kind == "ip" { s.ip_copied } else { s.password_copied };
                    let _ = sender.output(NetworkRowOutput::CopiedFeedback(label.to_string()));
                }
            }
            NetworkRowMsg::ToggleAutoconnect(enabled) => {
                let current = self
                    .details
                    .as_ref()
                    .and_then(|d| d.autoconnect)
                    .unwrap_or(true);
                if current == enabled {
                    return;
                }
                if let Some(details) = &mut self.details {
                    details.autoconnect = Some(enabled);
                }
                let _ = sender.output(NetworkRowOutput::SetAutoconnect {
                    ssid: self.network.ssid.clone(),
                    enabled,
                });
            }
        }
    }
}

fn details_summary(net: &WifiNetwork, details: Option<&NetworkDetails>, lang: Language) -> String {
    let s = i18n::strings(lang);
    let band = if net.frequency >= 5925 {
        "6 GHz"
    } else if net.frequency >= 4900 {
        "5 GHz"
    } else {
        "2.4 GHz"
    };
    let sec = details
        .map(|d| d.security_label.as_str())
        .unwrap_or(if net.secured { s.secured } else { s.open });
    format!(
        "{band} ({} MHz) • {}% • {sec}",
        net.frequency, net.strength
    )
}

fn ip_summary(details: Option<&NetworkDetails>) -> String {
    let Some(d) = details else {
        return String::new();
    };
    match (&d.ipv4, &d.gateway) {
        (Some(ip), Some(gw)) => format!("IPv4: {ip} • Gateway: {gw}"),
        (Some(ip), None) => format!("IPv4: {ip}"),
        _ => String::new(),
    }
}

fn subtitle_text(net: &WifiNetwork, lang: Language) -> String {
    let s = i18n::strings(lang);
    let band = if net.frequency >= 5925 {
        "6 GHz"
    } else if net.frequency >= 4900 {
        "5 GHz"
    } else {
        "2.4 GHz"
    };

    let state = if net.connected {
        s.connected
    } else if net.saved {
        s.saved
    } else if net.secured {
        s.secured
    } else {
        s.open
    };

    format!("{state} • {band}")
}

fn signal_icon_name(strength: u8) -> &'static str {
    match strength {
        75..=100 => "network-wireless-signal-excellent-symbolic",
        50..=74 => "network-wireless-signal-good-symbolic",
        25..=49 => "network-wireless-signal-ok-symbolic",
        1..=24 => "network-wireless-signal-weak-symbolic",
        _ => "network-wireless-signal-none-symbolic",
    }
}
