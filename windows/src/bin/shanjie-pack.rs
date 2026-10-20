//! Build-time tool, not shipped: shanjie-pack <stub.exe> <package folder> <output.exe>
//! appends the package to the setup stub, making the single-file installer.
use std::{fs, io::Write, path::Path};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [stub, package, output] = args.as_slice() else {
        eprintln!("用法：shanjie-pack <stub.exe> <package folder> <output.exe>");
        std::process::exit(2);
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut out = fs::File::create(output)?;
        out.write_all(&fs::read(stub)?)?;
        shanjie_windows::setup_bundle::pack(Path::new(package), &mut out)?;
        out.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("shanjie-pack: {error}");
        std::process::exit(1);
    }
}
