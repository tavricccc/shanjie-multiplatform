"""Build artifacts first; resume only a same-source draft and publish after every asset verifies."""
from __future__ import annotations
import argparse, base64, hashlib, io, json, math, os, pathlib, re, subprocess, sys, tomllib, urllib.error, urllib.parse, urllib.request, zipfile
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION_RE = re.compile(r'\d+\.\d+\.\d+(?:-beta\.\d+)?')
STABLE_VERSION_RE = re.compile(r'\d+\.\d+\.\d+')
SHA_RE = re.compile(r'[0-9a-f]{40}')
SHA256_RE = re.compile(r'[0-9a-f]{64}')
ZERO_SHA = '0' * 40
REPO_RE = re.compile(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+')
RELEASE_CHANNEL_KEYS = frozenset({'version', 'prerelease', 'recommended_stable', 'build_linux'})
REQUIRED_CI_WORKFLOW = '.github/workflows/ci.yml'
REQUIRED_CI_JOB = 'unit-tests'
CI_TEST_COMMAND_IDENTITY = 'pwsh -File scripts/test.ps1 -Level CI -CiSafe'
REQUIRED_CI_CTEST_NAMES = (
    'fast.infrastructure',
    'fast.prepare-core',
    'fast.rust.shared',
    'fast.rust.windows',
    'fast.upstream-data',
)
CI_TEST_CONTRACT_PATHS = (
    '.cargo/config.toml',
    '.github/workflows/ci.yml',
    'CMakeLists.txt',
    'CMakePresets.json',
    'linux/scripts/package.sh',
    'linux/scripts/test.sh',
    'scripts/cargo-cache-key.ps1',
    'scripts/cargo-input-cache.ps1',
    'scripts/core-shards.psm1',
    'scripts/ctest-case.ps1',
    'scripts/model_catalog/catalog.py',
    'scripts/model_catalog/probe.ps1',
    'scripts/model_catalog/test_catalog.py',
    'scripts/msvc-command.ps1',
    'scripts/prepare-candidates.ps1',
    'scripts/prepare-core.ps1',
    'scripts/prepare-english.ps1',
    'scripts/prepare-keys.ps1',
    'scripts/prepare-lm.ps1',
    'scripts/prepare-memory.ps1',
    'scripts/prepare-resident.ps1',
    'scripts/prepare-selection-learning.ps1',
    'scripts/prepare-smart-input.ps1',
    'scripts/prepare-test-payload.ps1',
    'scripts/preserve-output.ps1',
    'scripts/release-support.psm1',
    'scripts/release.py',
    'scripts/release_artifacts.py',
    'scripts/release_linux.py',
    'scripts/runner-diagnostics.ps1',
    'scripts/sync-upstream.ps1',
    'scripts/test-plan.psm1',
    'scripts/test-support.psm1',
    'scripts/test.ps1',
    'scripts/test_release.py',
    'scripts/verify-changed.ps1',
    'scripts/verify-selection.ps1',
    'scripts/verify-upstream-data.ps1',
    'tests/ctest_integration.ps1',
    'tests/release_guard.ps1',
    'tests/release_parallel.ps1',
    'tests/test_infrastructure.ps1',
    'tests/upstream_sources.ps1',
    'tests/verification_paths.ps1',
    'tests/verify_selection.ps1',
)
CI_CONTEXT_FILENAME = 'ci-context.json'
CI_CONTEXT_ARTIFACT_PREFIX = 'ci-context-'
CI_CONTEXT_MAX_ZIP_BYTES = 1024 * 1024
CI_CONTEXT_MAX_JSON_BYTES = 65536
CI_CONTEXT_KEYS = frozenset({
    'repository',
    'pull_request_number',
    'base_ref',
    'base_sha',
    'head_repository',
    'head_ref',
    'head_sha',
    'checkout_sha',
    'checkout_tree_sha',
    'checkout_parent_shas',
    'ci_workflow_sha256',
    'ci_test_contract_sha256',
    'test_command_identity',
    'ctest_inventory',
    'ctest_results',
    'ctest_junit_sha256',
    'run_id',
    'run_attempt',
    'job_name',
    'test_result',
})

class _NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

def filehash(path):
    with pathlib.Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

class GitHub:
    def __init__(self, repository, token):
        if not REPO_RE.fullmatch(repository):
            raise ValueError('Invalid repository')
        self.repository = repository
        self.base = f'https://api.github.com/repos/{repository}'
        self.token = token

    def request(self, method, path, body=None):
        raw = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(self.base + path, data=raw, method=method, headers={
            'User-Agent': 'Shanjie-Release', 'Accept': 'application/vnd.github+json',
            'Authorization': 'Bearer ' + self.token, 'Content-Type': 'application/json',
            'X-GitHub-Api-Version': '2022-11-28'})
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if method == 'GET' and error.code == 404:
                return None
            raise

    def upload(self, release, path):
        # Never trust an upload URL supplied by release metadata to receive the token.
        repository = self.base.removeprefix('https://api.github.com/repos/')
        url = f'https://uploads.github.com/repos/{repository}/releases/{release["id"]}/assets?'
        url += urllib.parse.urlencode({'name': path.name})
        with path.open('rb') as stream:
            request = urllib.request.Request(url, data=stream, method='POST', headers={
                'User-Agent': 'Shanjie-Release', 'Authorization': 'Bearer ' + self.token,
                'Content-Type': 'application/octet-stream', 'Content-Length': str(path.stat().st_size)})
            with urllib.request.urlopen(request, timeout=300) as response:
                return json.load(response)

    def download_artifact(self, artifact_id):
        if type(artifact_id) is not int or artifact_id <= 0:
            raise ValueError('Invalid artifact ID')
        # Never send the GitHub token to an untrusted or redirected storage URL.
        url = f'{self.base}/actions/artifacts/{artifact_id}/zip'
        request = urllib.request.Request(url, method='GET', headers={
            'User-Agent': 'Shanjie-Release', 'Accept': 'application/vnd.github+json',
            'Authorization': 'Bearer ' + self.token, 'X-GitHub-Api-Version': '2022-11-28'})
        opener = urllib.request.build_opener(_NoRedirectHandler())
        redirect_url = None
        try:
            with opener.open(request, timeout=60) as response:
                data = response.read(CI_CONTEXT_MAX_ZIP_BYTES + 1)
                if len(data) > CI_CONTEXT_MAX_ZIP_BYTES:
                    raise ValueError('CI context artifact exceeds size limit')
                return data
        except urllib.error.HTTPError as error:
            try:
                if error.code in (404, 410):
                    return None
                if error.code in (301, 302, 303, 307, 308):
                    redirect_url = error.headers.get('Location')
                    if not redirect_url or not redirect_url.startswith('https://'):
                        raise ValueError('Artifact redirect URL is missing or non-HTTPS') from error
                else:
                    raise
            finally:
                error.close()
        download = urllib.request.Request(redirect_url, method='GET', headers={'User-Agent': 'Shanjie-Release'})
        try:
            with urllib.request.urlopen(download, timeout=60) as response:
                data = response.read(CI_CONTEXT_MAX_ZIP_BYTES + 1)
                if len(data) > CI_CONTEXT_MAX_ZIP_BYTES:
                    raise ValueError('CI context artifact exceeds size limit')
                return data
        except urllib.error.HTTPError as error:
            try:
                if error.code in (404, 410):
                    return None
                raise
            finally:
                error.close()

def configuration(root, sha):
    if not re.fullmatch('[0-9a-f]{40}', sha):
        raise ValueError('A full source commit is required')
    channel = json.loads((root / 'RELEASE-CHANNEL.json').read_text(encoding='utf-8-sig'))
    match = re.search(r'^version = "([^"]+)"', (root / 'windows/Cargo.toml').read_text(), re.M)
    if not match or channel['version'] != match[1] or type(channel['prerelease']) is not bool or type(channel['build_linux']) is not bool:
        raise ValueError('Release channel does not match the source version')
    version = match[1]
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-beta\.\d+)?', version):
        raise ValueError('Invalid release version')
    if channel['prerelease'] != ('-beta.' in version):
        raise ValueError('Release channel does not match the version suffix')
    notes = root / f'docs/releases/RELEASE-NOTES-{version}.md'
    return channel | {'tag': 'v' + version, 'sha': sha, 'notes': notes.read_text(encoding='utf-8-sig')}

