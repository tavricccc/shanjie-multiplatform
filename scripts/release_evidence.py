"""CI-only, immutable full-Release evidence; local RELEASE schema 4 stays executed-only."""
from __future__ import annotations
import argparse, base64, copy, hashlib, io, json, os, pathlib, platform, re, shutil, subprocess, sys, zipfile
import xml.etree.ElementTree as ET

VERSION_FILES = ('RELEASE-CHANNEL.json', 'ui/Shanjie.UI.csproj', 'windows/Cargo.lock', 'windows/Cargo.toml')
REQUIRED_JOBS = ('Plan', 'Windows Release / x64', 'Windows Release / x86', 'Windows Release / Core',
                 'Windows Release / CTest', 'Windows Release / CTest x64', 'Windows Release / CTest x86',
                 'Windows Release / Core cap', 'Windows Release / Core mixed', 'Windows Release / Core share',
                 'Windows Release / Core latency', 'Windows Release / CTest Merge', 'Windows Release / Package')
# Every binary is a candidate. Eligibility depends on its complete inputs and environment.
MAX_ARTIFACT_BYTES = 128 * 1024 * 1024

def sha256(data):
    return hashlib.sha256(data).hexdigest()

def approved_notes(path):
    return path == 'Changelog.md' or bool(re.fullmatch(r'docs/releases/RELEASE-NOTES-\d+\.\d+\.\d+(?:-beta\.\d+)?\.md', path))

def normalize_version(path, data):
    text = data.decode('utf-8')
    if path == 'windows/Cargo.toml':
        pattern = r'(?ms)(^\[package\]\s*\n(?:(?!^\[).)*?^version\s*=\s*")[^"]+("[^\n]*)'
    elif path == 'windows/Cargo.lock':
        pattern = r'(?m)(^name = "shanjie-windows"\s*\nversion = ")[^"]+("[^\n]*)'
    elif path == 'ui/Shanjie.UI.csproj':
        pattern = r'(<Version>)[^<]+(</Version>)'
    else:
        channel = json.loads(text)
        if set(channel) != {'version', 'prerelease', 'recommended_stable', 'build_linux'}:
            raise ValueError('Unknown or missing Release Channel policy')
        pattern = r'("version"\s*:\s*")[^"]+(")'
    normalized, count = re.subn(pattern, lambda m: m[1] + '<release-version>' + m[2], text)
    if count != 1:
        raise ValueError(f'Cannot normalize exactly one approved version in {path}')
    return normalized.encode('utf-8')

def fingerprint(entries, read_blob):
    rows = []
    present = set()
    for entry in sorted(entries, key=lambda e: e['path']):
        path = entry['path']
        if entry['type'] == 'tree' or approved_notes(path):
            continue
        digest = entry['sha']
        if path in VERSION_FILES:
            present.add(path)
            digest = sha256(normalize_version(path, read_blob(path)))
        rows.append([path, entry['mode'], entry['type'], digest])
    if present != set(VERSION_FILES):
        raise ValueError('Release input tree lacks the four version files')
    return sha256(json.dumps(['release-inputs-v1', rows], ensure_ascii=False, separators=(',', ':')).encode())

def local_fingerprint(root, ref='HEAD'):
    tree = subprocess.check_output(['git', '-C', str(root), 'ls-tree', '-rz', ref])
    entries = []
    for record in tree.split(b'\0'):
        if record:
            meta, path = record.split(b'\t', 1)
            mode, kind, digest = meta.decode().split()
            entries.append(dict(path=path.decode('utf-8'), mode=mode, type=kind, sha=digest))
    return fingerprint(entries, lambda p: subprocess.check_output(['git', '-C', str(root), 'show', f'{ref}:{p}']))

def remote_fingerprint(api, commit):
    tree = api.request('GET', f'/git/trees/{commit}?recursive=1')
    if not isinstance(tree, dict) or tree.get('truncated') is not False or not isinstance(tree.get('tree'), list):
        raise ValueError('Complete product/test input tree unavailable')
    def blob(path):
        value = api.request('GET', f'/contents/{path}?ref={commit}')
        if not isinstance(value, dict) or value.get('encoding') != 'base64':
            raise ValueError(f'Cannot read pinned version file: {path}')
        return base64.b64decode(value['content'])
    return fingerprint(tree['tree'], blob)

