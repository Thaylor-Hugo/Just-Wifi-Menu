mod dbus;

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let connection = dbus::connect_system().await?;

    let Some((device_path, iface_name)) = dbus::wifi::find_wifi_device(&connection).await? else {
        println!("Nenhuma interface Wi-Fi encontrada no NetworkManager.");
        return Ok(());
    };

    println!("Interface Wi-Fi encontrada: {iface_name} ({device_path})");

    let networks = dbus::wifi::list_networks(&connection, &device_path).await?;
    println!("\nRedes disponíveis ({}):", networks.len());
    println!("{:<32} {:>5}  {:>6}  SEGURANÇA", "SSID", "SINAL", "FREQ");
    println!("{}", "-".repeat(60));

    for net in &networks {
        let band = if net.frequency >= 5925 {
            "6GHz"
        } else if net.frequency >= 4900 {
            "5GHz"
        } else {
            "2.4GHz"
        };
        let lock = if net.secured { "Protegida" } else { "Aberta" };
        println!(
            "{:<32} {:>4}%  {:>6}  {}",
            net.ssid, net.strength, band, lock
        );
    }

    Ok(())
}