def compute_ci_test_contract_sha256(contract_blobs):
    missing = [path for path in CI_TEST_CONTRACT_PATHS if path not in contract_blobs]
    if missing:
        raise ValueError(f'Missing CI test contract file(s): {", ".join(missing)}')
    parts = [f'v1\ncommand={CI_TEST_COMMAND_IDENTITY}\n']
    for path in CI_TEST_CONTRACT_PATHS:
        blob = contract_blobs[path]
        if not isinstance(blob, (bytes, bytearray)):
            raise ValueError(f'Invalid CI test contract content for {path}')
        parts.append(f'{path}\0{hashlib.sha256(blob).hexdigest()}\n')
    return hashlib.sha256(''.join(parts).encode('utf-8')).hexdigest()

def _validate_ci_ctest_evidence(ctest_inventory, ctest_results, ctest_junit_sha256):
    if (not isinstance(ctest_inventory, list)
            or any(type(name) is not str or not name for name in ctest_inventory)
            or len(set(ctest_inventory)) != len(ctest_inventory)
            or ctest_inventory != list(REQUIRED_CI_CTEST_NAMES)):
        return 'ci_ctest_inventory_incomplete'
    if not isinstance(ctest_results, list):
        return 'ci_ctest_results_invalid'
    if any(not isinstance(item, dict) or type(item.get('name')) is not str or not item['name'] for item in ctest_results):
        return 'ci_ctest_results_invalid'
    result_names = [item['name'] for item in ctest_results]
    if len(set(result_names)) != len(result_names) or sorted(result_names) != list(REQUIRED_CI_CTEST_NAMES):
        return 'ci_ctest_inventory_incomplete'
    required_result_keys = {'name', 'preset', 'status', 'duration_seconds'}
    for item in ctest_results:
        if set(item) != required_result_keys:
            return 'ci_ctest_results_invalid'
        if item['preset'] != 'windows-x64' or item['status'] != 'passed':
            return 'ci_ctest_results_invalid'
        duration = item['duration_seconds']
        if type(duration) not in (int, float) or not math.isfinite(duration) or duration < 0:
            return 'ci_ctest_results_invalid'
    if not isinstance(ctest_junit_sha256, dict) or not ctest_junit_sha256:
        return 'ci_ctest_results_invalid'
    for name, digest in ctest_junit_sha256.items():
        if (type(name) is not str or not name or '\\' in name or name.startswith('/')
                or '..' in pathlib.PurePosixPath(name).parts):
            return 'ci_ctest_results_invalid'
        if type(digest) is not str or not SHA256_RE.fullmatch(digest):
            return 'ci_ctest_results_invalid'
    return None