def environment():
    identity = {key: os.environ.get(key, '') for key in (
        'ImageOS', 'ImageVersion', 'PROCESSOR_IDENTIFIER', 'NUMBER_OF_PROCESSORS',
        'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUST_TEST_THREADS', 'TZ', 'LANG')}
    test_env=('SHANJIE_TEST_PROFILE','SHANJIE_HEADLESS','SHANJIE_CORE_LOCAL','SHANJIE_TEST_FAULT',
              'SHANJIE_TEST_FAULT_OP','SHANJIE_TEST_FAULT_SEQUENCE','SHANJIE_TEST_PINYIN','SHANJIE_TEST_ETEN')
    identity.update(os=platform.version(), machine=platform.machine(), workspace=str(pathlib.Path.cwd().resolve()),
                    logical_processors=os.cpu_count(), test_environment={k:os.environ.get(k) for k in test_env})
    identity['python_sha256']=sha256(pathlib.Path(sys.executable).read_bytes())
    identity['pwsh_sha256']=sha256(pathlib.Path(shutil.which('pwsh')).read_bytes())
    identity['git_sha256']=sha256(pathlib.Path(shutil.which('git')).read_bytes())
    identity['native_toolchain']=os.environ.get('SHANJIE_RELEASE_TOOLCHAIN_IDENTITY') or os.environ.get('SHANJIE_CORE_TOOLCHAIN_IDENTITY','')
    for name, args in {'rustc': ['rustc', '-Vv'], 'cargo': ['cargo', '--version'],
                       'cmake': ['cmake', '--version'],
                       'pwsh': ['pwsh', '-NoProfile', '-Command', '$PSVersionTable.PSVersion.ToString()']}.items():
        identity[name] = subprocess.check_output(args, text=True).strip()
    if os.name=='nt':
        probe='$c=Get-CimInstance Win32_Processor|Select-Object -First 1;$o=Get-CimInstance Win32_OperatingSystem;@{cpu_id=$c.ProcessorId;cpu_name=$c.Name;cpu_max_clock_mhz=$c.MaxClockSpeed;memory_total_bytes=([long]$o.TotalVisibleMemorySize*1024);memory_available_bytes=([long]$o.FreePhysicalMemory*1024);background_cpu_percent=$c.LoadPercentage}|ConvertTo-Json -Compress'
        identity.update(json.loads(subprocess.check_output(['pwsh','-NoProfile','-Command',probe],text=True)))
        system=pathlib.Path(os.environ['SystemRoot'])/'System32'
        identity['system_dlls']={n:sha256((system/n).read_bytes()) for n in
                                 ('ntdll.dll','kernel32.dll','kernelbase.dll','ucrtbase.dll','bcrypt.dll','advapi32.dll')}
        # A shared hosted runner reports a pre-test sample, not a comparable
        # dedicated performance environment. Such runs keep timing tests live.
        identity['timing_runner_class']=None
    return identity

def report_key(path):
    path = str(path).replace('\\', '/')
    marker = 'build/reports/'
    if marker not in path:
        raise ValueError('Evidence file is outside build/reports')
    return marker + path.split(marker, 1)[1]

