mod bus;
mod engine;
mod keys;
mod profile;
mod serial;
mod session;

use bus::{Factory, Maker, Session, Waker};
use std::sync::Arc;
use zbus::{connection, zvariant::OwnedValue};

const COMPONENT: &str = "org.freedesktop.IBus.Shanjie";
const ENGINE: &str = "shanjie";

/// The bus address: IBUS_ADDRESS when the daemon or the caller sets it, otherwise `ibus address`.
fn address() -> Result<String, Box<dyn std::error::Error>> {
    if let Ok(address) = std::env::var("IBUS_ADDRESS") {
        if !address.is_empty() {
            return Ok(address);
        }
    }
    let output = std::process::Command::new("ibus").arg("address").output()?;
    let address = String::from_utf8(output.stdout)?.trim().to_owned();
    if !output.status.success() || address.is_empty() || address == "(null)" {
        return Err("ibus-daemon is not running".into());
    }
    Ok(address)
}

/// Loads the shared lexicon and model once at startup, as the Windows resident core does at login,
/// so the first key of every context finds them in the process-wide cache.
fn warm(data: std::path::PathBuf, prefs: shanjie_shell::profile::Document) {
    std::thread::spawn(move || {
        let (config, _) = session::config(&data, &prefs);
        if engine::Engine::open(&config).is_none() {
            eprintln!("ibus-engine-shanjie: the core did not open with {}", data.display());
        }
    });
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ibus-engine-shanjie: {error:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launched_by_daemon = std::env::args().any(|a| a == "--ibus");
    futures_lite::future::block_on(async {
        let data = profile::data();
        let prefs = profile::load(&profile::path()).unwrap_or_else(|error| {
            eprintln!("ibus-engine-shanjie: profile.ini unreadable ({error}); using the Windows template");
            shanjie_shell::profile::Document::parse(profile::TEMPLATE).expect("bundled template")
        });
        warm(data.clone(), prefs.clone());
        let maker: Maker = Arc::new(move |_waker: Waker| Box::new(session::Shanjie::new(data.clone(), prefs.clone())) as Box<dyn Session>);
        let address = address()?;
        let connection = connection::Builder::address(address.as_str())?
            .serve_at("/org/freedesktop/IBus/Factory", Factory::new(ENGINE, maker))?
            .build()
            .await?;
        if !launched_by_daemon {
            // A development run: describe the component to the daemon ourselves.
            let exe = std::env::current_exe()?.display().to_string();
            let component: OwnedValue = serial::Identity {
                component: COMPONENT,
                engine: ENGINE,
                longname: "善解注音",
                description: "善解注音輸入法",
                version: env!("CARGO_PKG_VERSION"),
                exec: &format!("{exe} --ibus"),
                symbol: "善",
                setup: "",
            }
            .component();
            connection
                .call_method(Some("org.freedesktop.IBus"), "/org/freedesktop/IBus", Some("org.freedesktop.IBus"), "RegisterComponent", &(component,))
                .await?;
        }
        connection.request_name(COMPONENT).await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}
