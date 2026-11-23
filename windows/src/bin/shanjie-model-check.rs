//! Isolated maintenance probe. Never registers the IME or loads an installed profile.
use shanjie_windows::{engine::Engine,broker::{Config,Op}};
use shanjie_core::{engine::Layout,ffi::ShanjieKey};
use sha2::{Digest,Sha256};
use serde_json::json;
use std::{path::Path,fs,time::Instant};
use windows::Win32::System::{
    ProcessStatus::{GetProcessMemoryInfo,PROCESS_MEMORY_COUNTERS,PROCESS_MEMORY_COUNTERS_EX},
    Threading::GetCurrentProcess,
};
fn engine(data:&Path,id:&str,ranking:u32)->Engine{
    let config=Config{model:id.into(),ranking,right:false,prediction:false,..Config::default()};
    Engine::open_local(data,&config).expect("native bundle load")
}
fn key(engine:&mut Engine,kind:u32,ch:char)->shanjie_shell::protocol::Snapshot{
    engine.key(ShanjieKey{kind,ch:ch as u32,modifiers:0}).expect("native input")
}
fn mem_stats()->(f64,f64,f64,f64){
    let mut counters=PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb=std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    unsafe {
        assert!(GetProcessMemoryInfo(GetCurrentProcess(),&mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,counters.cb).is_ok());
    }
    let mib=|v:usize| v as f64 / 1048576.0;
    (mib(counters.PrivateUsage),mib(counters.WorkingSetSize),mib(counters.PeakWorkingSetSize),mib(counters.PeakPagefileUsage))
}
fn percentile_ms(samples:&mut [u64],pct:usize)->f64{
    if samples.is_empty(){return 0.0;}
    samples.sort_unstable();
    let rank=((samples.len()*pct+99)/100).saturating_sub(1).min(samples.len()-1);
    samples[rank] as f64 / 1_000_000.0
}
fn load_rows(path:&str)->Vec<(String,String,String)>{
    fs::read_to_string(path).expect("read row file").lines().filter(|l|!l.is_empty()&&!l.starts_with('#')).map(|l|{
        let p:Vec<_>=l.split('|').collect();assert_eq!(p.len(),3);
        (p[0].to_owned(),p[1].to_owned(),p[2].to_owned())
    }).collect()
}
fn type_row(e:&mut Engine,context:&str,reading:&str,mut key_ns:Option<&mut Vec<u64>>,mut rerank_ns:Option<&mut Vec<u64>>,drain_bg:bool)->String{
    e.reset(true).expect("reset");
    if !context.is_empty(){e.perform(Op::Context(context.to_owned())).expect("context");}
    let mut current=String::new();
    let mut pending_rerank=false;
    for syllable in reading.split_whitespace(){
        let mut tone=false;
        for c in syllable.chars(){
            let ch=if let Some(k)=Layout::Standard.key_of_tone(c){tone=true;k}
                   else{Layout::Standard.key_of_symbol(c).expect("public reading")};
            let t0=Instant::now();
            let snap=key(e,1,ch);
            if let Some(v)=key_ns.as_deref_mut(){v.push(t0.elapsed().as_nanos() as u64);}
            if let (Some(v),Some(ns))=(rerank_ns.as_deref_mut(),e.last_rerank_ns){v.push(ns);}
            current=snap.preedit;pending_rerank=snap.rerank;
        }
        if !tone{
            let t0=Instant::now();
            let snap=key(e,2,' ');
            if let Some(v)=key_ns.as_deref_mut(){v.push(t0.elapsed().as_nanos() as u64);}
            if let (Some(v),Some(ns))=(rerank_ns.as_deref_mut(),e.last_rerank_ns){v.push(ns);}
            current=snap.preedit;pending_rerank=snap.rerank;
        }
    }
    if drain_bg && pending_rerank {
        if let Some(snap)=e.perform(Op::Rerank){current=snap.preedit;}
    }
    current
}
fn run_bench(data:&Path,id:&str,short_path:&str,long_path:&str,ranking:u32){
    let tinylm_path=data.join("models/builtin/tinylm.sjtl");
    let bytes=fs::read(&tinylm_path).expect("read tinylm.sjtl");
    let tinylm_sha=format!("{:x}",Sha256::digest(&bytes));
    let magic=std::str::from_utf8(&bytes[..8]).unwrap_or("?").to_owned();
    let (left_len,flags,boundary_k)=if magic=="SJTL0006" && bytes.len()>=256 {
        (Some(u32::from_le_bytes(bytes[244..248].try_into().unwrap())),
         Some(u32::from_le_bytes(bytes[248..252].try_into().unwrap())),
         Some(u32::from_le_bytes(bytes[252..256].try_into().unwrap())))
    } else if magic=="SJTL0005" && bytes.len()>=248 {
        (Some(u32::from_le_bytes(bytes[244..248].try_into().unwrap())),None,None)
    } else {(None,None,None)};
    let short_rows=load_rows(short_path);
    let long_rows=load_rows(long_path);
    let (before_priv,before_ws,_,_)=mem_stats();
    let t_cold=Instant::now();
    let mut sync_engine=engine(data,id,ranking);
    let cold_open_ms=t_cold.elapsed().as_secs_f64()*1000.0;
    let (cold_priv,cold_ws,_,_)=mem_stats();
    let t_warm=Instant::now();
    let mut bg_engine=engine(data,id,ranking);
    let warm_open_ms=t_warm.elapsed().as_secs_f64()*1000.0;
    bg_engine.rerank_in_background();
    let measure_set=|rows:&[(String,String,String)],bg:&mut Engine,sync:&mut Engine|{
        let mut key_ns=Vec::new();
        for (ctx,_,reading) in rows {
            let _=type_row(bg,ctx,reading,Some(&mut key_ns),None,true);
        }
        let mut rerank_ns=Vec::new();
        let mut top1=0usize;
        for (ctx,want,reading) in rows {
            let got=type_row(sync,ctx,reading,None,Some(&mut rerank_ns),false);
            if got==*want {top1+=1;}
        }
        let key_p50=percentile_ms(&mut key_ns,50);
        let key_p95=percentile_ms(&mut key_ns,95);
        let key_max=percentile_ms(&mut key_ns,100);
        let rr_p50=percentile_ms(&mut rerank_ns,50);
        let rr_p95=percentile_ms(&mut rerank_ns,95);
        let rr_max=percentile_ms(&mut rerank_ns,100);
        json!({
            "rows":rows.len(),"top1":top1,
            "key_samples":key_ns.len(),"key_p50_ms":key_p50,"key_p95_ms":key_p95,"key_max_ms":key_max,
            "rerank_jobs":rerank_ns.len(),"rerank_p50_ms":rr_p50,"rerank_p95_ms":rr_p95,"rerank_max_ms":rr_max
        })
    };
    let short_metrics=measure_set(&short_rows,&mut bg_engine,&mut sync_engine);
    let long_metrics=measure_set(&long_rows,&mut bg_engine,&mut sync_engine);
    let (final_priv,final_ws,peak_ws,peak_pagefile)=mem_stats();
    println!("{}",json!({
        "tinylm_size":bytes.len(),"tinylm_sha256":tinylm_sha,"magic":magic,
        "left_len":left_len,"flags":flags,"boundary_k":boundary_k,"ranking":ranking,
        "cold_open_ms":cold_open_ms,"warm_open_ms":warm_open_ms,
        "memory_mib":{
            "before_private":before_priv,"before_working_set":before_ws,
            "after_cold_private":cold_priv,"after_cold_working_set":cold_ws,
            "final_private":final_priv,"final_working_set":final_ws,
            "peak_working_set":peak_ws,"peak_pagefile":peak_pagefile
        },
        "short":short_metrics,"long":long_metrics
    }));
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
    let builtin=args[1]=="builtin";
    if args[1]=="prepare"||builtin {
        let semantic=if builtin {
            assert_eq!(id,"builtin");
            true
        }else{
            let item=if root.join(".staging").join(id).join("model.json").is_file(){
                shanjie_shell::models::prepare(root,id)
            }else{shanjie_shell::models::checked(&root.join(id))}.expect("source, dependency and mapping validation");
            assert_eq!(item.spec.id,*id);
            item.spec.catalog.as_ref().is_some_and(|m|m.assets.iter().any(|a|a.role=="semantic"))
        };
        for ranking in 0..(if builtin{2}else{1}) {
            let mut e=engine(data,id,ranking);
            for ch in "su3cl3".chars(){key(&mut e,1,ch);}
            let out=key(&mut e,3,'\0');assert!(!out.commit.is_empty()&&out.preedit.is_empty());
            assert!(key(&mut e,3,'\0').commit.is_empty(),"commit must occur once");
            if semantic{assert!(e.rerank_jobs_run>0,"semantic model must actually run");}
            // The resident core reranks after replying: Op::Rerank must then show what the synchronous rerank shows. Every
            // other job is left unasked, so a newer key replaces it before it is applied.
            let mut sync=engine(data,id,ranking);let mut background=engine(data,id,ranking);background.rerank_in_background();
            let mut applied=0;
            for (index,(kind,ch)) in "su3cl3".chars().map(|c|(1,c)).chain([(3,'\0')]).enumerate(){
                let want=key(&mut sync,kind,ch);let mut got=key(&mut background,kind,ch);
                assert_eq!(got.commit,want.commit,"background rerank keeps the commit");
                if got.rerank{
                    if index%2==1{continue;}
                    let ranked=background.perform(Op::Rerank).expect("rerank");
                    assert!(ranked.commit.is_empty()&&!ranked.rerank,"a rerank commits nothing and leaves nothing running");
                    got=shanjie_shell::protocol::Snapshot{commit:got.commit,..ranked};applied+=1;
                }
                assert_eq!((got.preedit,got.cursor,got.candidates,got.selected),(want.preedit,want.cursor,want.candidates,want.selected),"background rerank");
            }
            if semantic{assert!(applied>0,"background rerank must actually apply");}
            if semantic {
                // Non-decoding key (unfinished zhuyin initial) must keep an unapplied background rerank job alive.
                let mut sync_keep=engine(data,id,ranking);let mut bg_keep=engine(data,id,ranking);bg_keep.rerank_in_background();
                let mut had_pending=false;
                for ch in "su3cl3".chars(){
                    let _=key(&mut sync_keep,1,ch);
                    had_pending=key(&mut bg_keep,1,ch).rerank;
                }
                assert!(had_pending,"completed syllables should queue a background rerank");
                let want_unfinished=key(&mut sync_keep,1,'b');
                let got_unfinished=key(&mut bg_keep,1,'b');
                assert!(got_unfinished.rerank,"unfinished syllable must not discard the pending rerank job");
                let ranked=bg_keep.perform(Op::Rerank).expect("rerank after non-decoding key");
                assert!(!ranked.rerank);
                assert_eq!((ranked.preedit,ranked.cursor,ranked.candidates,ranked.selected),(want_unfinished.preedit,want_unfinished.cursor,want_unfinished.candidates,want_unfinished.selected),"rerank after non-decoding key");
                let cleared=bg_keep.reset(true).expect("reset clears pending rerank");
                assert!(!cleared.rerank&&cleared.preedit.is_empty());
            }
            let mut result=json!({"usable":true,"native_input_and_commit":true,"ranking":ranking,"semantic_jobs_run":e.rerank_jobs_run,"background_reranks_applied":applied});
            if !builtin {result["mapped_equivalent"]=true.into();result["source_and_side_pairs_checked"]=true.into();}
            println!("{}",result);
        }
    } else if args[1]=="bench" {
        assert!(args.len()>=7,"bench expects <models_root> <id> <data> <short_rows> <long_rows> [ranking]");
        let ranking=args.get(7).and_then(|s|s.parse::<u32>().ok()).unwrap_or(0);
        run_bench(data,id,&args[5],&args[6],ranking);
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
