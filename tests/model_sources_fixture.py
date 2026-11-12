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
assets = []
for item in manifest["builtin"]["assets"]:
    if item["role"] not in ("bigram", "classes", "smoothing", "semantic"):
        continue
    source = root / "build/model" / f"{item['release']}-{item['filename']}"
    if source.stat().st_size != item["size"] or catalog.filehash(source) != item["sha256"]:
        raise ValueError("Model fixture source does not match the public manifest")
    with source.open("rb") as stream:
        magic = stream.read(8).decode("ascii")
    asset = {key: item[key] for key in ("role", "release", "filename", "size", "sha256")}
    asset.update(repository=catalog.MIRROR, format_magic=magic, requires_assets=[])
    assets.append(asset)
    target = assets_folder / item["sha256"]
    if not target.exists():
        os.link(source, target)

main = next(asset["sha256"] for asset in assets if asset["role"] == "bigram")
smoothing = next(asset["sha256"] for asset in assets if asset["role"] == "smoothing")
for asset in assets:
    if asset["role"] != "bigram":
        asset["requires_assets"] = [main]
    if asset["role"] == "semantic":
        asset["requires_assets"].append(smoothing)

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
        "body": "隔離驗證用的評測備註。", "measurements": []}]
    model["advisory_revision"] = catalog.digest(catalog.semantic(model["advisories"]))
    models.append(model)

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
