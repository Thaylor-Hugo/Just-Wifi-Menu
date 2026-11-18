pub mod proxies;
pub mod wifi;

/// Abre uma conexão assíncrona com o System Bus do D-Bus.
pub async fn connect_system() -> zbus::Result<zbus::Connection> {
    zbus::Connection::system().await
}
