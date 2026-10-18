mod bus;
mod serial;

use bus::{Effect, Effects, Factory, Maker, Session, Waker};
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

struct Echo {
    text: String,
}

impl Session for Echo {
    fn key(&mut self, keyval: u32, _keycode: u32, state: u32, out: &mut Effects) -> bool {
        if state & (1 << 30) != 0 {
            return false;
        }
        match keyval {
            0x20 if !self.text.is_empty() => {
                out.push(Effect::Commit(std::mem::take(&mut self.text).to_uppercase()));
                out.push(Effect::HidePreedit);
                out.push(Effect::HideLookup);
                true
            }
            0x61..=0x7a => {
                self.text.push(char::from_u32(keyval).unwrap());
                let len = self.text.chars().count() as u32;
                out.push(Effect::Preedit(self.text.clone(), len, vec![(serial::ATTR_UNDERLINE, serial::UNDERLINE_SINGLE, 0, len)]));
                out.push(Effect::Lookup(vec![self.text.to_uppercase(), self.text.clone()], vec!["1".into(), "2".into()], Some(0), false));
                true
            }
            _ => false,
        }
    }
    fn focus_in(&mut self, _out: &mut Effects) {}
    fn focus_out(&mut self, _out: &mut Effects) {}
    fn reset(&mut self, _out: &mut Effects) {
        self.text.clear();
    }
    fn enable(&mut self, _out: &mut Effects) {}
    fn disable(&mut self, _out: &mut Effects) {}
    fn capabilities(&mut self, _caps: u32) {}
    fn surrounding(&mut self, _text: &str, _cursor: u32, _anchor: u32, _out: &mut Effects) {}
    fn candidate_clicked(&mut self, _index: u32, _out: &mut Effects) {}
    fn page(&mut self, _down: bool, _out: &mut Effects) {}
    fn cursor(&mut self, _down: bool, _out: &mut Effects) {}
    fn property(&mut self, _name: &str, _state: u32, _out: &mut Effects) {}
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
        let maker: Maker = Arc::new(|_waker: Waker| Box::new(Echo { text: String::new() }) as Box<dyn Session>);
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
