#![windows_subsystem = "windows"]
use shanjie_windows::broker;
fn main() {
    let data = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("data");
    let result = match std::env::args().nth(1).as_deref() {
        Some("--status") => broker::control(&data, false).and_then(|r| {
            println!("{}", serde_json::to_string(&r)?);
            Ok(())
        }),
        Some("--stop") => broker::control(&data, true).map(|_| ()),
        Some("--serve") => broker::serve(&data),
        _ => Err(std::io::Error::other("Use --serve, --status or --stop")),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
