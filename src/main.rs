
#[tokio::main]
async fn main() -> zbus::Result<()> {
    println!("Hello, world!");

    return Ok(());
}


async fn print_access_points(connection: &zbus::Connection) -> zbus::Result<()> {
    let m = connection.call_method(
        Some("org.freedesktop.NetworkManager"),
        "/org/freedesktop/NetworkManager/Devices/5",
        Some("org.freedesktop.NetworkManager.Device.Wireless"),
        "GetAllAccessPoints",
        &()
    ).await?;

    let reply: Vec<zbus::zvariant::OwnedObjectPath> = m.body().deserialize().unwrap();
    println!("{:?}", reply);

    return Ok(());
}