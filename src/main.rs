mod config;
mod dbus;
mod ui;

use clap::{Args, Parser, Subcommand};
use config::AppConfig;
use dbus::wifi::{WifiManager, WifiNetwork};

#[derive(Parser)]
#[command(name = "Just-Wifi-Menu", about = "Menu Wi-Fi customizável em GTK4 + Relm4")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Abre a interface gráfica (padrão quando nenhum subcomando é passado)
    Gui(GuiArgs),
    /// Lista as redes Wi-Fi disponíveis
    List,
    /// Solicita um novo scan e lista as redes
    Scan,
    /// Liga ou desliga o rádio Wi-Fi
    Toggle {
        /// "on" ou "off"
        state: String,
    },
    /// Conecta a uma rede pelo SSID
    Connect {
        ssid: String,
        password: Option<String>,
    },
    /// Desconecta a interface Wi-Fi atual
    Disconnect,
    /// Remove o perfil salvo de um SSID
    Forget { ssid: String },
    /// Escuta eventos D-Bus do NetworkManager em tempo real
    Watch,
}

#[derive(Args, Default)]
struct GuiArgs {
    /// Posição na tela: top-right, top-left, bottom-right, bottom-left, center
    #[arg(long)]
    anchor: Option<String>,
    #[arg(long)]
    margin_top: Option<i32>,
    #[arg(long)]
    margin_right: Option<i32>,
    #[arg(long)]
    margin_bottom: Option<i32>,
    #[arg(long)]
    margin_left: Option<i32>,
}

fn main() -> zbus::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => launch_gui(GuiArgs::default()),
        Some(Command::Gui(args)) => launch_gui(args),
        Some(cmd) => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(run_cli(cmd))?;
        }
    }

    Ok(())
}

fn launch_gui(args: GuiArgs) {
    // Garante backend Wayland quando WAYLAND_DISPLAY estiver ativo (ex: terminais/IDEs com GDK_BACKEND=x11)
    // para que o gtk4-layer-shell funcione no Hyprland. Chamado antes de iniciar threads/GTK.
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        unsafe {
            std::env::set_var("GDK_BACKEND", "wayland");
        }
    }

    let mut config = AppConfig::load();
    if let Some(anchor) = args.anchor {
        config.anchor = anchor;
    }
    if let Some(top) = args.margin_top {
        config.margin_top = top;
    }
    if let Some(right) = args.margin_right {
        config.margin_right = right;
    }
    if let Some(bottom) = args.margin_bottom {
        config.margin_bottom = bottom;
    }
    if let Some(left) = args.margin_left {
        config.margin_left = left;
    }

    ui::run_gui(config);
}

async fn run_cli(command: Command) -> zbus::Result<()> {
    let connection = dbus::connect_system().await?;

    let Some(manager) = WifiManager::new(connection).await? else {
        println!("Nenhuma interface Wi-Fi encontrada no NetworkManager.");
        return Ok(());
    };

    match command {
        Command::List => {
            let enabled = manager.is_wireless_enabled().await?;
            println!(
                "Interface: {} | Wi-Fi: {}",
                manager.interface,
                if enabled { "Ligado" } else { "Desligado" }
            );
            let networks = manager.list_networks().await?;
            print_networks(&networks);
        }
        Command::Scan => {
            println!("Solicitando scan em {}...", manager.interface);
            manager.request_scan().await?;
            let networks = manager.list_networks().await?;
            print_networks(&networks);
        }
        Command::Toggle { state } => match state.as_str() {
            "on" => {
                manager.set_wireless_enabled(true).await?;
                println!("Wi-Fi ligado.");
            }
            "off" => {
                manager.set_wireless_enabled(false).await?;
                println!("Wi-Fi desligado.");
            }
            _ => println!("Uso: Just-Wifi-Menu toggle <on|off>"),
        },
        Command::Connect { ssid, password } => {
            let networks = manager.list_networks().await?;
            let Some(target) = networks.iter().find(|n| n.ssid == ssid) else {
                println!("Rede '{ssid}' não encontrada no scan atual.");
                return Ok(());
            };
            manager.connect(target, password.as_deref()).await?;
            println!("Solicitação de conexão enviada para '{ssid}'.");
        }
        Command::Disconnect => {
            manager.disconnect().await?;
            println!("Interface {} desconectada.", manager.interface);
        }
        Command::Forget { ssid } => {
            if manager.forget_network(&ssid).await? {
                println!("Perfil '{ssid}' removido.");
            } else {
                println!("Nenhum perfil salvo encontrado para '{ssid}'.");
            }
        }
        Command::Watch => {
            manager.watch_events().await?;
        }
        Command::Gui(_) => unreachable!(),
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