def validate_archive(raw):
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        members = archive.infolist()
        if sum(m.file_size for m in members) > 256 * 1024 * 1024:
            raise ValueError('Expanded Release evidence exceeds its size limit')
        names = [m.filename for m in members]
        if len(set(names)) != len(names) or any(m.is_dir() or '..' in pathlib.PurePosixPath(m.filename).parts or '\\' in m.filename or m.filename.startswith('/') for m in members):
            raise ValueError('Invalid Release evidence archive paths')
        files = {m.filename: archive.read(m) for m in members}
    att = json.loads(files.pop('attestation.json'))
    if att.get('schema') not in (1,2) or att.get('evidence_mode') != 'executed' or att.get('scope') != 'complete-ci-safe-release':
        raise ValueError('Not a full executed Release attestation')
    if att['files'] != {p: sha256(data) for p, data in files.items()}:
        raise ValueError('Release evidence file digest mismatch')
    snapshot = json.loads(files['source-snapshot.json'])
    bundle = json.loads(files['source-bundle.json'])
    if (sha256(files['source-snapshot.json']) != att['snapshot_sha256'] or snapshot['id'] != att['snapshot_id']
            or snapshot['source']['commit'] != att['commit'] or not snapshot['source']['clean']
            or snapshot['source']['version'] != att['version']
            or snapshot['files']['core-tests/bundle.json'] != sha256(files['source-bundle.json'])
            or bundle['commit'] != att['commit']):
        raise ValueError('Attested commit, Snapshot or Core bundle differs')
    for path, digest in bundle['files'].items():
        if snapshot['files'].get('core-tests/' + path) != digest:
            raise ValueError('Core bundle file map differs from the sealed Snapshot')
    report = json.loads(files[att['report']])
    if att['schema']==2:
        from release_ci import validate_attested_full
        return att,bundle,validate_attested_full(att,files,bundle),files
    if (report['schema'] != 4 or report['level'] != 'RELEASE' or not report['ci_safe']
            or report['status'] != 'passed' or report['commit'] != att['commit'] or report['version'] != att['version'] or report['worktree_dirty']
            or report['parallel']['run_id'] != str(att['run_id'])
            or report['release']['candidate_id'] != att['snapshot_id']
            or report['release']['snapshot_sha256'] != att['snapshot_sha256']):
        raise ValueError('Full Release report identity or execution mode differs')
    if sorted(report['release']['architectures']) != ['x64', 'x86'] or not report['subcases'] or any(c['status'] != 'passed' or not c['attempts'] for c in report['subcases']):
        raise ValueError('Original architecture or harness combination coverage is incomplete')
    expected = {(r['preset'], r['name']) for r in report['selection']['selected']}
    results = [(r['preset'], r['name']) for r in report['results'] if r['status'] == 'passed']
    if not expected or set(results) != expected or len(results) != len(expected) or len(results) != len(report['results']):
        raise ValueError('Full Release JSON coverage is incomplete')
    if att['test_inventory'] != [dict(preset=r['preset'], name=r['name'], evidence_mode='executed') for r in report['results']]:
        raise ValueError('Attested test inventory differs from the original results')
    actual = []
    for junit in report['ctest']:
        if junit['snapshot_id'] != att['snapshot_id'] or junit['snapshot_sha256'] != att['snapshot_sha256']:
            raise ValueError('Old JUnit belongs to another Snapshot')
        for case in ET.fromstring(files[report_key(junit['junit'])]).iter('testcase'):
            if any(case.find(t) is not None for t in ('failure', 'error', 'skipped')) or case.get('status', 'run') not in ('run', 'passed'):
                raise ValueError('Old JUnit contains a failed, skipped or unexecuted test')
            actual.append((junit['preset'], case.attrib['name']))
    if len(actual) != len(expected) or set(actual) != expected or ('windows-x64', 'release.package') not in expected or not any(p == 'windows-x86' for p, _ in expected):
        raise ValueError('Both architectures and Package must have complete passing JUnit')
    permit = json.loads(files[report_key(report['release']['packaging_permit'])])
    if (permit['status'] != 'tests-passed' or permit['schema'] != 4
            or permit['release']['snapshot_sha256'] != att['snapshot_sha256']
            or permit['release']['candidate_id'] != att['snapshot_id']
            or {(r['preset'], r['name']) for r in permit['selection']['selected']} != expected - {('windows-x64', 'release.package')}):
        raise ValueError('Source Packaging Permit differs')
    for receipt in report['parallel']['verification']:
        prefix = report_key(receipt['root']).rstrip('/') + '/'
        observed = {p[len(prefix):]: sha256(data) for p, data in files.items() if p.startswith(prefix) and p != prefix+'evidence.json'}
        if (observed != receipt['files'] or receipt['commit'] != att['commit']
                or receipt['snapshot_id'] != att['snapshot_id'] or receipt['snapshot_sha256'] != att['snapshot_sha256'] or receipt['run_id'] != str(att['run_id'])
                or receipt['status'] != 'passed'):
            raise ValueError('Original architecture receipt is incomplete or changed')
        if json.loads(files[prefix+'evidence.json']) != receipt:
            raise ValueError('Original architecture receipt was rewritten')
    if sorted(r['architecture'] for r in report['parallel']['verification']) != ['x64', 'x86']:
        raise ValueError('Both original architecture receipts are mandatory')
    binaries = {b['name']: b for b in bundle['binaries']}
    seen = set()
    core_results = {}
    for shard in bundle['shards']:
        key = report_key(report['parallel']['core_evidence']).rstrip('/') + '/' + shard['id'] + '/receipt.json'
        receipt = json.loads(files[key])
        if (receipt['schema'] != 1 or receipt['status'] != 'passed' or receipt['commit'] != att['commit']
                or receipt['run_id'] != str(att['run_id']) or receipt['snapshot_id'] != att['snapshot_id']
                or receipt['snapshot_sha256'] != att['snapshot_sha256'] or receipt['bundle_sha256'] != sha256(files['source-bundle.json'])
                or sorted(r['name'] for r in receipt['results']) != sorted(shard['tests'])):
            raise ValueError('Original Core receipt identity, mode or coverage differs')
        for row in receipt['results']:
            name = row['name']; binary = binaries[name]
            if (name in seen or row['status'] != 'passed' or row['executed'] is not True or row['reused'] is not False
                    or row.get('evidence_mode') != 'executed'
                    or row['exit_code'] != 0 or row['executable_sha256'] != bundle['files'][binary['executable']]
                    or row['processes'] != 1 or row['threads'] < 1
                    or row['arguments'] != bundle['skip_arguments'] + [f"--test-threads={row['threads']}"]
                    or (name in ('engine_lm', 'engine_replay', 'engine_newword') and row['threads'] != 1)):
                raise ValueError('Core source result was not fully executed')
            logs = {}
            for stream in ('stdout', 'stderr'):
                filename = row[stream]
                if pathlib.PurePosixPath(filename).name != filename:
                    raise ValueError('Invalid Core log filename')
                data = files[key.rsplit('/', 1)[0] + '/' + filename]
                if sha256(data) != row[stream + '_sha256']:
                    raise ValueError('Original Core log digest mismatch')
                logs[stream] = data
            log = logs['stdout'].decode('utf-8-sig')
            summary = re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;', log)
            if not summary or int(summary[1]) + int(summary[2]) != len(binary['cases']):
                raise ValueError('Original Core assertion count differs')
            for case in binary['cases']:
                if len(re.findall(r'(?m)^test ' + re.escape(case) + r'(?: - should panic)? \.\.\. (?:ok|ignored(?:, [^\r\n]*)?)\r?$', log)) != 1:
                    raise ValueError('Missing or duplicate original Core assertion')
            seen.add(name)
            core_results[name] = (row, receipt, key, logs)
    if len(binaries) != 37 or seen != set(binaries):
        raise ValueError('All 37 original Core binaries are mandatory')
    return att, bundle, core_results, files