def read_ci_test_report(root, checkout_sha, report_path=None):
    root = pathlib.Path(root)
    reports_root = (root / 'build/reports').resolve()
    if report_path is not None:
        report_file = pathlib.Path(report_path)
        if not report_file.is_file():
            raise ValueError(f'CI test report not found: {report_file}')
        report = json.loads(report_file.read_text(encoding='utf-8-sig'))
    else:
        candidates = []
        if reports_root.is_dir():
            for candidate in reports_root.glob('ci-*/report.json'):
                if not candidate.is_file():
                    continue
                try:
                    parsed = json.loads(candidate.read_text(encoding='utf-8-sig'))
                except (OSError, UnicodeDecodeError, json.JSONDecodeError):
                    continue
                if isinstance(parsed, dict) and parsed.get('commit') == checkout_sha:
                    candidates.append(((candidate.parent.name, candidate.stat().st_mtime_ns), candidate, parsed))
        if not candidates:
            raise ValueError('Missing passing CI test report for checkout HEAD')
        _, report_file, report = max(candidates, key=lambda item: item[0])
    if not isinstance(report, dict):
        raise ValueError('Invalid CI test report payload')
    if (report.get('level') != 'CI'
            or report.get('requested_level') != 'CI'
            or report.get('architecture') != 'x64'
            or report.get('ci_safe') is not True
            or report.get('plan_only') is not False
            or report.get('status') != 'passed'
            or report.get('commit') != checkout_sha):
        raise ValueError('CI test report does not represent a passing CI baseline run for HEAD')
    selection = report.get('selection')
    selected = selection.get('selected') if isinstance(selection, dict) else None
    if not isinstance(selected, list):
        raise ValueError('CI test report is missing selection.selected')
    selected_names = []
    for item in selected:
        if not isinstance(item, dict) or type(item.get('name')) is not str or item.get('preset') != 'windows-x64':
            raise ValueError('CI test report contains an invalid selected CTest entry')
        selected_names.append(item['name'])
    if len(set(selected_names)) != len(selected_names) or sorted(selected_names) != list(REQUIRED_CI_CTEST_NAMES):
        raise ValueError('CI test report selected CTest inventory is incomplete or unexpected')
    raw_results = report.get('results')
    if not isinstance(raw_results, list):
        raise ValueError('CI test report is missing results')
    normalized_results = []
    for item in raw_results:
        if not isinstance(item, dict) or type(item.get('name')) is not str:
            raise ValueError('CI test report contains an invalid CTest result entry')
        if item.get('preset') != 'windows-x64' or item.get('status') != 'passed':
            raise ValueError(f'CI CTest case did not pass: {item.get("name")}')
        duration = item.get('duration_seconds')
        if type(duration) not in (int, float) or not math.isfinite(duration) or duration < 0:
            raise ValueError(f'CI CTest case has invalid duration: {item.get("name")}')
        normalized_results.append({
            'name': item['name'],
            'preset': 'windows-x64',
            'status': 'passed',
            'duration_seconds': round(float(duration), 3),
        })
    normalized_results.sort(key=lambda item: item['name'])
    result_names = [item['name'] for item in normalized_results]
    if len(set(result_names)) != len(result_names) or result_names != list(REQUIRED_CI_CTEST_NAMES):
        raise ValueError('CI test report results do not match the required CI CTest inventory')
    raw_ctest = report.get('ctest')
    if not isinstance(raw_ctest, list):
        raise ValueError('CI test report is missing ctest JUnit metadata')
    ctest_entries = [entry for entry in raw_ctest if isinstance(entry, dict) and not entry.get('diagnostic')]
    if not ctest_entries:
        raise ValueError('CI test report does not reference any non-diagnostic CTest JUnit files')
    junit_hashes = {}
    junit_passed = []
    report_dir = report_file.resolve().parent
    for entry in ctest_entries:
        if entry.get('preset') != 'windows-x64' or type(entry.get('junit')) is not str or not entry['junit']:
            raise ValueError('CI test report contains an invalid ctest JUnit entry')
        raw_junit = pathlib.Path(entry['junit'])
        junit_path = raw_junit if raw_junit.is_absolute() else (root / raw_junit)
        if junit_path.is_symlink() or not junit_path.is_file():
            raise ValueError(f'CI CTest JUnit file is missing: {junit_path}')
        resolved_junit = junit_path.resolve()
        if not resolved_junit.is_relative_to(reports_root):
            raise ValueError(f'CI CTest JUnit file must live under build/reports: {junit_path}')
        rel_key = (
            resolved_junit.relative_to(report_dir).as_posix()
            if resolved_junit.is_relative_to(report_dir)
            else resolved_junit.relative_to(reports_root).as_posix()
        )
        junit_hashes[rel_key] = filehash(resolved_junit)
        cases = ET.parse(resolved_junit).findall('.//testcase')
        if not cases:
            raise ValueError(f'CI CTest JUnit file contains no testcase elements: {junit_path}')
        for case in cases:
            name = case.attrib.get('name')
            if not name:
                raise ValueError(f'CI CTest JUnit testcase is missing name: {junit_path}')
            status_attr = case.attrib.get('status')
            if status_attr is not None and status_attr not in ('run', 'passed'):
                raise ValueError(f'CI CTest JUnit testcase did not run cleanly: {name} ({status_attr})')
            if any(case.find(tag) is not None for tag in ('failure', 'error', 'skipped')):
                raise ValueError(f'CI CTest JUnit testcase failed or was skipped: {name}')
            junit_passed.append(name)
    if len(set(junit_passed)) != len(junit_passed) or sorted(junit_passed) != list(REQUIRED_CI_CTEST_NAMES):
        raise ValueError('CI CTest JUnit inventory is incomplete or duplicated')
    return {
        'ctest_inventory': list(REQUIRED_CI_CTEST_NAMES),
        'ctest_results': normalized_results,
        'ctest_junit_sha256': junit_hashes,
    }

