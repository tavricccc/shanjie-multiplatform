// map-model INPUT OUTPUT CLASSES|-   maps INPUT to OUTPUT (.winmap) and proves the mapping exact; run it
//                                     before any side file is beside OUTPUT, so both sides score alike.
// map-model --check MAP               loads MAP with every side file beside it, as the engine does.
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 && args[1] == "--check" {
        let map = std::path::Path::new(&args[2]);
        // Lm::load checks kn.sjkn and both trigram tables against the model.
        let lm = core::lm::Lm::load(map).expect("model with its side files");
        let tiny = map.with_file_name("tinylm.sjtl");
        if tiny.exists() { core::tinylm::TinyLm::load_paired(&tiny, &lm).expect("semantic model pairing"); }
        return;
    }
    assert_eq!(args.len(),4,"map-model INPUT OUTPUT CLASSES|- (explicit class-less v1/v2), or map-model --check MAP");
    let output = std::path::Path::new(&args[2]);
    for side in ["kn.sjkn","trigram.sjt3","trigram-chat.sjt3"] {
        assert!(!output.with_file_name(side).exists(),"map before {side} is beside the output");
    }
    let bytes = std::fs::read(&args[1]).expect("source model");
    let classes = if args[3] == "-" { None } else { Some(std::fs::read(&args[3]).expect("word classes")) };
    let source = match &classes {
        Some(table) => core::lm::Lm::parse_with_classes(&bytes,table).expect("model/class pairing"),
        None => core::lm::Lm::parse(&bytes).expect("explicit class-less model"),
    };
    source.write_windows_map(output,&bytes,classes.as_deref()).expect("mapped model");
    let mapped = core::lm::Lm::load(output).expect("verify mapping");
    assert!(source.windows_map_matches(&mapped),"mapping must preserve every model array");
    for word in ["你好","十分鐘","臺灣","後","世界"] {
        assert_eq!(source.count(word),mapped.count(word));
        assert_eq!(source.word(0.5,"今天",word,-5.0).to_bits(),mapped.word(0.5,"今天",word,-5.0).to_bits());
    }
}
