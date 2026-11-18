use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

#[derive(Debug, Clone)]
struct MockNetwork {
    ssid: String,
    strength: u8,
    secured: bool,
    connected: bool,
    saved: bool,
}

#[derive(Debug)]
pub enum AppMsg {
    ToggleWifi(bool),
    Scan,
    SelectNetwork(String),
    ConnectWithPassword,
    CancelPassword,
    Disconnect(String),
}

pub struct WifiApp {
    wifi_enabled: bool,
    scanning: bool,
    status_text: String,
    networks: Vec<MockNetwork>,
    pending_ssid: Option<String>,
    list_box: gtk::ListBox,
    password_entry: gtk::PasswordEntry,
}

#[relm4::component(pub)]
impl SimpleComponent for WifiApp {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        gtk::ApplicationWindow {
            set_title: Some("Just Wifi Menu"),
            set_default_width: 380,
            set_default_height: 480,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 12,

                // Cabeçalho: título + botão de scan + switch Wi-Fi
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,

                    gtk::Label {
                        set_label: "Wi-Fi",
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Spinner {
                        #[watch]
                        set_spinning: model.scanning,
                        #[watch]
                        set_visible: model.scanning,
                    },

                    gtk::Button {
                        set_icon_name: "view-refresh-symbolic",
                        set_tooltip_text: Some("Escanear redes"),
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
                },

                // Barra de status simples
                gtk::Label {
                    #[watch]
                    set_label: &model.status_text,
                    set_halign: gtk::Align::Start,
                },

                // Lista rolável de redes
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                    #[local_ref]
                    list_box -> gtk::ListBox {
                        set_selection_mode: gtk::SelectionMode::None,
                    }
                },