def build_ci_context(root, env, report_path=None):
    root = pathlib.Path(root)
    def require_str(name, fallback=None):
        value = (env.get(name) or (env.get(fallback) if fallback else '') or '').strip()
        if not value:
            raise ValueError(f'Missing required CI context variable: {name}')
        return value
    def require_int(name, fallback=None):
        raw = require_str(name, fallback)
        if not raw.isdigit() or int(raw) <= 0:
            raise ValueError(f'Invalid positive integer for {name}: {raw!r}')
        return int(raw)
    def require_sha(name):
        value = require_str(name)
        if not SHA_RE.fullmatch(value):
            raise ValueError(f'Invalid commit SHA for {name}: {value!r}')
        return value
    repository = require_str('CI_REPOSITORY', 'GITHUB_REPOSITORY')
    if not REPO_RE.fullmatch(repository):
        raise ValueError(f'Invalid repository in CI context: {repository!r}')
    head_repository = require_str('CI_HEAD_REPOSITORY')
    if not REPO_RE.fullmatch(head_repository):
        raise ValueError(f'Invalid head repository in CI context: {head_repository!r}')
    job_name = require_str('CI_JOB_NAME', 'GITHUB_JOB')
    if job_name != REQUIRED_CI_JOB:
        raise ValueError(f'Unexpected CI job name: {job_name!r}')
    checkout_sha = subprocess.check_output(
        ['git', '-C', str(root), 'rev-parse', '--verify', 'HEAD'], text=True
    ).strip()
    if not SHA_RE.fullmatch(checkout_sha):
        raise ValueError(f'Invalid checkout HEAD SHA: {checkout_sha!r}')
    # Read raw commit headers directly so shallow checkouts preserve true parent SHAs.
    commit_header = subprocess.check_output(
        ['git', '-C', str(root), 'cat-file', 'commit', 'HEAD'], text=True, encoding='utf-8'
    ).split('\n\n', 1)[0]
    tree_shas = []
    parent_shas = []
    for line in commit_header.splitlines():
        if line.startswith('tree '):
            tree_shas.append(line.removeprefix('tree ').strip())
        elif line.startswith('parent '):
            parent_shas.append(line.removeprefix('parent ').strip())
    if len(tree_shas) != 1 or not SHA_RE.fullmatch(tree_shas[0]):
        raise ValueError('Unable to determine checkout tree SHA from HEAD commit')
    if not parent_shas or any(not SHA_RE.fullmatch(parent) for parent in parent_shas):
        raise ValueError('Unable to determine valid checkout parent SHAs from HEAD commit')
    contract_blobs = {}
    for rel_path in CI_TEST_CONTRACT_PATHS:
        blob = subprocess.check_output(['git', '-C', str(root), 'show', f'HEAD:{rel_path}'])
        disk_file = root / rel_path
        if not disk_file.is_file():
            raise ValueError(f'Missing CI test contract file in working tree: {rel_path}')
        working_bytes = disk_file.read_bytes()
        if blob.replace(b'\r\n', b'\n') != working_bytes.replace(b'\r\n', b'\n'):
            if rel_path == REQUIRED_CI_WORKFLOW:
                raise ValueError('Working tree CI workflow differs from checkout HEAD')
            raise ValueError(f'Working tree CI test contract file differs from checkout HEAD: {rel_path}')
        contract_blobs[rel_path] = blob
    workflow_blob = contract_blobs[REQUIRED_CI_WORKFLOW]
    if CI_TEST_COMMAND_IDENTITY.encode('utf-8') not in workflow_blob:
        raise ValueError('CI workflow does not contain the required lightweight CI command')
    ctest_evidence = read_ci_test_report(root, checkout_sha, report_path=report_path)
    return {
        'repository': repository,
        'pull_request_number': require_int('CI_PULL_REQUEST_NUMBER'),
        'base_ref': require_str('CI_BASE_REF'),
        'base_sha': require_sha('CI_BASE_SHA'),
        'head_repository': head_repository,
        'head_ref': require_str('CI_HEAD_REF'),
        'head_sha': require_sha('CI_HEAD_SHA'),
        'checkout_sha': checkout_sha,
        'checkout_tree_sha': tree_shas[0],
        'checkout_parent_shas': parent_shas,
        'ci_workflow_sha256': hashlib.sha256(workflow_blob).hexdigest(),
        'ci_test_contract_sha256': compute_ci_test_contract_sha256(contract_blobs),
        'test_command_identity': CI_TEST_COMMAND_IDENTITY,
        'ctest_inventory': ctest_evidence['ctest_inventory'],
        'ctest_results': ctest_evidence['ctest_results'],
        'ctest_junit_sha256': ctest_evidence['ctest_junit_sha256'],
        'run_id': require_int('CI_RUN_ID', 'GITHUB_RUN_ID'),
        'run_attempt': require_int('CI_RUN_ATTEMPT', 'GITHUB_RUN_ATTEMPT'),
        'job_name': job_name,
        'test_result': 'success',
    }

