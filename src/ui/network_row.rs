use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::dbus::wifi::WifiNetwork;

#[derive(Debug, Clone)]
pub struct NetworkRow {
    pub network: WifiNetwork,
}

#[derive(Debug)]
pub enum NetworkRowMsg {}

#[derive(Debug)]
pub enum NetworkRowOutput {
    Connect(WifiNetwork),
    Disconnect,
    Forget(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for NetworkRow {
    type Init = WifiNetwork;
    type Input = NetworkRowMsg;
    type Output = NetworkRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_top: 6,
            set_margin_bottom: 6,
            set_margin_start: 8,
            set_margin_end: 8,

            gtk::Image {
                set_icon_name: Some(signal_icon_name(self.network.strength, self.network.secured)),
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_hexpand: true,
                set_spacing: 2,

                gtk::Label {
                    set_label: &self.network.ssid,
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                },

                gtk::Label {
                    set_label: &subtitle_text(&self.network),
                    set_halign: gtk::Align::Start,
                },
            },

            gtk::Label {
                set_label: &format!("{}%", self.network.strength),
            },

            gtk::Button {
                set_icon_name: "user-trash-symbolic",
                set_tooltip_text: Some("Esquecer rede salva"),
                set_visible: self.network.saved && !self.network.connected,
                connect_clicked[sender, ssid = self.network.ssid.clone()] => move |_| {
                    let _ = sender.output(NetworkRowOutput::Forget(ssid.clone()));
                },
            },

            gtk::Button {
                set_label: if self.network.connected { "Desconectar" } else { "Conectar" },
                connect_clicked[sender, net = self.network.clone()] => move |_| {
                    let msg = if net.connected {
                        NetworkRowOutput::Disconnect
                    } else {
                        NetworkRowOutput::Connect(net.clone())
                    };
                    let _ = sender.output(msg);
                },
            },
        }
    }

    fn init_model(network: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self { network }
    }
}

fn subtitle_text(net: &WifiNetwork) -> String {
    let band = if net.frequency >= 5925 {
        "6 GHz"
    } else if net.frequency >= 4900 {
        "5 GHz"
    } else {
        "2.4 GHz"
    };

    let state = if net.connected {
        "Conectado"
    } else if net.saved {
        "Salvo"
    } else if net.secured {
        "Protegida"
    } else {
        "Aberta"
    };

    format!("{state} • {band}")
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
