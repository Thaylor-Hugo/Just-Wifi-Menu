use std::collections::HashMap;

use futures::StreamExt;
use zbus::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::proxies::{
    AccessPointProxy, DeviceProxy, IP4ConfigProxy, NetworkManagerProxy, SettingsConnectionProxy,
    SettingsProxy, WirelessDeviceProxy,
};

/// Valor de `NMDeviceType` para interfaces Wi-Fi (802.11).
const NM_DEVICE_TYPE_WIFI: u32 = 2;

/// Tipo de segurança para conexão manual (redes ocultas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSecurity {
    Open,
    WpaPsk,
    Sae,
}

/// Representa uma rede Wi-Fi agrupada por SSID.
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    pub frequency: u32,
    pub connected: bool,
    pub saved: bool,
    pub path: OwnedObjectPath,
}

/// Detalhes contextuais exibidos ao expandir uma rede na lista.
#[derive(Debug, Clone)]
pub struct NetworkDetails {
    pub ssid: String,
    pub ipv4: Option<String>,
    pub gateway: Option<String>,
    pub autoconnect: Option<bool>,
    pub security_label: String,
}

/// Informações de um perfil Wi-Fi salvo no NetworkManager.
#[derive(Debug, Clone)]
pub struct SavedNetworkInfo {
    pub ssid: String,
    pub security_label: String,
    pub autoconnect: bool,
    pub hidden: bool,
    pub timestamp: u64,
}

/// Centraliza operações de Wi-Fi sobre uma interface específica do NetworkManager.
pub struct WifiManager {
    connection: Connection,
    pub device_path: OwnedObjectPath,
    pub interface: String,
}

impl WifiManager {
    /// Procura dinamicamente a primeira interface Wi-Fi disponível no NetworkManager.
    pub async fn new(connection: Connection) -> zbus::Result<Option<Self>> {
        let nm = NetworkManagerProxy::new(&connection).await?;
        let device_paths = nm.get_devices().await?;

        for path in device_paths {
            let device = DeviceProxy::builder(&connection)
                .path(path.as_str())?
                .build()
                .await?;

            if device.device_type().await? == NM_DEVICE_TYPE_WIFI {
                let interface = device.interface().await?;
                return Ok(Some(Self {
                    connection,
                    device_path: path,
                    interface,
                }));
            }
        }

        Ok(None)
    }

