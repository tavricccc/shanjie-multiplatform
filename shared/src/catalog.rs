//! Versioned model capabilities and immutable bundle identities, shared with the native loader.
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::collections::{BTreeSet,BTreeMap};
pub type Result<T> = std::result::Result<T,Box<dyn std::error::Error>>;
pub const PROFILE:&str="windows-bundle-v1";
pub const CAPABILITIES:&[&str]=&["lm.sjlm1","lm.classless","classes.sjcl1","map.sjwm2",
    "kn.sjkn2","trigram.sjt30001","semantic.sjtl1","semantic.sjtl2","semantic.sjtl3","semantic.sjtl4"];
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub role:String,pub repository:String,pub release:String,pub filename:String,
    pub format_magic:String,pub size:u64,pub sha256:String,pub requires_assets:Vec<String>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    pub target:String,pub profile_id:String,pub source_commit:String,pub state:String,
    pub checked_at:String,pub report:String,#[serde(default,skip_serializing_if="Option::is_none")]pub reason:Option<String>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advisory {pub code:String,pub title:String,pub body:String,pub measurements:Vec<Value>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id:String,pub name:String,pub required_capabilities:Vec<String>,pub assets:Vec<Asset>,
    pub compatibility:Vec<Compatibility>,pub advisory_revision:String,pub advisories:Vec<Advisory>,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    #[serde(rename="$schema",default,skip_serializing_if="Option::is_none")]pub schema:Option<String>,
    pub format:u32,pub revision:String,pub generated_at:String,pub example_only:bool,pub models:Vec<Model>,
}
pub fn digest(value:&Value)->String {format!("{:x}",Sha256::digest(serde_json::to_vec(value).unwrap()))}
pub fn semantic(mut value:Value)->Value {
    fn strip(v:&mut Value){match v {
        Value::Object(m)=>{for k in ["checked_at","measured_at","generated_at","revision"] {m.remove(k);}
            for (_,v) in m{strip(v);}},
        Value::Array(a)=>for v in a{strip(v)},_=>{}
    }}
    strip(&mut value);value
}
pub fn identity(m:&Model)->String {
    let mut assets=m.assets.clone();assets.sort_by(|a,b|a.role.cmp(&b.role).then(a.sha256.cmp(&b.sha256)));
    let mut caps=m.required_capabilities.clone();caps.sort();
    format!("bundle-{}",digest(&json!({"assets":assets,"required_capabilities":caps})))
}
pub fn advisory_revision(m:&Model)->String {digest(&semantic(serde_json::to_value(&m.advisories).unwrap()))}
pub fn safe_name(s:&str,max:usize)->bool {
    !s.is_empty()&&s.len()<=max&&s.as_bytes()[0].is_ascii_alphanumeric()
        &&s.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-".contains(&c))&&!s.contains("..")
}
fn fail<T>(s:&str)->Result<T>{Err(s.into())}
pub fn asset_capability(a:&Asset)->Result<Option<&'static str>> {
    let cap=match (a.role.as_str(),a.format_magic.as_str(),a.filename.as_str()) {
        ("bigram","SJLM0001","bigram.sjlm"|"colloquial.sjlm")=>Some("lm.sjlm1"),
        ("classes","SJCL0001","classes.sjc")=>Some("classes.sjcl1"),
        ("smoothing","SJKN0002","kn.sjkn")=>Some("kn.sjkn2"),
        ("trigram","SJT30001","trigram.sjt3")=>Some("trigram.sjt30001"),
        ("semantic","SJTL0001","tinylm.sjtl")=>Some("semantic.sjtl1"),
        ("semantic","SJTL0002","tinylm.sjtl")=>Some("semantic.sjtl2"),
        ("semantic","SJTL0003","tinylm.sjtl")=>Some("semantic.sjtl3"),
        ("semantic","SJTL0004","tinylm.sjtl")=>Some("semantic.sjtl4"),
        ("attribution"|"license","TEXT0001",_)=>None,
        _=>return fail("模型含未知格式／附件角色，需要更新核心。"),
    };Ok(cap)
}
pub fn validate(m:&Model)->Result<()> {
    if !crate::models::valid_id(&m.id)||m.name.is_empty()||m.name.chars().count()>240
        ||m.assets.is_empty()||m.assets.len()>16||m.advisories.len()>32||m.compatibility.len()>128 {return fail("模型資訊格式有誤。");}
    let mut names=BTreeSet::new();let mut roles=BTreeSet::new();let mut caps=BTreeSet::from(["map.sjwm2"]);
    let hashes:BTreeSet<_>=m.assets.iter().map(|a|a.sha256.as_str()).collect();
    for a in &m.assets {
        if a.repository!="Nanako0129/shanjie"||!safe_name(&a.release,96)||!safe_name(&a.filename,120)
            ||!crate::models::digest_ok(&a.sha256)||a.sha256!=a.sha256.to_ascii_lowercase()
            ||a.size==0||a.size>1073741824||!names.insert(a.filename.as_str())
            ||!roles.insert(a.role.as_str())||a.requires_assets.len()>16 {return fail("模型附件來源或身份無效。");}
        if let Some(c)=asset_capability(a)? {caps.insert(c);}
        for h in &a.requires_assets {if !hashes.contains(h.as_str())||h==&a.sha256{return fail("模型附件依賴有誤。");}}
    }
    if !roles.contains("bigram"){return fail("缺少主模型。");}
    if !roles.contains("classes"){caps.insert("lm.classless");}
    let mut graph:BTreeMap<&str,Vec<&str>>=m.assets.iter().map(|a|(a.sha256.as_str(),a.requires_assets.iter().map(String::as_str).collect())).collect();
    while !graph.is_empty(){
        let leaves:BTreeSet<_>=graph.iter().filter(|(_,d)|d.is_empty()).map(|(h,_)|*h).collect();
        if leaves.is_empty(){return fail("模型附件依賴有環。");}
        graph.retain(|h,_|!leaves.contains(h));for d in graph.values_mut(){d.retain(|h|!leaves.contains(h));}
    }
    let requested:BTreeSet<_>=m.required_capabilities.iter().map(String::as_str).collect();
    if requested!=caps||requested.len()!=m.required_capabilities.len(){return fail("模型所需能力與附件不符。");}
    for a in &m.advisories {
        if !safe_name(&a.code,96)||a.title.is_empty()||a.title.chars().count()>120||a.body.is_empty()
            ||a.body.chars().count()>4000||a.measurements.len()>64{return fail("模型備註格式有誤。");}
        for n in &a.measurements {
            let count=n["sample_count"].as_u64().ok_or("評測分母有誤。")?;
            let b=n["baseline"].as_f64().ok_or("評測資料有誤。")?;
            let c=n["candidate"].as_f64().ok_or("評測資料有誤。")?;
            if count==0||!b.is_finite()||!c.is_finite()||b<0.0||c<0.0
                ||(n["unit"]=="sentences"&&(b>count as f64||c>count as f64)){return fail("評測數字超出範圍。");}
        }
    }
    if m.id!=identity(m)||m.advisory_revision!=advisory_revision(m){return fail("模型或備註的內容識別不符。");}
    Ok(())
}
pub fn usable(m:&Model)->bool {
    let target=if cfg!(target_pointer_width="32"){"windows-x86"}else{"windows-x64"};
    validate(m).is_ok()&&m.compatibility.iter().any(|c|c.target==target&&c.profile_id==PROFILE
        &&c.state=="usable"&&c.source_commit.len()==40&&c.source_commit.bytes().all(|b|b.is_ascii_hexdigit())
        &&c.report.starts_with("reports/")&&!c.report.contains(".."))
}
pub fn parse(bytes:&[u8])->Result<Catalog>{
    if bytes.len()>8*1024*1024{return fail("線上模型清單過大。");}
    let c:Catalog=serde_json::from_slice(bytes)?;
    if c.format!=2||c.example_only||c.models.len()>1000||c.revision!=digest(&semantic(serde_json::to_value(&c)?)){return fail("模型清單格式或修訂無效。");}
    let mut ids=BTreeSet::new();
    for m in &c.models {
        if m.required_capabilities.iter().all(|x|CAPABILITIES.contains(&x.as_str())){validate(m)?;}
        else if m.id!=identity(m)||m.advisory_revision!=advisory_revision(m)
            ||m.assets.iter().any(|a|a.repository!="Nanako0129/shanjie"||!safe_name(&a.filename,120)||!safe_name(&a.release,96)){
            return fail("未知能力的模型資訊有誤。");
        }
        if !ids.insert(&m.id){return fail("模型識別重複。");}
    }
    Ok(c)
}
