use std::collections::HashMap;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

const NM_BUS: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_IFACE: &str = "org.freedesktop.NetworkManager";
const NM_DEVICE_IFACE: &str = "org.freedesktop.NetworkManager.Device";
const NM_WIRELESS_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const NM_AP_IFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";

/// Valor de `NMDeviceType` para interfaces Wi-Fi (802.11).
const NM_DEVICE_TYPE_WIFI: u32 = 2;

/// Representa uma rede Wi-Fi agrupada por SSID.
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    pub frequency: u32,
    #[allow(dead_code)] // Será usado na Etapa 2 para conectar a um AP específico
    pub path: OwnedObjectPath,
}

/// Procura dinamicamente a primeira interface de rede Wi-Fi no NetworkManager.
/// Retorna `Ok(Some((object_path, nome_da_interface)))` se encontrar.
pub async fn find_wifi_device(
    connection: &Connection,
) -> zbus::Result<Option<(OwnedObjectPath, String)>> {
    let nm_proxy = Proxy::new(connection, NM_BUS, NM_PATH, NM_IFACE).await?;
    let device_paths: Vec<OwnedObjectPath> = nm_proxy.call("GetDevices", &()).await?;

    for path in device_paths {
        let device_proxy = Proxy::new(connection, NM_BUS, path.as_str(), NM_DEVICE_IFACE).await?;
        let device_type: u32 = device_proxy.get_property("DeviceType").await?;

        if device_type == NM_DEVICE_TYPE_WIFI {
            let iface_name: String = device_proxy.get_property("Interface").await?;
            return Ok(Some((path, iface_name)));
        }
    }

    Ok(None)
}

/// Lista os Access Points visíveis pelo device Wi-Fi, agrupados por SSID
/// (mantendo o AP de maior sinal) e ordenados do sinal mais forte ao mais fraco.
pub async fn list_networks(
    connection: &Connection,
    device_path: &OwnedObjectPath,
) -> zbus::Result<Vec<WifiNetwork>> {
    let wireless_proxy =
        Proxy::new(connection, NM_BUS, device_path.as_str(), NM_WIRELESS_IFACE).await?;
    let ap_paths: Vec<OwnedObjectPath> = wireless_proxy.call("GetAllAccessPoints", &()).await?;

    let mut by_ssid: HashMap<String, WifiNetwork> = HashMap::new();

    for ap_path in ap_paths {
        let ap_proxy = Proxy::new(connection, NM_BUS, ap_path.as_str(), NM_AP_IFACE).await?;

        let ssid_bytes: Vec<u8> = ap_proxy.get_property("Ssid").await?;
        if ssid_bytes.is_empty() {
            // Redes ocultas vêm com SSID vazio; serão tratadas em etapas futuras.
            continue;
        }

        let ssid = String::from_utf8_lossy(&ssid_bytes).into_owned();
        let strength: u8 = ap_proxy.get_property("Strength").await?;
        let frequency: u32 = ap_proxy.get_property("Frequency").await?;
        let flags: u32 = ap_proxy.get_property("Flags").await?;
        let wpa_flags: u32 = ap_proxy.get_property("WpaFlags").await?;
        let rsn_flags: u32 = ap_proxy.get_property("RsnFlags").await?;

        // Bit 0x1 em Flags indica privacidade (WEP); WpaFlags/RsnFlags > 0 indica WPA/WPA2/WPA3.
        let secured = (flags & 0x1) != 0 || wpa_flags > 0 || rsn_flags > 0;

        let candidate = WifiNetwork {
            ssid: ssid.clone(),
            strength,
            secured,
            frequency,
            path: ap_path,
        };

        by_ssid
            .entry(ssid)
            .and_modify(|existing| {
                if candidate.strength > existing.strength {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }

    let mut networks: Vec<WifiNetwork> = by_ssid.into_values().collect();
    networks.sort_by(|a, b| b.strength.cmp(&a.strength));

    Ok(networks)
}
