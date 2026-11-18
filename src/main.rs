mod dbus;

#[tokio::main]
async fn main() -> zbus::Result<()> {
    println!("Just Wifi Menu!");

    let connection = dbus::connect_system().await?;
    println!("Conectado ao System Bus: {:?}", connection.unique_name());

    Ok(())
}