    /// Indica se o rádio Wi-Fi está ligado globalmente no NetworkManager.
    pub async fn is_wireless_enabled(&self) -> zbus::Result<bool> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        nm.wireless_enabled().await
    }

    /// Liga ou desliga o rádio Wi-Fi globalmente.
    pub async fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        nm.set_wireless_enabled(enabled).await
    }

    /// Solicita um novo scan de redes na placa Wi-Fi.
    pub async fn request_scan(&self) -> zbus::Result<()> {
        let wireless = WirelessDeviceProxy::builder(&self.connection)
            .path(self.device_path.as_str())?
            .build()
            .await?;
        wireless.request_scan(HashMap::new()).await
    }

    /// Lista os Access Points visíveis, agrupados por SSID e marcados com `connected` / `saved`.
    pub async fn list_networks(&self) -> zbus::Result<Vec<WifiNetwork>> {
        let wireless = WirelessDeviceProxy::builder(&self.connection)
            .path(self.device_path.as_str())?
            .build()
            .await?;

        let active_ap_path = wireless.active_access_point().await?;
        let saved_map = self.saved_connections_by_ssid().await?;
        let ap_paths = wireless.get_all_access_points().await?;

        let mut by_ssid: HashMap<String, WifiNetwork> = HashMap::new();

        for ap_path in ap_paths {
            let ap = AccessPointProxy::builder(&self.connection)
                .path(ap_path.as_str())?
                .build()
                .await?;

            let ssid_bytes = ap.ssid().await?;
            let Some(ssid) = decode_ssid(&ssid_bytes) else {
                continue;
            };

            let strength = ap.strength().await?;
            let frequency = ap.frequency().await?;
            let flags = ap.flags().await?;
            let wpa_flags = ap.wpa_flags().await?;
            let rsn_flags = ap.rsn_flags().await?;

            let secured = is_secured(flags, wpa_flags, rsn_flags);
            let connected = ap_path == active_ap_path;
            let saved = saved_map.contains_key(&ssid);

            let candidate = WifiNetwork {
                ssid: ssid.clone(),
                strength,
                secured,
                frequency,
                connected,
                saved,
                path: ap_path,
            };

            by_ssid
                .entry(ssid)
                .and_modify(|existing| {
                    // Se qualquer AP desse SSID for o ativo, preservamos `connected = true`.
                    let is_connected = existing.connected || candidate.connected;
                    if candidate.connected || (!existing.connected && candidate.strength > existing.strength) {
                        *existing = candidate.clone();
                    }
                    existing.connected = is_connected;
                })
                .or_insert(candidate);
        }

        let mut networks: Vec<WifiNetwork> = by_ssid.into_values().collect();
        sort_networks(&mut networks);

        Ok(networks)
    }

    /// Conecta a uma rede por SSID. Usa o perfil salvo se existir; caso contrário, cria um novo.
    pub async fn connect(
        &self,
        network: &WifiNetwork,
        password: Option<&str>,
    ) -> zbus::Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        let saved_map = self.saved_connections_by_ssid().await?;

        if let Some(saved_path) = saved_map.get(&network.ssid) {
            nm.activate_connection(saved_path, &self.device_path, &network.path)
                .await?;
            return Ok(());
        }

        let mut conn_section: HashMap<&str, Value<'_>> = HashMap::new();
        conn_section.insert("type", Value::from("802-11-wireless"));
        conn_section.insert("id", Value::from(network.ssid.as_str()));

        let mut wifi_section: HashMap<&str, Value<'_>> = HashMap::new();
        wifi_section.insert("ssid", Value::from(network.ssid.as_bytes().to_vec()));
        wifi_section.insert("mode", Value::from("infrastructure"));

        let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
        settings.insert("connection", conn_section);
        settings.insert("802-11-wireless", wifi_section);

        if network.secured
            && let Some(psk) = password
        {
            let mut sec_section: HashMap<&str, Value<'_>> = HashMap::new();
            sec_section.insert("key-mgmt", Value::from("wpa-psk"));
            sec_section.insert("psk", Value::from(psk));
            settings.insert("802-11-wireless-security", sec_section);
        }

        nm.add_and_activate_connection(settings, &self.device_path, &network.path)
            .await?;

        Ok(())
    }

    /// Conecta manualmente a uma rede (ex: rede oculta) sem precisar de um AP visível no scan.
    pub async fn connect_hidden(
        &self,
        ssid: &str,
        security: WifiSecurity,
        password: Option<&str>,
        hidden: bool,
    ) -> zbus::Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        let saved_map = self.saved_connections_by_ssid().await?;
        let root_obj = ObjectPath::try_from("/").map_err(zbus::Error::Variant)?;

        if let Some(saved_path) = saved_map.get(ssid) {
            nm.activate_connection(saved_path, &self.device_path, &root_obj)
                .await?;
            return Ok(());
        }

        let mut conn_section: HashMap<&str, Value<'_>> = HashMap::new();
        conn_section.insert("type", Value::from("802-11-wireless"));
        conn_section.insert("id", Value::from(ssid));

        let mut wifi_section: HashMap<&str, Value<'_>> = HashMap::new();
        wifi_section.insert("ssid", Value::from(ssid.as_bytes().to_vec()));
        wifi_section.insert("mode", Value::from("infrastructure"));
        if hidden {
            wifi_section.insert("hidden", Value::from(true));
        }

        let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
        settings.insert("connection", conn_section);
        settings.insert("802-11-wireless", wifi_section);

        match security {
            WifiSecurity::Open => {}
            WifiSecurity::WpaPsk => {
                let mut sec_section: HashMap<&str, Value<'_>> = HashMap::new();
                sec_section.insert("key-mgmt", Value::from("wpa-psk"));
                sec_section.insert("psk", Value::from(password.unwrap_or("")));
                settings.insert("802-11-wireless-security", sec_section);
            }
            WifiSecurity::Sae => {
                let mut sec_section: HashMap<&str, Value<'_>> = HashMap::new();
                sec_section.insert("key-mgmt", Value::from("sae"));
                sec_section.insert("psk", Value::from(password.unwrap_or("")));
                settings.insert("802-11-wireless-security", sec_section);
            }
        }

        nm.add_and_activate_connection(settings, &self.device_path, &root_obj)
            .await?;

        Ok(())
    }

    /// Desconecta a interface Wi-Fi atual.
    pub async fn disconnect(&self) -> zbus::Result<()> {
        let device = DeviceProxy::builder(&self.connection)
            .path(self.device_path.as_str())?
            .build()
            .await?;
        device.disconnect().await
    }

    /// Remove o perfil salvo de um SSID no NetworkManager.
    /// Retorna `true` se encontrou e removeu algum perfil.
    pub async fn forget_network(&self, ssid: &str) -> zbus::Result<bool> {
        let saved_map = self.saved_connections_by_ssid().await?;
        let Some(conn_path) = saved_map.get(ssid) else {
            return Ok(false);
        };

        let conn_proxy = SettingsConnectionProxy::builder(&self.connection)
            .path(conn_path.as_str())?
            .build()
            .await?;
        conn_proxy.delete().await?;
        Ok(true)
    }

    /// Coleta detalhes contextuais de uma rede (IPv4/Gateway se conectada, autoconnect se salva).
    pub async fn network_details(&self, network: &WifiNetwork) -> zbus::Result<NetworkDetails> {
        let mut ipv4 = None;
        let mut gateway = None;

        if network.connected {
            let device = DeviceProxy::builder(&self.connection)
                .path(self.device_path.as_str())?
                .build()
                .await?;
            let ip4_path = device.ip4_config().await?;
            if ip4_path.as_str() != "/" {
                let ip4 = IP4ConfigProxy::builder(&self.connection)
                    .path(ip4_path.as_str())?
                    .build()
                    .await?;
                if let Ok(addrs) = ip4.address_data().await
                    && let Some(first) = addrs.first()
                    && let (Some(addr_val), Some(prefix_val)) =
                        (first.get("address"), first.get("prefix"))
                    && let (Ok(addr), Ok(prefix)) = (
                        String::try_from(addr_val.try_clone()?),
                        u32::try_from(prefix_val.try_clone()?),
                    )
                {
                    ipv4 = Some(format!("{addr}/{prefix}"));
                }

                if let Ok(gw) = ip4.gateway().await
                    && !gw.is_empty()
                {
                    gateway = Some(gw);
                }
            }
        }

        let mut autoconnect = None;
        let mut security_label = if network.secured {
            "Protegida (WPA/WPA2/WPA3)".to_string()
        } else {
            "Aberta".to_string()
        };

        if network.saved {
            let saved_map = self.saved_connections_by_ssid().await?;
            if let Some(conn_path) = saved_map.get(&network.ssid) {
                let conn_proxy = SettingsConnectionProxy::builder(&self.connection)
                    .path(conn_path.as_str())?
                    .build()
                    .await?;
                if let Ok(settings) = conn_proxy.get_settings().await {
                    let auto = settings
                        .get("connection")
                        .and_then(|c| c.get("autoconnect"))
                        .and_then(|v| bool::try_from(v.try_clone().ok()?).ok())
                        .unwrap_or(true);
                    autoconnect = Some(auto);

                    if let Some(km) = settings
                        .get("802-11-wireless-security")
                        .and_then(|s| s.get("key-mgmt"))
                        .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
                    {
                        security_label = format_key_mgmt(&km).to_string();
                    }
                }
            }
        }

        Ok(NetworkDetails {
            ssid: network.ssid.clone(),
            ipv4,
            gateway,
            autoconnect,
            security_label,
        })
    }

    /// Busca a senha salva de um perfil Wi-Fi sob demanda via `GetSecrets`.
    pub async fn get_saved_password(&self, ssid: &str) -> zbus::Result<Option<String>> {
        let saved_map = self.saved_connections_by_ssid().await?;
        let Some(conn_path) = saved_map.get(ssid) else {
            return Ok(None);
        };

        let conn_proxy = SettingsConnectionProxy::builder(&self.connection)
            .path(conn_path.as_str())?
            .build()
            .await?;

        let secrets = conn_proxy.get_secrets("802-11-wireless-security").await?;
        let Some(sec) = secrets.get("802-11-wireless-security") else {
            return Ok(None);
        };

        let key = sec.get("psk").or_else(|| sec.get("wep-key0"));
        let Some(val) = key else {
            return Ok(None);
        };

        Ok(String::try_from(val.try_clone()?).ok())
    }

    /// Atualiza `connection.autoconnect` no perfil salvo de um SSID.
    pub async fn set_autoconnect(&self, ssid: &str, enabled: bool) -> zbus::Result<()> {
        let saved_map = self.saved_connections_by_ssid().await?;
        let Some(conn_path) = saved_map.get(ssid) else {
            return Ok(());
        };

        let conn_proxy = SettingsConnectionProxy::builder(&self.connection)
            .path(conn_path.as_str())?
            .build()
            .await?;

        let mut settings = conn_proxy.get_settings().await?;
        let conn_section = settings.entry("connection".to_string()).or_default();
        conn_section.insert("autoconnect".to_string(), OwnedValue::from(enabled));
        conn_proxy.update(settings).await
    }

    /// Lista todos os perfis Wi-Fi salvos no NetworkManager (mesmo fora de alcance).
    pub async fn list_saved_networks(&self) -> zbus::Result<Vec<SavedNetworkInfo>> {
        let settings = SettingsProxy::new(&self.connection).await?;
        let paths = settings.list_connections().await?;
        let mut list = Vec::new();

        for path in paths {
            let conn = SettingsConnectionProxy::builder(&self.connection)
                .path(path.as_str())?
                .build()
                .await?;

            let Ok(mut data) = conn.get_settings().await else {
                continue;
            };

            let Some(mut wifi_section) = data.remove("802-11-wireless") else {
                continue;
            };

            let Some(ssid_value) = wifi_section.remove("ssid") else {
                continue;
            };

            let Ok(bytes) = Vec::<u8>::try_from(ssid_value) else {
                continue;
            };
            let Some(ssid) = decode_ssid(&bytes) else {
                continue;
            };

            let hidden = wifi_section
                .get("hidden")
                .and_then(|v| bool::try_from(v.try_clone().ok()?).ok())
                .unwrap_or(false);

            let conn_section = data.get("connection");
            let autoconnect = conn_section
                .and_then(|c| c.get("autoconnect"))
                .and_then(|v| bool::try_from(v.try_clone().ok()?).ok())
                .unwrap_or(true);
            let timestamp = conn_section
                .and_then(|c| c.get("timestamp"))
                .and_then(|v| u64::try_from(v.try_clone().ok()?).ok())
                .unwrap_or(0);

            let security_label = data
                .get("802-11-wireless-security")
                .and_then(|s| s.get("key-mgmt"))
                .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
                .map(|km| format_key_mgmt(&km))
                .unwrap_or("Aberta")
                .to_string();

            list.push(SavedNetworkInfo {
                ssid,
                security_label,
                autoconnect,
                hidden,
                timestamp,
            });
        }

        list.sort_by(|a, b| {
            b.timestamp
                .cmp(&a.timestamp)
                .then_with(|| a.ssid.cmp(&b.ssid))
        });

        Ok(list)
    }

    /// Escuta sinais do NetworkManager em tempo real até Ctrl+C.
    pub async fn watch_events(&self) -> zbus::Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        let wireless = WirelessDeviceProxy::builder(&self.connection)
            .path(self.device_path.as_str())?
            .build()
            .await?;
        let device = DeviceProxy::builder(&self.connection)
            .path(self.device_path.as_str())?
            .build()
            .await?;

        let mut ap_added = wireless.receive_access_point_added().await?;
        let mut ap_removed = wireless.receive_access_point_removed().await?;
        let mut wireless_changed = nm.receive_wireless_enabled_changed().await;
        let mut active_ap_changed = wireless.receive_active_access_point_changed().await;
        let mut device_state_changed = device.receive_state_changed().await;

        println!("Escutando eventos D-Bus em {} (Ctrl+C para sair)...", self.interface);

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("\nEncerrando monitoramento.");
                    break;
                }
                Some(sig) = ap_added.next() => {
                    if let Ok(args) = sig.args() {
                        println!("[+] AccessPointAdded: {}", args.access_point);
                    }
                }
                Some(sig) = ap_removed.next() => {
                    if let Ok(args) = sig.args() {
                        println!("[-] AccessPointRemoved: {}", args.access_point);
                    }
                }
                Some(change) = wireless_changed.next() => {
                    if let Ok(enabled) = change.get().await {
                        println!("[*] WirelessEnabled mudou para: {enabled}");
                    }
                }
                Some(change) = active_ap_changed.next() => {
                    if let Ok(ap_path) = change.get().await {
                        println!("[*] ActiveAccessPoint mudou para: {ap_path}");
                    }
                }
                Some(change) = device_state_changed.next() => {
                    if let Ok(state) = change.get().await {
                        println!("[*] Estado do device ({}): {state}", self.interface);
                    }
                }
            }
        }

        Ok(())
    }

    /// Mapeia SSID -> ObjectPath das conexões Wi-Fi já salvas no NetworkManager.
    async fn saved_connections_by_ssid(&self) -> zbus::Result<HashMap<String, OwnedObjectPath>> {
        let settings = SettingsProxy::new(&self.connection).await?;
        let paths = settings.list_connections().await?;
        let mut map = HashMap::new();

        for path in paths {
            let conn = SettingsConnectionProxy::builder(&self.connection)
                .path(path.as_str())?
                .build()
                .await?;

            let Ok(mut data) = conn.get_settings().await else {
                continue;
            };

            let Some(mut wifi_section) = data.remove("802-11-wireless") else {
                continue;
            };

            let Some(ssid_value) = wifi_section.remove("ssid") else {
                continue;
            };

            if let Ok(bytes) = Vec::<u8>::try_from(ssid_value) {
                let ssid = String::from_utf8_lossy(&bytes).into_owned();
                map.insert(ssid, path);
            }
        }

        Ok(map)
    }
}

