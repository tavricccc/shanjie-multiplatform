#![windows_subsystem="windows"]
fn main(){
    let args:Vec<_>=std::env::args().collect();
    let result=if args.get(1).map(String::as_str)==Some("--capture"){
        shanjie_windows::word_helper::capture(args.get(2).and_then(|s|s.parse().ok()).unwrap_or(0),
            args.get(3).and_then(|s|s.parse().ok()).unwrap_or(0))
    }else{
        shanjie_windows::word_helper::serve(args.get(2).and_then(|s|s.parse().ok()).unwrap_or(0))
    };
    if let Err(e)=result{eprintln!("{e}");std::process::exit(1);}
}