def write_ci_context(root, output_path, env, report_path=None):
    receipt = build_ci_context(root, env, report_path=report_path)
    output_path = pathlib.Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    return receipt

def marker(config):
    return f'<!-- shanjie-release-source:{config["sha"]} -->'

def existing(api, config):
    tag = urllib.parse.quote(config['tag'], safe='')
    release = api.request('GET', '/releases/tags/' + tag)
    if release and not release['draft']:
        return release  # Published releases are immutable, even after a complete rerun.
    # The tag lookup never returns drafts (a draft has no tag yet), so find them in the list.
    drafts = []
    for page in range(1, 11):
        items = api.request('GET', f'/releases?per_page=100&page={page}') or []
        drafts += [r for r in items if r.get('draft') and r.get('tag_name') == config['tag']]
        if len(items) < 100:
            break
    else:
        raise ValueError('Too many releases to search for a draft')
    if len(drafts) > 1:
        raise ValueError('Several drafts share this tag; review them before publishing: '
                         + ', '.join(str(d.get('id')) for d in drafts))
    release = drafts[0] if drafts else None
    ref = api.request('GET', '/git/ref/tags/' + tag)
    if ref:
        obj = ref['object']
        for _ in range(5):
            if obj['type'] != 'tag':
                break
            obj = api.request('GET', '/git/tags/' + obj['sha'])['object']
        if obj['type'] != 'commit' or obj['sha'] != config['sha']:
            raise ValueError('Tag belongs to another source commit')
        if not release:
            raise ValueError('Tag exists without a release; review it before publishing')
    if release and (marker(config) not in (release.get('body') or '')
                    or release.get('target_commitish') != config['sha']
                    or release.get('tag_name') != config['tag']):
        raise ValueError('Draft source is missing or differs; refusing to modify it')
    return release