def create(root, snapshot_id, output):
    if (os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('GITHUB_EVENT_NAME') != 'push'
            or os.environ.get('GITHUB_REF') != 'refs/heads/main'):
        raise ValueError('Only main push can attest a full Release')
    evidence_root = root / 'build/reports' / f'release-parallel-{snapshot_id}'
    report_path = evidence_root / 'report.json'
    report = json.loads(report_path.read_bytes())
    if report.get('evidence_mode') not in (None,'executed') or report['schema'] not in (4,6):
        return False  # Reused evidence can never establish another full baseline.
    snapshot_root = root / 'build/release-snapshots' / snapshot_id
    files = {p.relative_to(root).as_posix(): p.read_bytes() for p in evidence_root.rglob('*') if p.is_file()}
    files['source-snapshot.json'] = (snapshot_root / 'snapshot.json').read_bytes()
    files['source-bundle.json'] = (snapshot_root / 'core-tests/bundle.json').read_bytes()
    jobs=list(REQUIRED_JOBS)
    if report['schema']==6:
        from release_ci import required_jobs
        files['source-plan.json']=(snapshot_root/'ci-plan.json').read_bytes()
        jobs=required_jobs(json.loads(files['source-plan.json']))
    att = dict(schema=2 if report['schema']==6 else 1, scope='complete-ci-safe-release', evidence_mode='executed', version=report['version'],
               repository=os.environ['GITHUB_REPOSITORY'], commit=report['commit'],
               tree=subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD^{tree}'], text=True).strip(),
               test_input_fingerprint=local_fingerprint(root), snapshot_id=snapshot_id,
               snapshot_sha256=report['release']['snapshot_sha256'],
               run_id=int(os.environ['GITHUB_RUN_ID']), run_attempt=int(os.environ['GITHUB_RUN_ATTEMPT']),
               workflow='.github/workflows/release.yml', required_jobs=jobs,
               workflow_sha256=sha256((root/'.github/workflows/release.yml').read_bytes().replace(b'\r\n',b'\n')),
               test_inventory=[dict(preset=r['preset'], name=r['name'], evidence_mode='executed') for r in report['results']],
               upstream_commit=json.loads((root/'UPSTREAM.json').read_bytes())['commit'],
               model_sources_sha256=sha256((root/'MODEL-SOURCES.json').read_bytes()),
               report=report_path.relative_to(root).as_posix(),
               files={p: sha256(data) for p, data in files.items()})
    files['attestation.json'] = json.dumps(att, indent=2).encode()
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as archive:
        for path, data in files.items():
            archive.writestr(path, data)
    validate_archive(buffer.getvalue())
    output.mkdir(parents=True, exist_ok=True)
    for path, data in files.items():
        dest = output/path; dest.parent.mkdir(parents=True, exist_ok=True); dest.write_bytes(data)
    return True

