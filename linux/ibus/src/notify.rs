//! The settings app's notifications (Windows: the "Shanjie.ProfileChanged.v2" and "Shanjie.Restart.v2"
//! window messages). The engine owns NAME on the session bus; `Changed(kind)` reaches every context.
use crate::bus::{broadcast, Engines};
use zbus::{connection, interface, Connection};

pub const NAME: &str = "io.github.Nanako0129.Shanjie";
pub const PATH: &str = "/io/github/Nanako0129/Shanjie";

struct Notifier {
    ibus: Connection,
    engines: Engines,
}

#[interface(name = "io.github.Nanako0129.Shanjie.Engine")]
impl Notifier {
    /// 0: profile.ini changed; 1: restart (rebuild the core in every context).
    async fn changed(&self, kind: u32) -> zbus::fdo::Result<()> {
        if kind > 1 {
            return Err(zbus::fdo::Error::InvalidArgs(format!("unknown kind {kind}")));
        }
        broadcast(&self.ibus, &self.engines, |session, out| session.notify(kind, out)).await;
        Ok(())
    }
}

/// Keeps the session connection alive for the life of the process.
pub async fn serve(ibus: Connection, engines: Engines) -> zbus::Result<()> {
    let session = connection::Builder::session()?.serve_at(PATH, Notifier { ibus, engines })?.name(NAME)?.build().await?;
    std::mem::forget(session);
    Ok(())
}
