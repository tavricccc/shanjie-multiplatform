//! The settings app's side of the engine notifications (Windows: posting "Shanjie.ProfileChanged.v2"
//! and "Shanjie.Restart.v2" to every window). The engine serves NAME on the session bus.
use shanjie_shell::profile::Result;

pub const NAME: &str = "io.github.Nanako0129.Shanjie";
pub const PATH: &str = "/io/github/Nanako0129/Shanjie";
pub const INTERFACE: &str = "io.github.Nanako0129.Shanjie.Engine";

pub const CHANGED: u32 = 0;
pub const RESTART: u32 = 1;

/// Tells a running engine to reload profile.ini (CHANGED) or rebuild its core (RESTART). Like a
/// broadcast with no listener, nothing to tell is not an error: the next engine reads the file.
pub fn send(kind: u32) -> Result<()> {
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref() == Ok("1") {
        return Ok(());
    }
    let bus = zbus::blocking::Connection::session()?;
    match bus.call_method(Some(NAME), PATH, Some(INTERFACE), "Changed", &(kind,)) {
        Ok(_) => Ok(()),
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" || name.as_str() == "org.freedesktop.DBus.Error.NameHasNoOwner" =>
        {
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
