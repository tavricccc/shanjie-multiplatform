#![windows_subsystem="windows"]
fn main(){
    // Only this owned process changes its DPI mode, before any HWND or UIA work.
    if let Err(e)=shanjie_windows::word_helper::initialize_dpi(){eprintln!("{e}");std::process::exit(1);}
    let args:Vec<_>=std::env::args().collect();
    if args.get(1).map(String::as_str)==Some("--fixture-hang"){
        assert_eq!(std::env::var("SHANJIE_TEST_PROFILE").as_deref(),Ok("1"));
        std::thread::sleep(std::time::Duration::from_secs(60));return;
    }
    let result=if args.get(1).map(String::as_str)==Some("--fixture-display"){
        shanjie_windows::word_helper::display_fixture(args.get(2).and_then(|s|s.parse().ok()).unwrap_or(0),
            args.get(3).and_then(|s|s.parse().ok()).unwrap_or(0))
    }else if args.get(1).map(String::as_str)==Some("--fixture-capture"){
        shanjie_windows::word_helper::capture_fixture(args.get(2).and_then(|s|s.parse().ok()).unwrap_or(0),
            args.get(3).and_then(|s|s.parse().ok()).unwrap_or(0))
    }else if args.get(1).map(String::as_str)==Some("--capture"){
        shanjie_windows::word_helper::capture(args.get(2).and_then(|s|s.parse().ok()).unwrap_or(0),
            args.get(3).and_then(|s|s.parse().ok()).unwrap_or(0))
    }else{
        shanjie_windows::word_helper::serve(args.get(2).and_then(|s|s.parse().ok()).unwrap_or(0))
    };
    if let Err(e)=result{eprintln!("{e}");std::process::exit(1);}
}
