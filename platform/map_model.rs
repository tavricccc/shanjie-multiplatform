fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(args.len(),4,"map-model INPUT OUTPUT CLASSES|- (explicit class-less v1/v2)");
    let bytes = std::fs::read(&args[1]).expect("source model");
    let classes = if args[3] == "-" { None } else { Some(std::fs::read(&args[3]).expect("word classes")) };
    let source = match &classes {
        Some(table) => core::lm::Lm::parse_with_classes(&bytes,table).expect("model/class pairing"),
        None => core::lm::Lm::parse(&bytes).expect("explicit class-less model"),
    };
    source.write_windows_map(std::path::Path::new(&args[2]),&bytes,classes.as_deref()).expect("mapped model");
    let mapped = core::lm::Lm::load(std::path::Path::new(&args[2])).expect("verify mapping");
    assert!(source.windows_map_matches(&mapped),"mapping must preserve every model array");
    for word in ["你好","十分鐘","臺灣","後","世界"] {
        assert_eq!(source.count(word),mapped.count(word));
        assert_eq!(source.word(0.5,"今天",word,-5.0).to_bits(),mapped.word(0.5,"今天",word,-5.0).to_bits());
    }
}
