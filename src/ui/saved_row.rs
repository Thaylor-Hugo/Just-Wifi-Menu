use std::time::{SystemTime, UNIX_EPOCH};

use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::dbus::wifi::SavedNetworkInfo;
use crate::i18n::{self, Language};

#[derive(Debug, Clone)]
pub struct SavedRowInit {
    pub info: SavedNetworkInfo,
    pub lang: Language,
    pub expanded: bool,
}

#[derive(Debug, Clone)]
pub struct SavedRow {
    pub info: SavedNetworkInfo,
    pub lang: Language,
    pub expanded: bool,
}

#[derive(Debug)]
pub enum SavedRowMsg {
    ToggleExpandClicked,
    SetLanguage(Language),
    SetExpanded(bool),
    ToggleAutoconnect(bool),
}

#[derive(Debug)]
pub enum SavedRowOutput {
    ToggleExpand(String),
    SetAutoconnect { ssid: String, enabled: bool },
    Forget(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for SavedRow {
    type Init = SavedRowInit;
    type Input = SavedRowMsg;
    type Output = SavedRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 6,
            set_margin_top: 6,
            set_margin_bottom: 6,
            set_margin_start: 8,
            set_margin_end: 8,
            add_css_class: "wifi-row",

            gtk::Button {
                set_has_frame: false,
                add_css_class: "wifi-row-expand-btn",
                connect_clicked => SavedRowMsg::ToggleExpandClicked,

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

                    gtk::Image {
                        set_icon_name: Some("network-wireless-symbolic"),
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_hexpand: true,
                        set_spacing: 2,

                        gtk::Label {
                            set_label: &self.info.ssid,
                            set_halign: gtk::Align::Start,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "wifi-ssid",
                        },

                        gtk::Label {
                            set_label: &self.info.security_label,
                            set_halign: gtk::Align::Start,
                            add_css_class: "wifi-subtitle",
                        },
                    },
                },
            },

            gtk::Revealer {
                #[watch]
                set_reveal_child: self.expanded,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 6,
                    add_css_class: "wifi-details-box",

                    gtk::Label {
                        #[watch]
                        set_label: &saved_meta_text(&self.info, self.lang),
                        set_halign: gtk::Align::Start,
                        set_wrap: true,
                        add_css_class: "wifi-subtitle",
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::CheckButton {
                            #[watch]
                            set_label: Some(i18n::strings(self.lang).autoconnect),
                            set_hexpand: true,
                            #[watch]
                            set_active: self.info.autoconnect,
                            connect_toggled[sender] => move |btn| {
                                sender.input(SavedRowMsg::ToggleAutoconnect(btn.is_active()));
                            },
                        },

                        gtk::Button {
                            set_icon_name: "user-trash-symbolic",
                            #[watch]
                            set_label: i18n::strings(self.lang).forget,
                            add_css_class: "wifi-forget-btn",
                            connect_clicked[sender, ssid = self.info.ssid.clone()] => move |_| {
                                let _ = sender.output(SavedRowOutput::Forget(ssid.clone()));
                            },
                        },
                    },
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            info: init.info,
            lang: init.lang,
            expanded: init.expanded,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            SavedRowMsg::ToggleExpandClicked => {
                let _ = sender.output(SavedRowOutput::ToggleExpand(self.info.ssid.clone()));
            }
            SavedRowMsg::SetLanguage(lang) => {
                self.lang = lang;
            }
            SavedRowMsg::SetExpanded(expanded) => {
                self.expanded = expanded;
            }
            SavedRowMsg::ToggleAutoconnect(enabled) => {
                if self.info.autoconnect == enabled {
                    return;
                }
                self.info.autoconnect = enabled;
                let _ = sender.output(SavedRowOutput::SetAutoconnect {
                    ssid: self.info.ssid.clone(),
                    enabled,
                });
            }
        }
    }
}

fn saved_meta_text(info: &SavedNetworkInfo, lang: Language) -> String {
    let s = i18n::strings(lang);
    format!(
        "{}: {} • {}: {}",
        s.last_connected_label,
        i18n::format_last_connected_i18n(lang, info.timestamp, now_unix_secs()),
        s.hidden_label,
        if info.hidden { s.yes } else { s.no }
    )
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
