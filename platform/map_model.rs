fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(args.len(),3,"map-model INPUT OUTPUT");
    let source = core::lm::Lm::load(std::path::Path::new(&args[1])).expect("source model");
    source.write_windows_map(std::path::Path::new(&args[2])).expect("mapped model");
    let mapped = core::lm::Lm::load(std::path::Path::new(&args[2])).expect("verify mapping");
    assert!(source.windows_map_matches(&mapped),"mapping must preserve every model array");
    for word in ["你好","十分鐘","臺灣","後","世界"] {
        assert_eq!(source.count(word),mapped.count(word));
        assert_eq!(source.word(0.5,"今天",word,-5.0).to_bits(),mapped.word(0.5,"今天",word,-5.0).to_bits());
    }
}