def fetch_source(api, commit):
    runs = api.request('GET', f'/actions/workflows/release.yml/runs?event=push&branch=main&head_sha={commit}&per_page=100')
    candidates = runs.get('workflow_runs', []) if isinstance(runs, dict) else []
    if not candidates:
        raise ValueError('No full main Release run for github.event.before')
    run = max(candidates, key=lambda r: r['id'])
    if (run['event'] != 'push' or run['head_branch'] != 'main' or run['head_sha'] != commit
            or run['repository']['full_name'] != api.repository or run['head_repository']['full_name'] != api.repository
            or run['path'] != '.github/workflows/release.yml' or run['status'] != 'completed' or run['conclusion'] != 'success'):
        raise ValueError('Source Release is not a successful same-repository main push')
    jobs = api.request('GET', f"/actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100")
    items = jobs.get('jobs', []) if isinstance(jobs, dict) else []
    artifacts = api.request('GET', f"/actions/runs/{run['id']}/artifacts?per_page=100")
    name = f"release-test-attestation-{run['id']}-{run['run_attempt']}"
    matches = [a for a in artifacts.get('artifacts', []) if a['name'] == name] if isinstance(artifacts, dict) else []
    if len(matches) != 1:
        raise ValueError('New-format full Release attestation missing')
    artifact = matches[0]; origin = artifact['workflow_run']
    if (artifact['expired'] is not False or origin['id'] != run['id'] or origin['head_sha'] != commit
            or origin['head_branch'] != 'main' or origin['repository_id'] != run['repository']['id']
            or origin['head_repository_id'] != run['repository']['id']):
        raise ValueError('Source artifact provenance differs')
    raw = api.download_artifact(artifact['id'], max_bytes=MAX_ARTIFACT_BYTES)
    if not raw or artifact.get('digest') != 'sha256:' + sha256(raw):
        raise ValueError('Source artifact digest differs or is unavailable')
    att, bundle, results, _ = validate_archive(raw)
    for name in att['required_jobs']:
        matching=[j for j in items if j['name']==name]
        if len(matching)!=1 or matching[0]['conclusion']!='success' or matching[0]['status']!='completed' or matching[0]['run_attempt']!=run['run_attempt']:
            raise ValueError(f'Required source job did not execute successfully: {name}')
    if (att['repository'] != api.repository or att['commit'] != commit or att['run_id'] != run['id']
            or att['run_attempt'] != run['run_attempt'] or (att['schema']==1 and att['required_jobs'] != list(REQUIRED_JOBS))):
        raise ValueError('Attestation identity differs from its source run')
    source_commit = api.request('GET', f'/git/commits/{commit}')
    if source_commit['sha'] != commit or source_commit['tree']['sha'] != att['tree'] or remote_fingerprint(api, commit) != att['test_input_fingerprint']:
        raise ValueError('Source commit tree or input fingerprint differs')
    provenance = {k: att[k] for k in ('repository', 'commit', 'run_id', 'run_attempt', 'snapshot_id', 'snapshot_sha256', 'test_input_fingerprint')}
    provenance.update(artifact_id=artifact['id'], artifact_digest=artifact['digest'])
    return raw, att, bundle, results, provenance

