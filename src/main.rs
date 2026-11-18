mod dbus;

use dbus::wifi::{WifiManager, WifiNetwork};

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let connection = dbus::connect_system().await?;

    let Some(manager) = WifiManager::new(connection).await? else {
        println!("Nenhuma interface Wi-Fi encontrada no NetworkManager.");
        return Ok(());
    };

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("list");

    match command {
        "list" => {
            let enabled = manager.is_wireless_enabled().await?;
            println!(
                "Interface: {} | Wi-Fi: {}",
                manager.interface,
                if enabled { "Ligado" } else { "Desligado" }
            );
            let networks = manager.list_networks().await?;
            print_networks(&networks);
        }
        "scan" => {
            println!("Solicitando scan em {}...", manager.interface);
            manager.request_scan().await?;
            let networks = manager.list_networks().await?;
            print_networks(&networks);
        }
        "toggle" => match args.get(2).map(String::as_str) {
            Some("on") => {
                manager.set_wireless_enabled(true).await?;
                println!("Wi-Fi ligado.");
            }
            Some("off") => {
                manager.set_wireless_enabled(false).await?;
                println!("Wi-Fi desligado.");
            }
            _ => println!("Uso: cargo run -- toggle <on|off>"),
        },
        "connect" => {
            let Some(ssid) = args.get(2) else {
                println!("Uso: cargo run -- connect <ssid> [senha]");
                return Ok(());
            };
            let password = args.get(3).map(String::as_str);
            let networks = manager.list_networks().await?;
            let Some(target) = networks.iter().find(|n| n.ssid == *ssid) else {
                println!("Rede '{ssid}' não encontrada no scan atual.");
                return Ok(());
            };
            manager.connect(target, password).await?;
            println!("Solicitação de conexão enviada para '{ssid}'.");
        }
        "disconnect" => {
            manager.disconnect().await?;
            println!("Interface {} desconectada.", manager.interface);
        }
        "forget" => {
            let Some(ssid) = args.get(2) else {
                println!("Uso: cargo run -- forget <ssid>");
                return Ok(());
            };
            if manager.forget_network(ssid).await? {
                println!("Perfil '{ssid}' removido.");
            } else {
                println!("Nenhum perfil salvo encontrado para '{ssid}'.");
            }
        }
        "watch" => {
            manager.watch_events().await?;
        }
        other => {
            println!("Comando desconhecido: {other}");
            println!("Comandos: list | scan | toggle <on|off> | connect <ssid> [senha] | disconnect | forget <ssid> | watch");
        }
    }

    Ok(())
}

fn print_networks(networks: &[WifiNetwork]) {
    println!("\nRedes disponíveis ({}):", networks.len());
    println!("{:<30} {:>5}  {:>6}  {:<10}  STATUS", "SSID", "SINAL", "FREQ", "SEGURANÇA");
    println!("{}", "-".repeat(72));

    for net in networks {
        let band = if net.frequency >= 5925 {
            "6GHz"
        } else if net.frequency >= 4900 {
            "5GHz"
        } else {
            "2.4GHz"
        };
        let lock = if net.secured { "Protegida" } else { "Aberta" };
        let status = match (net.connected, net.saved) {
            (true, _) => "Conectado",
            (false, true) => "Salvo",
            (false, false) => "-",
        };

        println!(
            "{:<30} {:>4}%  {:>6}  {:<10}  {}",
            net.ssid, net.strength, band, lock, status
        );
    }
}