                // Painel expansível para digitar senha (quando necessário)
                gtk::Revealer {
                    #[watch]
                    set_reveal_child: model.pending_ssid.is_some(),

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

                        gtk::Label {
                            #[watch]
                            set_label: &match &model.pending_ssid {
                                Some(ssid) => format!("Senha para \"{ssid}\":"),
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
                                connect_clicked => AppMsg::ConnectWithPassword,
                            },

                            gtk::Button {
                                set_label: "Cancelar",
                                connect_clicked => AppMsg::CancelPassword,
                            },
                        }
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let list_box = gtk::ListBox::new();
        let password_entry = gtk::PasswordEntry::new();

        let model = Self {
            wifi_enabled: true,
            scanning: false,
            status_text: "Modo de demonstração (dados mockados)".to_string(),
            networks: sample_networks(),
            pending_ssid: None,
            list_box: list_box.clone(),
            password_entry: password_entry.clone(),
        };

        let list_box = &model.list_box;
        let password_entry = &model.password_entry;
        let widgets = view_output!();

        rebuild_network_rows(&model.list_box, &model.networks, model.wifi_enabled, &sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::ToggleWifi(enabled) => {
                self.wifi_enabled = enabled;
                self.pending_ssid = None;
                self.status_text = if enabled {
                    "Wi-Fi ligado (mock)".to_string()
                } else {
                    "Wi-Fi desligado (mock)".to_string()
                };
            }
            AppMsg::Scan => {
                self.scanning = !self.scanning;
                self.status_text = if self.scanning {
                    "Escaneando redes (mock)...".to_string()
                } else {
                    "Scan concluído (mock)".to_string()
                };
            }
            AppMsg::SelectNetwork(ssid) => {
                let Some(net) = self.networks.iter().find(|n| n.ssid == ssid) else {
                    return;
                };

                if net.connected {
                    self.status_text = format!("Já conectado em \"{ssid}\"");
                } else if net.secured && !net.saved {
                    self.password_entry.set_text("");
                    self.pending_ssid = Some(ssid.clone());
                    self.status_text = format!("Aguardando senha para \"{ssid}\"");
                } else {
                    self.connect_mock(&ssid);
                }
            }
            AppMsg::ConnectWithPassword => {
                let Some(ssid) = self.pending_ssid.take() else {
                    return;
                };
                let _password = self.password_entry.text();
                self.password_entry.set_text("");
                self.connect_mock(&ssid);
            }
            AppMsg::CancelPassword => {
                self.pending_ssid = None;
                self.password_entry.set_text("");
                self.status_text = "Conexão cancelada".to_string();
            }
            AppMsg::Disconnect(ssid) => {
                for net in &mut self.networks {
                    if net.ssid == ssid {
                        net.connected = false;
                    }
                }
                self.status_text = format!("Desconectado de \"{ssid}\" (mock)");
            }
        }

        rebuild_network_rows(&self.list_box, &self.networks, self.wifi_enabled, &sender);
    }
}

impl WifiApp {
    fn connect_mock(&mut self, target_ssid: &str) {
        for net in &mut self.networks {
            if net.ssid == target_ssid {
                net.connected = true;
                net.saved = true;
            } else {
                net.connected = false;
            }
        }
        self.status_text = format!("Conectado em \"{target_ssid}\" (mock)");
    }
}

fn rebuild_network_rows(
    list_box: &gtk::ListBox,
    networks: &[MockNetwork],
    wifi_enabled: bool,
    sender: &ComponentSender<WifiApp>,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    if !wifi_enabled {
        let label = gtk::Label::new(Some("Wi-Fi desligado"));
        label.set_margin_top(24);
        list_box.append(&label);
        return;
    }

    for net in networks {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(6);
        row.set_margin_end(6);

        let icon_name = signal_icon_name(net.strength, net.secured);
        let icon = gtk::Image::from_icon_name(icon_name);
        row.append(&icon);

        let title = if net.connected {
            format!("{} (Conectado)", net.ssid)
        } else if net.saved {
            format!("{} (Salvo)", net.ssid)
        } else {
            net.ssid.clone()
        };

        let label = gtk::Label::new(Some(&title));
        label.set_hexpand(true);
        label.set_halign(gtk::Align::Start);
        row.append(&label);

        let strength_label = gtk::Label::new(Some(&format!("{}%", net.strength)));
        row.append(&strength_label);

        let ssid = net.ssid.clone();
        let sender_clone = sender.clone();

        if net.connected {
            let btn = gtk::Button::with_label("Desconectar");
            btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::Disconnect(ssid.clone()));
            });
            row.append(&btn);
        } else {
            let btn = gtk::Button::with_label("Conectar");
            btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::SelectNetwork(ssid.clone()));
            });
            row.append(&btn);
        }

        list_box.append(&row);
    }
}

fn signal_icon_name(strength: u8, secured: bool) -> &'static str {
    let level = match strength {
        75..=100 => "excellent",
        50..=74 => "good",
        25..=49 => "ok",
        _ => "weak",
    };

    match (level, secured) {
        ("excellent", true) => "network-wireless-signal-excellent-secure-symbolic",
        ("excellent", false) => "network-wireless-signal-excellent-symbolic",
        ("good", true) => "network-wireless-signal-good-secure-symbolic",
        ("good", false) => "network-wireless-signal-good-symbolic",
        ("ok", true) => "network-wireless-signal-ok-secure-symbolic",
        ("ok", false) => "network-wireless-signal-ok-symbolic",
        (_, true) => "network-wireless-signal-weak-secure-symbolic",
        (_, false) => "network-wireless-signal-weak-symbolic",
    }
}

fn sample_networks() -> Vec<MockNetwork> {
    vec![
        MockNetwork {
            ssid: "The Force -5G".to_string(),
            strength: 78,
            secured: true,
            connected: true,
            saved: true,
        },
        MockNetwork {
            ssid: "Cafe_Visitantes".to_string(),
            strength: 62,
            secured: false,
            connected: false,
            saved: false,
        },
        MockNetwork {
            ssid: "Vizinho_Fibra".to_string(),
            strength: 38,
            secured: true,
            connected: false,
            saved: false,
        },
        MockNetwork {
            ssid: "Escritorio".to_string(),
            strength: 54,
            secured: true,
            connected: false,
            saved: true,
        },
    ]
}

pub fn run_gui() {
    let app = RelmApp::new("com.just.wifi.menu");
    app.run::<WifiApp>(());
}
