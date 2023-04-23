use zbus::{block_on, Interface, InterfaceRef, Result};

async fn get_session_connection() -> Result<zbus::Connection> {
    zbus::Connection::session().await
}

// Explicit name because we already have a lot of `Connection` types floating
// around and this is nicer than typing `dbus::Connection`.
/// Synchronous wrapper around `zbus::Connection`
#[derive(Clone)]
pub struct DBusConnection {
    conn: zbus::Connection,
}

impl DBusConnection {
    pub fn new() -> Result<Self> {
        let conn = block_on(get_session_connection())?;
        Ok(Self { conn })
    }

    pub fn register_server(&self, name: &str, path: &str, server: impl Interface) -> Result<()> {
        block_on((|| async {
            self.conn.object_server().at(path, server).await?;
            self.conn.request_name(name).await?;
            Ok(())
        })())
    }

    pub fn remove_server<I: Interface>(&self, name: &str, path: &str) -> Result<()> {
        block_on(async {
            self.conn.object_server().remove::<I, _>(path).await?;
            self.conn.release_name(name).await?;
            Ok(())
        })
    }

    pub fn get_interface<I: Interface>(&self, path: &str) -> InterfaceRef<I> {
        block_on(async {
            self.conn
                .object_server()
                .interface::<_, I>(path)
                .await
                .unwrap()
        })
    }
}
