"""Source-verified discovery, native probes, advisory evaluation and atomic catalog publication.

Only release assets from Nanako0129/shanjie are data inputs. No upstream script is executed.
All native parsers run in disposable child processes with a private LOCALAPPDATA.
"""
from __future__ import annotations
import argparse, copy, datetime, hashlib, json, os, pathlib, re, shutil, subprocess, sys, time
import urllib.request, urllib.error

REPO = "tavricccc/shanjie-multiplatform"
UPSTREAM = "Nanako0129/shanjie"
PROFILE = "windows-bundle-v1"
BRANCH = "model-catalog"
ROOT = pathlib.Path(__file__).resolve().parents[2]
NAMES = {"bigram.sjlm":"bigram", "colloquial.sjlm":"bigram", "classes.sjc":"classes",
         "kn.sjkn":"smoothing", "trigram.sjt3":"trigram", "tinylm.sjtl":"semantic"}
CAPS = {"SJLM0001":"lm.sjlm1","SJCL0001":"classes.sjcl1","SJKN0002":"kn.sjkn2",
        "SJT30001":"trigram.sjt30001", **{f"SJTL000{i}":f"semantic.sjtl{i}" for i in range(1,5)}}
def now(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def encoded(x): return json.dumps(x, ensure_ascii=False, sort_keys=True, separators=(",",":")).encode()
def digest(x): return hashlib.sha256(encoded(x)).hexdigest()
def filehash(p):
    with open(p,"rb") as f: return hashlib.file_digest(f,"sha256").hexdigest()
def semantic(x):
    if isinstance(x,dict): return {k:semantic(v) for k,v in x.items() if k not in ("generated_at","checked_at","measured_at","revision")}
    if isinstance(x,list): return [semantic(v) for v in x]
    return x
def write(p,x):
    p=pathlib.Path(p);p.parent.mkdir(parents=True,exist_ok=True)
    temp=p.with_name(p.name+".new");temp.write_bytes(json.dumps(x,ensure_ascii=False,sort_keys=True,indent=2).encode()+b"\n");os.replace(temp,p)
def read(p): return json.loads(pathlib.Path(p).read_text(encoding="utf-8-sig"))
def safe(s,limit=120): return bool(isinstance(s,str) and len(s)<=limit and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*",s) and ".." not in s)
def request(url,method="GET",body=None):
    headers={"User-Agent":"Shanjie-Model-Catalog/1","Accept":"application/vnd.github+json","X-GitHub-Api-Version":"2022-11-28"}
    token=os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token and url.startswith("https://api.github.com/"): headers["Authorization"]="Bearer "+token
    raw=None if body is None else encoded(body)
    if raw:headers["Content-Type"]="application/json"
    for attempt in range(4):
        try: return urllib.request.urlopen(urllib.request.Request(url,data=raw,headers=headers,method=method),timeout=50)
        except urllib.error.HTTPError as e:
            if e.code not in (429,500,502,503,504) or attempt==3: raise
            time.sleep(min(2**attempt,8))
def api(path,method="GET",body=None):
    with request("https://api.github.com/"+path,method,body) as response:return json.load(response)
def releases():
    out=[]
    for page in range(1,101):
        rows=api(f"repos/{UPSTREAM}/releases?per_page=100&page={page}")
        out.extend(r for r in rows if not r["draft"])
        if len(rows)<100:return out
    raise ValueError("Release pagination exceeded bound; refusing a partial snapshot")
def branch_head():
    try:return api(f"repos/{REPO}/git/ref/heads/{BRANCH}")["object"]["sha"]
    except urllib.error.HTTPError as e:
        if e.code==404:return None
        raise
def previous():
    head=branch_head()
    if not head:return None,{"format":2,"models":[]},{}
    def raw(name):
        with request(f"https://raw.githubusercontent.com/{REPO}/{head}/{name}") as r:return json.load(r)
    return head,raw("MODEL-CATALOG-V2.json"),raw("MODEL-CATALOG-STATE.json")
def download(a,cache):
    sha=a.get("digest") or ""
    if not re.fullmatch(r"sha256:[0-9a-f]{64}",sha):raise ValueError("No official SHA-256 digest; source remains unverified")
    size=a["size"]
    if not 0<size<=1073741824:raise ValueError("Asset size exceeds loader limit")
    path=cache/sha[7:];cache.mkdir(parents=True,exist_ok=True)
    if path.exists() and path.stat().st_size==size and filehash(path)==sha[7:]:return path
    # Public cache seeded by local build downloads, without reading any personal folder.
    for p in (ROOT/"build/model").glob("*"):
        if p.is_file() and p.stat().st_size==size and filehash(p)==sha[7:]:
            try:os.link(p,path)
            except OSError:shutil.copyfile(p,path)
            return path
    temp=path.with_suffix(".download")
    with request(a["browser_download_url"]) as response,temp.open("wb") as output:
        count=0
        while block:=response.read(1024*1024):
            count+=len(block)
            if count>size:raise ValueError("Asset exceeds advertised size")
            output.write(block)
    if temp.stat().st_size!=size or filehash(temp)!=sha[7:]:raise ValueError("Official asset digest mismatch")
    os.replace(temp,path);return path
def model(assets,label):
    assets=copy.deepcopy(assets);assets.sort(key=lambda a:(a["role"],a["sha256"]))
    caps=sorted({"map.sjwm2",*(CAPS[a["format_magic"]] for a in assets if a["role"] in NAMES.values())}
                |({"lm.classless"} if not any(a["role"]=="classes" for a in assets) else set()))
    identity={"assets":assets,"required_capabilities":caps}
    return {"id":"bundle-"+digest(identity),"name":label,"assets":assets,"required_capabilities":caps,
            "compatibility":[],"advisories":[],"advisory_revision":digest([])}
def discover(out):
    out.mkdir(parents=True,exist_ok=True);cache=out/"assets";rows=releases()
    head,old,state=previous();found=[];pending=[];sources=[]
    for r in rows:
        if not safe(r["tag_name"],96):
            pending.append({"release_id":r["id"],"state":"unsupported","reason":"Unsafe release tag"});continue
        for a in r["assets"]:
            legal=a["name"]=="colloquial-attribution.tsv" or (a["name"].endswith(".NOTICE.txt") and any(x["name"]=="colloquial.sjlm" for x in r["assets"]))
            if a["name"] not in NAMES and not legal:
                if re.search(r"\.sj[a-z0-9]+$",a["name"]):pending.append({"release":r["tag_name"],"filename":a["name"],"state":"unsupported","reason":"Unknown model attachment role"})
                continue
            expected=f"https://github.com/{UPSTREAM}/releases/download/{r['tag_name']}/{a['name']}"
            if a["browser_download_url"]!=expected:raise ValueError("Unexpected upstream download URL")
            try:
                path=download(a,cache)
                with path.open("rb") as stream:header=stream.read(256)
                magic="TEXT0001" if legal else header[:8].decode("ascii")
                if magic not in CAPS and not legal:raise NotImplementedError("Unknown magic or format version")
                role=("attribution" if a["name"].endswith(".tsv") else "license") if legal else NAMES[a["name"]]
                item={"role":role,"repository":UPSTREAM,"release":r["tag_name"],"filename":a["name"],
                      "format_magic":magic,"size":a["size"],"sha256":a["digest"][7:],"requires_assets":[]}
                pair=None;kn=None
                if item["role"]=="classes":pair=header[8:40].hex()
                elif item["role"] in ("smoothing","trigram"):pair=header[12:44].hex()
                elif item["role"]=="semantic":
                    pair=header[68:100].hex()
                    if magic=="SJTL0004" and header[212:244]!=bytes(32):kn=header[212:244].hex()
                found.append({"asset":item,"pair":pair,"kn":kn,"prerelease":r["prerelease"]})
                sources.append({"release_id":r["id"],"asset_id":a["id"],"release":r["tag_name"],"filename":a["name"],
                                "size":a["size"],"digest":a["digest"]})
            except Exception as e:
                pending.append({"release":r["tag_name"],"filename":a["name"],"state":"unsupported" if isinstance(e,NotImplementedError) else "unverified","reason":str(e)[:600]})
    candidates=[];baseline=None
    for base in (x for x in found if x["asset"]["role"]=="bigram"):
        b=base["asset"];classes=[x for x in found if x["asset"]["role"]=="classes" and x["pair"]==b["sha256"]]
        # Offer every released paired class table. A genuinely classless model has its own capability.
        options=classes or [None]
        for c in options:
            legal=[x["asset"] for x in found if x["asset"]["role"] in ("license","attribution") and x["asset"]["release"]==b["release"]]
            assets=[b]+([dict(c["asset"],requires_assets=[b["sha256"]])] if c else [])+legal
            m=model(assets,b["release"]+(" ＋ "+c["asset"]["release"] if c else "（獨立無詞類模型）"))
            candidates.append(m)
            if b["release"]=="model-v6" and c and c["asset"]["release"]=="classes-v5":baseline=m["id"]
        default=options[0]
        standard=[b]+([dict(default["asset"],requires_assets=[b["sha256"]])] if default else [])+legal
        for extra in (x for x in found if x["pair"]==b["sha256"] and x["asset"]["role"] in ("smoothing","trigram","semantic")):
            assets=copy.deepcopy(standard);deps=[b["sha256"]]
            if extra["kn"]:
                side=next((x for x in found if x["asset"]["sha256"]==extra["kn"] and x["pair"]==b["sha256"]),None)
                if side is None:
                    pending.append({"release":extra["asset"]["release"],"state":"unsupported","reason":"Required paired KN attachment missing"});continue
                assets.append(dict(side["asset"],requires_assets=[b["sha256"]]));deps.append(extra["kn"])
            assets.append(dict(extra["asset"],requires_assets=sorted(deps)))
            candidates.append(model(assets," ＋ ".join(a["release"] for a in assets)))
    candidates=list({m["id"]:m for m in candidates}.values())
    if not baseline:raise ValueError("Public baseline pair not found; refusing incomplete discovery")
    dataset=ROOT/"scripts/model_catalog/evaluation.txt"
    plan={"source_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
          "profile":PROFILE,"head":head,"previous":old,"previous_state":state,"sources":sources,"pending":pending,
          "candidates":candidates,"baseline":baseline,"dataset_sha256":filehash(dataset),"created_at":now(),
          "prereleases":[x["asset"]["sha256"] for x in found if x["prerelease"]]}
    write(out/"plan.json",plan);print(f"Discovered {len(candidates)} bundles; {len(pending)} pending")
def spec(m):
    b=next(a for a in m["assets"] if a["role"]=="bigram");c=next((a for a in m["assets"] if a["role"]=="classes"),{})
    return {"id":m["id"],"name":m["name"],"model_release":b["release"],"classes_release":c.get("release",""),
            "model_sha256":b["sha256"],"classes_sha256":c.get("sha256",""),"model_size":b["size"],"classes_size":c.get("size",0),"catalog":m}
def run_child(exe,args,env,timeout):
    p=subprocess.run([str(exe),*map(str,args)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=timeout)
    if p.returncode:raise RuntimeError(f"Native checker exited {p.returncode}: "+p.stderr.decode("utf-8","replace")[:800])
    return json.loads(p.stdout.decode("utf-8-sig"))
def probe(out,exe,data,target):
    plan=read(out/"plan.json");work=out/target;work.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env.update(LOCALAPPDATA=str(work/"userdata"),SHANJIE_CORE_LOCAL="1",SHANJIE_TEST_PROFILE="1",SHANJIE_NO_WORD_HELPER="1")
    for name in ("GH_TOKEN","GITHUB_TOKEN"):env.pop(name,None)
    models=work/"userdata/Shanjie/Models";reports=[];cache=out/"probe-cache";cache.mkdir(exist_ok=True)
    for original in plan["candidates"]:
        m=copy.deepcopy(original)
        m["compatibility"]=[{"target":target,"profile_id":PROFILE,"source_commit":plan["source_commit"],"state":"usable","checked_at":now(),"report":"reports/provisional.json"}]
        m["advisory_revision"]=digest([])
        stage=models/".staging"/m["id"];stage.mkdir(parents=True,exist_ok=True)
        for a in m["assets"]:
            path=stage/a["filename"]
            if not path.exists():
                try:os.link(out/"assets"/a["sha256"],path)
                except OSError:shutil.copyfile(out/"assets"/a["sha256"],path)
        write(stage/"model.json",spec(m))
        key=digest({"id":m["id"],"commit":plan["source_commit"],"target":target,"dataset":plan["dataset_sha256"]})
        stored=cache/(key+".json")
        try:
            # Still prepare this run's immutable local bundle; cached technical evidence
            # is reused only for the same code, target, data and exact source assets.
            technical=run_child(exe,["prepare",models,m["id"],data],env,180)
            if not technical.get("usable"):raise ValueError("Native loader did not confirm usability")
            result={"id":m["id"],"target":target,"profile_id":PROFILE,"source_commit":plan["source_commit"],
                    "state":"usable","asset_hashes":[a["sha256"] for a in m["assets"]],"checked_at":now(),"technical":technical}
            if stored.exists():
                prior=read(stored)
                if prior["state"]=="usable":result=prior
        except Exception as e:
            result={"id":m["id"],"target":target,"profile_id":PROFILE,"source_commit":plan["source_commit"],
                    "state":"unverified" if isinstance(e,subprocess.TimeoutExpired) else "invalid","asset_hashes":[a["sha256"] for a in m["assets"]],
                    "checked_at":now(),"reason":str(e)[:1000]}
        reports.append(result);print(target,m["name"],result["state"],flush=True)
    for r in reports:
        if r["state"]!="usable" or "evaluation" in r:continue
        try:
            r["evaluation"]=run_child(exe,["evaluate",models,r["id"],data,ROOT/"scripts/model_catalog/evaluation.txt"],env,180)
        except Exception as e:r["evaluation"]={"state":"not_evaluated","reason":str(e)[:600]}
        key=digest({"id":r["id"],"commit":plan["source_commit"],"target":target,"dataset":plan["dataset_sha256"]})
        write(cache/(key+".json"),r)
    write(work/"results.json",reports)
def validate(c):
    if c.get("format")!=2 or c.get("example_only") is not False:raise ValueError("Not a production v2 catalog")
    if c.get("revision")!=digest(semantic(c)):raise ValueError("Catalog revision mismatch")
    ids=set()
    for m in c["models"]:
        if m["id"] in ids:raise ValueError("Duplicate bundle identity")
        ids.add(m["id"])
        expected=model(m["assets"],m["name"])
        if m["id"]!=expected["id"] or m["required_capabilities"]!=expected["required_capabilities"]:raise ValueError("Bundle identity/capability mismatch")
        if m["advisory_revision"]!=digest(semantic(m["advisories"])):raise ValueError("Advisory revision mismatch")
        if not any(x["state"]=="usable" for x in m["compatibility"]):raise ValueError("No tested usable target")
        roles=set();hashes={a["sha256"] for a in m["assets"]};graph={}
        for a in m["assets"]:
            if a["role"] in roles or a["repository"]!=UPSTREAM or not safe(a["release"],96) or not safe(a["filename"]):raise ValueError("Unsafe or duplicate asset")
            roles.add(a["role"]);graph[a["sha256"]]=set(a["requires_assets"])
            if not graph[a["sha256"]]<=hashes or a["sha256"] in graph[a["sha256"]]:raise ValueError("Missing/self dependency")
        while graph:
            leaves={h for h,deps in graph.items() if not deps}
            if not leaves:raise ValueError("Cyclic dependency")
            graph={h:deps-leaves for h,deps in graph.items() if h not in leaves}
        for a in m["advisories"]:
            for n in a["measurements"]:
                if n["sample_count"]<1 or min(n["baseline"],n["candidate"])<0:raise ValueError("Invalid measurements")
                if n["unit"]=="sentences" and max(n["baseline"],n["candidate"])>n["sample_count"]:raise ValueError("Wrong denominator")
    return c
def assemble(out):
    plan=read(out/"plan.json");all_results=[]
    for p in out.glob("windows-*/results.json"):all_results+=read(p)
    if not all_results:raise ValueError("No native probe results")
    by_id={m["id"]:copy.deepcopy(m) for m in plan["previous"].get("models",[])}
    published=out/"publication";published.mkdir(exist_ok=True);pending=list(plan["pending"])
    baseline={r["target"]:r.get("evaluation",{}) for r in all_results if r["id"]==plan["baseline"] and r["state"]=="usable"}
    for m in copy.deepcopy(plan["candidates"]):
        matches=[r for r in all_results if r["id"]==m["id"]]
        for r in matches:
            if r["source_commit"]!=plan["source_commit"] or r["asset_hashes"]!=[a["sha256"] for a in m["assets"]]:raise ValueError("Probe evidence identity mismatch")
        usable=[r for r in matches if r["state"]=="usable"]
        if not usable:
            pending.extend(matches);continue
        m["compatibility"]=[];m["advisories"]=[]
        for r in matches:
            path="reports/"+digest(r)+".json";write(published/path,r)
            m["compatibility"].append({k:r[k] for k in ("target","profile_id","source_commit","state","checked_at")}|{"report":path}|({"reason":r["reason"]} if "reason" in r else {}))
        measurements=[]
        for r in usable:
            e=r.get("evaluation",{});b=baseline.get(r["target"],{})
            if e.get("state")!="evaluated" or b.get("state")!="evaluated":continue
            for ranking in ("chat","formal"):
                if e["sample_count"]!=b["sample_count"]:raise ValueError("Unequal comparison denominators")
                config={"profile":PROFILE,"target":r["target"],"ranking":ranking,"source_commit":plan["source_commit"],"learning":False,"context":True}
                measurements.append({"metric":"top1-"+ranking+"-"+r["target"],"unit":"sentences","dataset_id":"windows-public20-v1",
                    "dataset_sha256":plan["dataset_sha256"],"configuration_sha256":digest(config),"sample_count":e["sample_count"],
                    "baseline_model_id":plan["baseline"],"baseline":b[ranking],"candidate":e[ranking],"measured_at":r["checked_at"],
                    "context":"Same Windows native core, public20, supplied left context, no learning; "+r["target"]})
        def note(code,title,body,values=None):m["advisories"].append({"code":code,"title":title,"body":body,"measurements":values or []})
        if measurements:
            body="；".join(f"{n['metric']}：{n['candidate']}/{n['sample_count']}，基準 {n['baseline']}/{n['sample_count']}" for n in measurements)
            lower=any(n["candidate"]<n["baseline"] for n in measurements)
            note("quality-regression" if lower else "evaluation","部分評測低於基準" if lower else "公開評測資料",body+"。此比較只代表固定測試集，您仍可選用。",measurements)
        else:note("not-evaluated","尚未完成可比評測","已通過完整性、配對與載入測試；目前沒有可比的準確率資料，仍可選用。")
        if any(r.get("evaluation",{}).get("state")!="evaluated" for r in usable):
            note("partial-evaluation","部分平台尚未評測","未取得成績的平台不沿用其他平台分數；技術上可用者仍可選用。")
        if any(a["sha256"] in plan["prereleases"] for a in m["assets"]):note("preview","預覽模型","包含上游預覽資料，可能有使用差異；仍可選用。")
        if any(a["role"]=="semantic" for a in m["assets"]):note("semantic-cost","包含語意重排","語意模型會增加記憶體與每個音節的計算量。本版在常駐核心使用標準重排強度；效能未作跨機器比較。")
        if any(a["role"]=="trigram" for a in m["assets"]):note("trigram-profile","正式模式使用三元模型","依上游規則，三元模型用於正式模式；聊天模式仍使用二元模型。")
        if any(a["filename"]=="colloquial.sjlm" for a in m["assets"]):note("standalone-colloquial","獨立口語模型","這個配套單獨使用口語語料模型，取代一般主模型；不是把它暗中混合進原模型。詞彙覆蓋與排序可能不同。")
        m["advisory_revision"]=digest(semantic(m["advisories"]));by_id[m["id"]]=m
    catalog={"format":2,"example_only":False,"generated_at":now(),"models":sorted(by_id.values(),key=lambda m:m["id"])}
    catalog["revision"]=digest(semantic(catalog));validate(catalog)
    state={"format":1,"source_snapshot":digest(plan["sources"]),"sources":plan["sources"],"pending":pending,"loader_profile":PROFILE}
    write(published/"MODEL-CATALOG-V2.json",catalog);write(published/"MODEL-CATALOG-STATE.json",state)
    files={str(p.relative_to(published)).replace("\\","/"):filehash(p) for p in published.rglob("*.json")}
    write(out/"publication-plan.json",{"head":plan["head"],"source_commit":plan["source_commit"],"files":files,"sources":plan["sources"]})
    print(f"Catalog has {len(catalog['models'])} usable bundles; {len(pending)} pending; revision {catalog['revision']}")
def publish(out):
    plan=read(out/"publication-plan.json");folder=out/"publication"
    for name,sha in plan["files"].items():
        if name not in ("MODEL-CATALOG-V2.json","MODEL-CATALOG-STATE.json") and not re.fullmatch(r"reports/[a-f0-9]{64}\.json",name):raise ValueError("Publication path not allowed")
        if filehash(folder/name)!=sha:raise ValueError("Publication plan content changed")
    for a in plan["sources"]:
        current=api(f"repos/{UPSTREAM}/releases/assets/{a['asset_id']}")
        if any(current[k]!=a[v] for k,v in (("size","size"),("name","filename"),("digest","digest"))):raise ValueError("Upstream asset changed; rediscover before publishing")
    for attempt in range(3):
        head,old,old_state=previous();catalog=read(folder/"MODEL-CATALOG-V2.json");validate(catalog)
        if head!=plan["head"]:
            merged={m["id"]:m for m in old["models"]};merged.update({m["id"]:m for m in catalog["models"]})
            catalog["models"]=sorted(merged.values(),key=lambda m:m["id"]);catalog["revision"]=digest(semantic(catalog));validate(catalog)
        if old.get("revision")==catalog["revision"] and digest(old_state)==digest(read(folder/"MODEL-CATALOG-STATE.json")):
            print("No semantic catalog change; no empty commit");return
        tree_entries=[]
        for name in sorted(plan["files"]):
            data=catalog if name=="MODEL-CATALOG-V2.json" else read(folder/name)
            tree_entries.append({"path":name,"mode":"100644","type":"blob","content":json.dumps(data,ensure_ascii=False,sort_keys=True,indent=2)+"\n"})
        body={"tree":tree_entries}
        if head:body["base_tree"]=api(f"repos/{REPO}/git/commits/{head}")["tree"]["sha"]
        tree=api(f"repos/{REPO}/git/trees","POST",body)["sha"]
        commit=api(f"repos/{REPO}/git/commits","POST",{"message":"models: refresh verified catalog "+catalog["revision"][:12],"tree":tree,"parents":[head] if head else []})["sha"]
        try:
            if head:api(f"repos/{REPO}/git/refs/heads/{BRANCH}","PATCH",{"sha":commit,"force":False})
            else:api(f"repos/{REPO}/git/refs","POST",{"ref":"refs/heads/"+BRANCH,"sha":commit})
            print("Published",commit);return
        except urllib.error.HTTPError as e:
            if e.code not in (409,422) or attempt==2:raise
    raise RuntimeError("Publication race; retry maintenance")
def main():
    p=argparse.ArgumentParser();p.add_argument("command",choices=["discover","probe","assemble","publish","validate"])
    p.add_argument("--out",type=pathlib.Path,default=ROOT/"build/model-catalog");p.add_argument("--checker",type=pathlib.Path)
    p.add_argument("--data",type=pathlib.Path,default=ROOT/"dist/data");p.add_argument("--target",choices=["windows-x64","windows-x86"])
    a=p.parse_args();a.out=a.out.resolve()
    if a.command=="discover":discover(a.out)
    elif a.command=="probe":probe(a.out,a.checker.resolve(),a.data.resolve(),a.target)
    elif a.command=="assemble":assemble(a.out)
    elif a.command=="publish":publish(a.out)
    else:validate(read(a.out/"publication/MODEL-CATALOG-V2.json"));print("Catalog contract passed")
if __name__=="__main__":main()