def prepare(api, root, commit, bundle_root, output):
    if os.environ.get('GITHUB_EVENT_NAME') != 'push' or os.environ.get('GITHUB_REF') != 'refs/heads/main':
        raise ValueError('Manual verify and non-main runs always require Full')
    event_path = os.environ.get('GITHUB_EVENT_PATH')
    if not event_path or json.loads(pathlib.Path(event_path).read_bytes()).get('before') != commit:
        raise ValueError('Reuse source must be exactly github.event.before')
    channel = json.loads((root/'RELEASE-CHANNEL.json').read_bytes())
    if channel['build_linux']:
        raise ValueError('Linux reuse is unproven; both platforms require Full')
    raw, att, old, _, provenance = fetch_source(api, commit)
    if channel['version'] == att['version']:
        raise ValueError('Only a version bump can evaluate historical test reuse')
    if att.get('workflow_sha256') != sha256((root/'.github/workflows/release.yml').read_bytes().replace(b'\r\n',b'\n')):
        raise ValueError('Release workflow differs from the full source baseline')
    if att['commit'] == subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip():
        raise ValueError('Reuse requires a new version commit')
    if local_fingerprint(root) != att['test_input_fingerprint']:
        raise ValueError('Product/test inputs changed beyond approved version metadata')
    current = json.loads((bundle_root/'bundle.json').read_bytes())
    if not current.get('toolchain'):
        raise ValueError('Core compiler and SDK identity unavailable')
    if not {b['executable'] for b in current['binaries']}.issubset(current['files']):
        raise ValueError('New Core executable file map is incomplete')
    for field in ('toolchain', 'source_fingerprint', 'source_key', 'manifest_directory', 'skip_arguments'):
        if current[field] != old[field]:
            raise ValueError(f'Newly compiled Core input is not byte-equivalent: {field}')
    eligible = sorted(b['name'] for b in current['binaries'])
    if not eligible:
        raise ValueError('No proven equivalent test is eligible for reuse')
    output.mkdir(parents=True, exist_ok=True)
    (output/'source.zip').write_bytes(raw)
    _,_,_,files=validate_archive(raw)
    source_report=json.loads(files[att['report']])
    # Only the version-neutral data case uses this worker-side index. Core and
    # the consumer still validate the complete original archive.
    cases={k:c for k,c in source_report.get('ci',{}).get('case_evidence',{}).items() if c['test']=='full.dist-data'}
    index=dict(origin=provenance,cases=cases,
               receipt_hashes={c['receipt_file']:sha256(files[c['receipt_file']]) for c in cases.values()})
    (output/'source-index.json').write_text(json.dumps(index),encoding='utf-8')
    (output/'source.json').write_text(json.dumps(provenance, indent=2), encoding='utf-8')
    return eligible

def reuse_row(snapshot_root, name, current_environment, output):
    source = json.loads((snapshot_root/'ci-reuse/source.json').read_bytes())
    raw = (snapshot_root/'ci-reuse/source.zip').read_bytes()
    if source['artifact_digest'] != 'sha256:' + sha256(raw):
        raise ValueError('Retained original artifact changed')
    att, old, results, files = validate_archive(raw)
    current = json.loads((snapshot_root/'core-tests/bundle.json').read_bytes())
    for field in ('repository', 'commit', 'run_id', 'run_attempt', 'snapshot_id', 'snapshot_sha256', 'test_input_fingerprint'):
        if source[field] != att[field]:
            raise ValueError('Source identity differs from its immutable attestation')
    result=select_core(current,old,current_environment,source,results,files).get(name)
    if result is not None and output is not None:
        for stream, data in results[name][3].items():
            (output/result[stream]).write_bytes(data)
    return result

def core_candidates(source_root, current_bundle, current_environment):
    origin=json.loads((source_root/'source.json').read_bytes());raw=(source_root/'source.zip').read_bytes()
    if origin['artifact_digest']!='sha256:'+sha256(raw):raise ValueError('Source artifact changed')
    att,old,results,files=validate_archive(raw)
    return select_core(current_bundle,old,current_environment,origin,results,files)

