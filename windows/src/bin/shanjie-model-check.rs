//! Isolated maintenance probe. Never registers the IME or loads an installed profile.
use shanjie_windows::{engine::Engine,broker::{Config,Op}};
use shanjie_core::{engine::Layout,ffi::ShanjieKey};
use serde_json::json;
use std::{path::Path,fs};
fn engine(data:&Path,id:&str,ranking:u32)->Engine{
    let config=Config{model:id.into(),ranking,right:false,prediction:false,..Config::default()};
    Engine::open_local(data,&config).expect("native bundle load")
}
fn key(engine:&mut Engine,kind:u32,ch:char)->shanjie_shell::protocol::Snapshot{
    engine.key(ShanjieKey{kind,ch:ch as u32,modifiers:0}).expect("native input")
}
fn main(){
    let result=std::panic::catch_unwind(run);
    if result.is_err(){eprintln!("Native model probe failed");std::process::exit(1);}
}
fn run(){
    assert_eq!(std::env::var("SHANJIE_TEST_PROFILE").as_deref(),Ok("1"));
    assert!(std::env::var_os("SHANJIE_CORE_LOCAL").is_some());
    let args:Vec<_>=std::env::args().collect();assert!(args.len()>=5);
    let root=Path::new(&args[2]);let id=&args[3];let data=Path::new(&args[4]);
    assert_eq!(root,std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()).join("Shanjie/Models"));
    if args[1]=="prepare" {
        let item=if root.join(".staging").join(id).join("model.json").is_file(){
            shanjie_shell::models::prepare(root,id)
        }else{shanjie_shell::models::checked(&root.join(id))}.expect("source, dependency and mapping validation");
        assert_eq!(item.spec.id,*id);
        let mut e=engine(data,id,0);
        for ch in "su3cl3".chars(){key(&mut e,1,ch);}
        let out=key(&mut e,3,'\0');assert!(!out.commit.is_empty()&&out.preedit.is_empty());
        assert!(key(&mut e,3,'\0').commit.is_empty(),"commit must occur once");
        if item.spec.catalog.as_ref().is_some_and(|m|m.assets.iter().any(|a|a.role=="semantic")){assert!(e.rerank_jobs_run>0,"semantic model must actually run");}
        println!("{}",json!({"usable":true,"mapped_equivalent":true,"source_and_side_pairs_checked":true,"native_input_and_commit":true,"semantic_jobs_run":e.rerank_jobs_run}));
    } else {
        assert_eq!(args[1],"evaluate");let text=fs::read_to_string(&args[5]).unwrap();
        let rows:Vec<_>=text.lines().filter(|l|!l.is_empty()&&!l.starts_with('#')).map(|l|{
            let p:Vec<_>=l.split('|').collect();assert_eq!(p.len(),3);(p[0],p[1],p[2])
        }).collect();
        let mut scores=[0usize;2];
        for ranking in 0..2 {
            let mut e=engine(data,id,ranking);
            for (context,want,reading) in &rows {
                e.reset(true).unwrap();e.perform(Op::Context((*context).into())).unwrap();
                let mut current=String::new();
                for syllable in reading.split_whitespace(){
                    let mut tone=false;
                    for c in syllable.chars(){
                        if let Some(ch)=Layout::Standard.key_of_tone(c){current=key(&mut e,1,ch).preedit;tone=true;}
                        else {let ch=Layout::Standard.key_of_symbol(c).expect("public reading");current=key(&mut e,1,ch).preedit;}
                    }
                    if !tone{current=key(&mut e,2,' ').preedit;}
                }
                if current==*want {scores[ranking as usize]+=1;}
            }
        }
        println!("{}",json!({"state":"evaluated","sample_count":rows.len(),"chat":scores[0],"formal":scores[1]}));
    }
}