/// Formata um timestamp Unix (segundos) em tempo relativo legível.
pub fn format_last_connected(timestamp: u64, now_secs: u64) -> String {
    if timestamp == 0 {
        return "Nunca".to_string();
    }

    let diff = now_secs.saturating_sub(timestamp);
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

/// Formata o identificador `key-mgmt` do NetworkManager para exibição.
pub fn format_key_mgmt(key_mgmt: &str) -> &'static str {
    match key_mgmt {
        "wpa-psk" => "WPA/WPA2-PSK",
        "sae" => "WPA3-SAE",
        "wpa-eap" => "WPA-Enterprise",
        "none" => "WEP",
        _ => "Protegida",
    }
}

/// Converte os bytes de um SSID para `String`, retornando `None` se estiver vazio (rede oculta).
pub fn decode_ssid(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Determina se o AP exige autenticação a partir das flags 802.11 do NetworkManager.
pub fn is_secured(flags: u32, wpa_flags: u32, rsn_flags: u32) -> bool {
    (flags & 0x1) != 0 || wpa_flags > 0 || rsn_flags > 0
}

/// Ordena redes: conectada primeiro, depois salvas, depois por maior sinal.
pub fn sort_networks(networks: &mut [WifiNetwork]) {
    networks.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.saved.cmp(&a.saved))
            .then_with(|| b.strength.cmp(&a.strength))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_network(ssid: &str, strength: u8, connected: bool, saved: bool) -> WifiNetwork {
        WifiNetwork {
            ssid: ssid.to_string(),
            strength,
            secured: true,
            frequency: 5180,
            connected,
            saved,
            path: OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/AccessPoint/1")
                .unwrap(),
        }
    }

    #[test]
    fn decode_ssid_handles_empty_and_utf8() {
        assert_eq!(decode_ssid(b""), None);
        assert_eq!(decode_ssid(b"The Force -5G"), Some("The Force -5G".to_string()));
        assert_eq!(decode_ssid(&[0xff, b'A']), Some("\u{FFFD}A".to_string()));
    }

    #[test]
    fn is_secured_checks_privacy_and_wpa_flags() {
        assert!(!is_secured(0, 0, 0));
        assert!(is_secured(0x1, 0, 0));
        assert!(is_secured(0, 0x100, 0));
        assert!(is_secured(0, 0, 0x100));
    }

    #[test]
    fn sort_networks_orders_connected_then_saved_then_strength() {
        let mut nets = vec![
            fake_network("Fraca", 30, false, false),
            fake_network("Salva_Media", 50, false, true),
            fake_network("Forte_Aberta", 95, false, false),
            fake_network("Conectada_Fraca", 40, true, true),
        ];

        sort_networks(&mut nets);

        let ordered: Vec<&str> = nets.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(
            ordered,
            vec!["Conectada_Fraca", "Salva_Media", "Forte_Aberta", "Fraca"]
        );
    }

    #[test]
    fn format_last_connected_formats_relative_times() {
        assert_eq!(format_last_connected(0, 10_000), "Nunca");
        assert_eq!(format_last_connected(9_970, 10_000), "Agora mesmo");
        assert_eq!(format_last_connected(9_400, 10_000), "Há 10 min");
        assert_eq!(format_last_connected(2_800, 10_000), "Há 2 h");
        assert_eq!(format_last_connected(100_000, 359_200), "Há 3 dia(s)");
    }
}