def plan(api, config, verify):
    return not verify and not ((release := existing(api, config)) and not release['draft'])

def manifest(config, platform, folder):
    pattern = 'Shanjie-Windows-Setup.exe' if platform == 'windows' else 'shanjie-linux-*.tar.gz'
    packages = list(folder.glob(pattern))
    if len(packages) != 1:
        raise ValueError('Expected one installer/package')
    package = packages[0]
    checksum = package.with_name(package.name + '.sha256')
    if checksum.read_text(encoding='utf-8-sig').strip() != f'{filehash(package)}  {package.name}':
        raise ValueError('Package checksum does not match')
    data = {'sha': config['sha'], 'tag': config['tag'], 'platform': platform,
            'assets': [{'name': p.name, 'size': p.stat().st_size, 'sha256': filehash(p)} for p in [package, checksum]]}
    (folder / 'release-source.json').write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8')

def assets_for(config, folder, platform=None):
    files = {}
    platforms = set()
    for path in folder.rglob('release-source.json'):
        data = json.loads(path.read_text(encoding='utf-8'))
        if data['sha'] != config['sha'] or data['tag'] != config['tag'] or data['platform'] in platforms:
            raise ValueError('Artifact source, version or platform differs')
        platforms.add(data['platform'])
        if len(data['assets']) != 2:
            raise ValueError('Artifact must contain package and checksum')
        for asset in data['assets']:
            name = asset['name']
            if name in files or pathlib.Path(name).name != name or '/' in name or '\\' in name:
                raise ValueError('Invalid or duplicate asset name')
            source = path.parent / name
            if not source.is_file() or source.is_symlink() or source.stat().st_size != asset['size'] or filehash(source) != asset['sha256']:
                raise ValueError('Artifact content differs from its manifest')
            files[name] = (source, asset)
        # Recheck the relationship, not just two independently valid hashes.
        pattern = 'Shanjie-Windows-Setup.exe' if data['platform'] == 'windows' else 'shanjie-linux-*.tar.gz'
        packages = list(path.parent.glob(pattern))
        if len(packages) != 1 or set(a['name'] for a in data['assets']) != {packages[0].name, packages[0].name + '.sha256'}:
            raise ValueError('Unexpected release asset names')
        checksum = packages[0].with_name(packages[0].name + '.sha256')
        if checksum.read_text(encoding='utf-8-sig').strip() != f'{filehash(packages[0])}  {packages[0].name}':
            raise ValueError('Artifact checksum does not match its package')
    expected = {platform} if platform else ({'windows', 'linux'} if config['build_linux'] else {'windows'})
    if platforms != expected:
        raise ValueError('Missing or unexpected platform artifacts')
    return files

