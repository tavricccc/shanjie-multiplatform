"""Prepare offline model-manager fixtures from the verified public build cache."""
import copy
import importlib.util
import json
import os
import pathlib
import sys

root = pathlib.Path(__file__).resolve().parents[1]
module = importlib.util.spec_from_file_location("catalog", root / "scripts/model_catalog/catalog.py")
catalog = importlib.util.module_from_spec(module)
module.loader.exec_module(catalog)
folder = pathlib.Path(sys.argv[1])
folder.mkdir(parents=True, exist_ok=True)
assets_folder = folder / "assets"
assets_folder.mkdir(exist_ok=True)
manifest = catalog.read(root / "MODEL-SOURCES.json")
all_assets = []
for item in manifest["builtin"]["assets"]:
    source = root / "build/model" / f"{item['release']}-{item['filename']}"
    if source.stat().st_size != item["size"] or catalog.filehash(source) != item["sha256"]:
        raise ValueError("Model fixture source does not match the public manifest")
    with source.open("rb") as stream:
        magic = "TEXT0001" if item["role"] == "attribution" else stream.read(8).decode("ascii")
    asset = {key: item[key] for key in ("role", "release", "filename", "size", "sha256")}
    asset.update(repository=catalog.MIRROR, format_magic=magic, requires_assets=[])
    all_assets.append(asset)
    target = assets_folder / item["sha256"]
    if not target.exists():
        os.link(source, target)

main = next(asset["sha256"] for asset in all_assets if asset["role"] == "bigram")
smoothing = next(asset["sha256"] for asset in all_assets if asset["role"] == "smoothing")
for asset in all_assets:
    if asset["role"] not in ("bigram", "attribution"):
        asset["requires_assets"] = [main]
    if asset["role"] == "semantic":
        asset["requires_assets"].append(smoothing)

assets = [asset for asset in all_assets if asset["role"] in ("bigram", "classes", "smoothing", "semantic")]
models = []
for updated in (False, True):
    entries = copy.deepcopy(assets)
    if updated:
        for asset in entries:
            asset["repository"] = catalog.UPSTREAM
            if asset["role"] == "semantic":
                asset["release"] += "-revision"
    model = catalog.model(entries, "模型來源與更新驗證")
    model["compatibility"] = [{"target": "windows-x64", "profile_id": catalog.PROFILE,
        "source_commit": "1" * 40, "state": "usable", "checked_at": catalog.now(),
        "report": "reports/" + "2" * 64 + ".json"}]
    model["advisories"] = [{"code": "quality-regression", "title": "退步提示測試",
        "body": "隔離驗證用的評測備註。", "measurements": []},
        {"code": "performance-unmeasured", "title": "效能尚未比較",
         "body": "未提供同場速度與記憶體測量；不以缺少效能資料限制選用。", "measurements": []},
        {"code": "semantic-cost", "title": "包含語意校正",
         "body": "語意模型會增加記憶體與每個音節的計算量，強度依設定；效能未作跨機器比較。",
         "measurements": []}]
    model["advisory_revision"] = catalog.digest(catalog.semantic(model["advisories"]))
    models.append(model)

complete_assets = copy.deepcopy(all_assets)
for asset in complete_assets:
    asset["repository"] = catalog.UPSTREAM
complete = catalog.model(complete_assets, "與內建同版的完整配套")
complete["compatibility"] = copy.deepcopy(models[0]["compatibility"])
models.append(complete)
future_assets = copy.deepcopy(all_assets)
future_semantic = next(asset for asset in future_assets if asset["role"] == "semantic")
parts = future_semantic["release"].split("-")
parts[2] = str(int(parts[2]) + 1)
future_semantic["release"] = "-".join(parts)
future = catalog.model(future_assets, "較新的完整配套")
future["compatibility"] = copy.deepcopy(models[0]["compatibility"])
future_list = {"format": 2, "example_only": False, "generated_at": catalog.now(), "models": [future]}
future_list["revision"] = catalog.digest(catalog.semantic(future_list))
catalog.validate(future_list)
catalog.write(folder / "future-v2.json", future_list)
stale_assets = copy.deepcopy(assets)
stale_semantic = next(asset for asset in stale_assets if asset["role"] == "semantic")
stale_semantic.update(release="sj-sr-7-15m", sha256="f7d0ac6377bfe973cf4239a949d7a763f1acb0095e50bc6c1323bd3ed09d3478")
stale_model = catalog.model(stale_assets, "較舊的配套索引")
stale_model["compatibility"] = copy.deepcopy(models[0]["compatibility"])
stale = {"format": 2, "example_only": False, "generated_at": catalog.now(), "models": [stale_model]}
stale["revision"] = catalog.digest(catalog.semantic(stale))
catalog.validate(stale)
catalog.write(folder / "stale-v2.json", stale)

listing = {"format": 2, "example_only": False, "generated_at": catalog.now(), "models": models}
listing["revision"] = catalog.digest(catalog.semantic(listing))
catalog.validate(listing)
catalog.write(folder / "fixture-v2.json", listing)
releases = {}
for model in models:
    for asset in model["assets"]:
        release = releases.setdefault(asset["release"], {"tag_name": asset["release"],
            "draft": False, "prerelease": False, "assets": []})
        if any(row["name"] == asset["filename"] for row in release["assets"]):
            continue
        release["assets"].append({"name": asset["filename"], "size": asset["size"],
            "digest": "sha256:" + asset["sha256"],
            "browser_download_url": f"https://github.com/{catalog.UPSTREAM}/releases/download/{asset['release']}/{asset['filename']}"})
catalog.write(folder / "upstream-releases.json", list(releases.values()))
