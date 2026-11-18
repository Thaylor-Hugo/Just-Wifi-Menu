mod network_row;

use futures::StreamExt;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tokio::sync::mpsc;

use crate::dbus;
use crate::dbus::proxies::{DeviceProxy, NetworkManagerProxy, WirelessDeviceProxy};
use crate::dbus::wifi::{WifiManager, WifiNetwork};
use network_row::{NetworkRow, NetworkRowOutput};

#[derive(Debug)]
enum BackendCmd {
    Scan,
    ToggleWifi(bool),
    Connect {
        network: WifiNetwork,
        password: Option<String>,
    },
    Disconnect,
    Forget(String),
}

#[derive(Debug)]
pub enum AppMsg {
    ToggleWifi(bool),
    Scan,
    SelectNetwork(WifiNetwork),
    ConnectWithPassword,
    CancelPassword,
    Disconnect,
    Forget(String),
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
    interface: String,
    wifi_enabled: bool,
    scanning: bool,
    status_text: String,
    pending_network: Option<WifiNetwork>,
    networks: FactoryVecDeque<NetworkRow>,
    password_entry: gtk::PasswordEntry,
    cmd_tx: mpsc::UnboundedSender<BackendCmd>,
}

#[relm4::component(pub)]
impl SimpleComponent for WifiApp {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        gtk::ApplicationWindow {
            set_title: Some("Just Wifi Menu"),
            set_default_width: 400,
            set_default_height: 500,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 12,

                // Cabeçalho: título + spinner + botão de scan + switch Wi-Fi
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,

                    gtk::Label {
                        #[watch]
                        set_label: &if model.interface.is_empty() {
                            "Wi-Fi".to_string()
                        } else {
                            format!("Wi-Fi ({})", model.interface)
                        },
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

                // Barra de status
                gtk::Label {
                    #[watch]
                    set_label: &model.status_text,
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                },

                // Lista rolável gerenciada por FactoryVecDeque
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                    #[local_ref]
                    networks_box -> gtk::ListBox {
                        set_selection_mode: gtk::SelectionMode::None,
                    }
                },

                // Painel expansível para digitar senha
                gtk::Revealer {
                    #[watch]
                    set_reveal_child: model.pending_network.is_some(),

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

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
        let networks = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                NetworkRowOutput::Connect(net) => AppMsg::SelectNetwork(net),
                NetworkRowOutput::Disconnect => AppMsg::Disconnect,
                NetworkRowOutput::Forget(ssid) => AppMsg::Forget(ssid),
            });

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        spawn_dbus_worker(sender.clone(), cmd_rx);

        let password_entry = gtk::PasswordEntry::new();

        let model = Self {
            interface: String::new(),
            wifi_enabled: false,
            scanning: false,
            status_text: "Conectando ao NetworkManager...".to_string(),
            pending_network: None,
            networks,
            password_entry: password_entry.clone(),
            cmd_tx,
        };

        let networks_box = model.networks.widget();
        let password_entry = &model.password_entry;
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
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
            AppMsg::Disconnect => {
                self.status_text = "Desconectando...".to_string();
                let _ = self.cmd_tx.send(BackendCmd::Disconnect);
            }
            AppMsg::Forget(ssid) => {
                self.status_text = format!("Removendo perfil \"{ssid}\"...");
                let _ = self.cmd_tx.send(BackendCmd::Forget(ssid));
            }
            AppMsg::BackendState {
                interface,
                wifi_enabled,
                networks,
            } => {
                self.interface = interface;
                self.wifi_enabled = wifi_enabled;

                let mut guard = self.networks.guard();
                guard.clear();
                if wifi_enabled {
                    for net in networks {
                        guard.push_back(net);
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

pub fn run_gui() {
    let app = RelmApp::new("com.just.wifi.menu");
    app.run::<WifiApp>(());
}