def check_remote(remote, expected):
    if remote.get('state') != 'uploaded' or remote.get('size') != expected['size'] or remote.get('digest') != 'sha256:' + expected['sha256']:
        raise ValueError('Existing/uploaded asset content differs or has no verifiable digest')

def publish(api, config, folder):
    files = assets_for(config, folder)  # No remote mutation before all local artifacts verify.
    release = existing(api, config)
    if release and not release['draft']:
        print('Release already public; no changes')
        return
    if not release:
        release = api.request('POST', '/releases', {
            'tag_name': config['tag'], 'target_commitish': config['sha'],
            'name': '善解輸入法 ' + config['version'], 'body': config['notes'] + '\n\n' + marker(config),
            'draft': True, 'prerelease': config['prerelease'], 'make_latest': 'false'})
    route = f'/releases/{release["id"]}'
    def remote_assets():
        items = api.request('GET', route + '/assets?per_page=100')
        if len(items) != len({a['name'] for a in items}) or any(a['name'] not in files for a in items):
            raise ValueError('Draft has duplicate or unexpected assets; manual review required')
        return {a['name']: a for a in items}
    present = remote_assets()
    # Check every preexisting asset before uploading any missing one.
    for name, remote in present.items():
        check_remote(remote, files[name][1])
    for name, (path, expected) in files.items():
        if name not in present:
            check_remote(api.upload(release, path), expected)
    present = remote_assets()
    if present.keys() != files.keys():
        raise ValueError('Draft is incomplete')
    for name, remote in present.items():
        check_remote(remote, files[name][1])
    # Recheck source/visibility just before the single publication mutation.
    current = existing(api, config)
    if not current or not current['draft'] or current['id'] != release['id']:
        raise ValueError('Release state changed during upload')
    api.request('PATCH', route, {'draft': False, 'prerelease': config['prerelease'],
                               'make_latest': 'false' if config['prerelease'] else 'true'})

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['plan', 'manifest', 'check', 'publish', 'ci-context'])
    parser.add_argument('--root', type=pathlib.Path, default=ROOT)
    parser.add_argument('--folder', type=pathlib.Path)
    parser.add_argument('--output', type=pathlib.Path)
    parser.add_argument('--report', type=pathlib.Path)
    parser.add_argument('--platform', choices=['windows', 'linux'])
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    if args.mode == 'ci-context':
        if not args.output:
            raise ValueError('--output is required for ci-context mode')
        receipt = write_ci_context(args.root, args.output, os.environ, report_path=args.report)
        print(f"Wrote CI context receipt: pr=#{receipt['pull_request_number']} run={receipt['run_id']} attempt={receipt['run_attempt']} tree={receipt['checkout_tree_sha']} contract={receipt['ci_test_contract_sha256'][:12]}")
        return
    config = configuration(args.root, os.environ.get('GITHUB_SHA', ''))
    if args.mode == 'manifest':
        manifest(config, args.platform, args.folder)
        return
    if args.mode == 'check':
        files = assets_for(config, args.folder, args.platform)
        print(f'Verified {len(files)} release assets from {args.platform or "the required platforms"}')
        return
    api = GitHub(os.environ['GITHUB_REPOSITORY'], os.environ['GH_TOKEN'])
    if args.mode == 'plan':
        outputs = {'tag': config['tag'], 'build_linux': config['build_linux'], 'publish': plan(api, config, args.verify)}
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as stream:
            for key, value in outputs.items():
                print(f'{key}={str(value).lower() if isinstance(value, bool) else value}', file=stream)
    else:
        publish(api, config, args.folder)

if __name__ == '__main__':
    main()