def select_core(current_bundle, old, current_environment, origin, results, files):
    from release_ci import environment_matches,TIMING_CORE
    for field in ('toolchain','source_fingerprint','source_key','manifest_directory','skip_arguments'):
        if current_bundle[field]!=old[field]:raise ValueError('New Core binary or complete input manifest differs: '+field)
    old_binaries={b['name']:b for b in old['binaries']}
    current_binaries={b['name']:b for b in current_bundle['binaries']}
    old_exes={b['executable'] for b in old_binaries.values()}
    new_exes={b['executable'] for b in current_binaries.values()}
    old_inputs={p:h for p,h in old['files'].items() if p not in old_exes}
    new_inputs={p:h for p,h in current_bundle['files'].items() if p not in new_exes}
    if old_inputs!=new_inputs:raise ValueError('Core source, DLL, fixture or model inputs changed')
    reused={}
    for name,(row,receipt,path,logs) in results.items():
        binary=current_binaries.get(name)
        if binary!=old_binaries[name] or current_bundle['files'].get(binary['executable'])!=row['executable_sha256']:
            continue
        threads=1 if name in ('engine_lm','engine_replay','engine_newword') else current_environment.get('logical_processors')
        if row['threads']!=threads:continue
        if not environment_matches(row.get('environment',receipt.get('environment')),current_environment,timing=name in TIMING_CORE):continue
        value=copy.deepcopy(row)
        value.update(executed=False,reused=True,evidence_mode='reused',duration_seconds=0,
                     source=dict(origin,receipt_path=path,receipt_sha256=sha256(files[path]),
                                 original_duration_seconds=row['duration_seconds']))
        reused[name]=value
    return reused

def check_current_core(root, snapshot_root, evidence_root):
    reused = 0
    for receipt_path in evidence_root.glob('*/receipt.json'):
        receipt = json.loads(receipt_path.read_bytes())
        for row in receipt['results']:
            if row.get('reused'):
                reused += 1
                expected = reuse_row(snapshot_root, row['name'], receipt['environment'], None)
                if expected is None or row != expected:
                    raise ValueError('Reused test is outside the proven scope or source evidence differs')
    if not reused:
        raise ValueError('Mixed CI schema requires an actual proven reused test')

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['environment', 'create', 'prepare', 'reuse-row', 'check-core','candidates'])
    p.add_argument('--root', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    p.add_argument('--snapshot-id'); p.add_argument('--before'); p.add_argument('--name')
    p.add_argument('--bundle', type=pathlib.Path); p.add_argument('--snapshot-root', type=pathlib.Path)
    p.add_argument('--evidence-root', type=pathlib.Path); p.add_argument('--output', type=pathlib.Path)
    p.add_argument('--environment-file', type=pathlib.Path)
    a = p.parse_args()
    if a.mode == 'environment':
        a.output.write_text(json.dumps(environment()), encoding='utf-8')
    elif a.mode == 'create':
        print('Full executed attestation created' if create(a.root, a.snapshot_id, a.output) else 'Reused run cannot create a full baseline')
    elif a.mode == 'prepare':
        from release import GitHub
        try:
            eligible = prepare(GitHub(os.environ['GITHUB_REPOSITORY'], os.environ['GH_TOKEN']), a.root, a.before, a.bundle, a.output)
            print('Proven reuse candidates: ' + ', '.join(eligible))
        except (ValueError, OSError, KeyError, TypeError, zipfile.BadZipFile, ET.ParseError) as error:
            print(f'Full required: {error}')
    elif a.mode == 'reuse-row':
        row = reuse_row(a.snapshot_root, a.name, json.loads(a.environment_file.read_bytes()), a.output.parent)
        if row is not None:
            a.output.write_text(json.dumps(row, indent=2), encoding='utf-8')
    elif a.mode=='candidates':
        try:
            rows=core_candidates(a.root,json.loads((a.bundle/'bundle.json').read_bytes()),json.loads(a.environment_file.read_bytes()))
        except (ValueError,OSError,KeyError,TypeError,zipfile.BadZipFile,ET.ParseError) as error:
            print(f'Core tests will execute: {error}')
            rows={}
        a.output.write_text(json.dumps(rows,indent=2),encoding='utf-8')
    else:
        check_current_core(a.root, a.snapshot_root, a.evidence_root)

if __name__ == '__main__':
    main()
