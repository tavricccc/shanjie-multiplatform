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
    'scripts/release-ci.psm1',
    'scripts/release-costs.json',
    'scripts/release-support.psm1',
    'scripts/release.py',
    'scripts/release_artifacts.py',
    'scripts/release_ci.py',
    'scripts/release_evidence.py',
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

    def download_artifact(self, artifact_id, max_bytes=CI_CONTEXT_MAX_ZIP_BYTES):
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
                data = response.read(max_bytes + 1)
                if len(data) > max_bytes:
                    raise ValueError('Artifact exceeds size limit')
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
                data = response.read(max_bytes + 1)
                if len(data) > max_bytes:
                    raise ValueError('Artifact exceeds size limit')
                return data
        except urllib.error.HTTPError as error:
            try:
                if error.code in (404, 410):
                    return None
                raise
            finally:
                error.close()

def validate_release_channel(channel, context='current commit'):
    if not isinstance(channel, dict):
        raise ValueError(f'release_channel_policy_conflict: invalid RELEASE-CHANNEL.json object in {context}')
    unknown_keys = sorted(set(channel) - RELEASE_CHANNEL_KEYS)
    if unknown_keys:
        raise ValueError(
            f'release_channel_policy_conflict: unknown RELEASE-CHANNEL.json field(s) in {context}: '
            f'{", ".join(unknown_keys)}; update release policy handling or bump to a new version before releasing'
        )
    missing_keys = sorted(RELEASE_CHANNEL_KEYS - set(channel))
    if missing_keys:
        raise ValueError(
            f'release_channel_policy_conflict: RELEASE-CHANNEL.json in {context} is missing required field(s): '
            f'{", ".join(missing_keys)}'
        )
    version = channel.get('version')
    if type(version) is not str or not VERSION_RE.fullmatch(version):
        raise ValueError('Invalid release version')
    if type(channel.get('prerelease')) is not bool or type(channel.get('build_linux')) is not bool:
        raise ValueError('Release channel does not match the source version')
    recommended_stable = channel.get('recommended_stable')
    if type(recommended_stable) is not str or not STABLE_VERSION_RE.fullmatch(recommended_stable):
        raise ValueError(
            f'release_channel_policy_conflict: invalid recommended_stable in RELEASE-CHANNEL.json ({context})'
        )
    if channel['prerelease'] != ('-beta.' in version):
        raise ValueError(
            'release_channel_policy_conflict: RELEASE-CHANNEL.json prerelease does not match the version suffix; '
            'bump to a new version or use gh release edit for an already published release'
        )
    return dict(channel)

def parse_release_version(channel_text, cargo_text, lock_text, csproj_text):
    channel = json.loads(channel_text)
    channel = validate_release_channel(channel)
    cargo = tomllib.loads(cargo_text)
    cargo_version = cargo.get('package', {}).get('version')
    lock = tomllib.loads(lock_text)
    lock_versions = [pkg.get('version') for pkg in lock.get('package', []) if pkg.get('name') == 'shanjie-windows']
    if len(lock_versions) != 1:
        raise ValueError('Cargo.lock must contain one shanjie-windows package')
    ui_root = ET.fromstring(csproj_text)
    ui_versions = [el.text.strip() for el in ui_root.findall('./PropertyGroup/Version') if el.text and el.text.strip()]
    if len(ui_versions) != 1:
        raise ValueError('Shanjie.UI.csproj must contain one Version element')
    version = channel['version']
    if cargo_version != version or lock_versions[0] != version or ui_versions[0] != version:
        raise ValueError('Release versions in Cargo, Cargo.lock, WinUI and RELEASE-CHANNEL.json must match')
    return channel

def configuration(root, sha):
    if not SHA_RE.fullmatch(sha):
        raise ValueError('A full source commit is required')
    channel = parse_release_version(
        (root / 'RELEASE-CHANNEL.json').read_text(encoding='utf-8-sig'),
        (root / 'windows/Cargo.toml').read_text(encoding='utf-8-sig'),
        (root / 'windows/Cargo.lock').read_text(encoding='utf-8-sig'),
        (root / 'ui/Shanjie.UI.csproj').read_text(encoding='utf-8-sig'),
    )
    version = channel['version']
    notes = root / f'docs/releases/RELEASE-NOTES-{version}.md'
    return channel | {'channel': dict(channel), 'tag': 'v' + version, 'sha': sha, 'notes': notes.read_text(encoding='utf-8-sig')}

def read_remote_file(api, sha, path):
    raw = read_remote_blob_bytes(api, sha, path)
    if raw is None:
        raise ValueError(f'before_commit_unavailable: unable to read {path} at commit {sha}')
    try:
        return raw.decode('utf-8-sig')
    except UnicodeDecodeError as error:
        raise ValueError(f'before_commit_unavailable: invalid content for {path} at commit {sha}') from error

def read_remote_blob_bytes(api, sha, path):
    encoded_path = urllib.parse.quote(path, safe='/')
    item = api.request('GET', f'/contents/{encoded_path}?ref={sha}')
    if (not isinstance(item, dict) or item.get('type') != 'file'
            or item.get('encoding') != 'base64' or not isinstance(item.get('content'), str)):
        return None
    try:
        return base64.b64decode(item['content'])
    except ValueError:
        return None

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

def read_before_channel(api, before):
    if not before or not SHA_RE.fullmatch(before) or before == ZERO_SHA:
        raise ValueError('before_commit_unavailable: push event requires a valid non-zero before commit SHA')
    channel_text = read_remote_file(api, before, 'RELEASE-CHANNEL.json')
    cargo_text = read_remote_file(api, before, 'windows/Cargo.toml')
    lock_text = read_remote_file(api, before, 'windows/Cargo.lock')
    csproj_text = read_remote_file(api, before, 'ui/Shanjie.UI.csproj')
    try:
        return parse_release_version(channel_text, cargo_text, lock_text, csproj_text)
    except (ValueError, ET.ParseError) as error:
        if str(error).startswith('release_channel_policy_conflict:'):
            raise ValueError(
                f'release_channel_policy_conflict: before commit {before} has invalid RELEASE-CHANNEL.json policy: {error}'
            ) from error
        raise ValueError(f'before_commit_unavailable: invalid version metadata at commit {before}: {error}') from error

def read_before_version(api, before):
    return read_before_channel(api, before)['version']

def channel_policy_from_config(config):
    raw = config['channel'] if 'channel' in config else {
        key: value for key, value in config.items() if key not in ('tag', 'sha', 'notes')
    }
    return validate_release_channel(raw, context='current commit')

def compare_release_channel_policy(before_channel, current_channel, tag):
    validated_current = validate_release_channel(current_channel, context='current commit')
    validated_before = validate_release_channel(before_channel, context='before commit')
    changed = sorted(
        key for key in (set(validated_before) | set(validated_current))
        if key != 'version' and (
            key not in validated_before
            or key not in validated_current
            or type(validated_before[key]) is not type(validated_current[key])
            or validated_before[key] != validated_current[key]
        )
    )
    if changed:
        raise ValueError(
            f'release_channel_policy_conflict: RELEASE-CHANNEL.json non-version policy changed '
            f'({", ".join(changed)}) while version stayed at already published release {tag}; '
            'published releases cannot be updated in place — bump to a new version or handle channel policy '
            'separately (for example with gh release edit)'
        )

def _parse_ci_context_zip(raw_zip):
    try:
        with zipfile.ZipFile(io.BytesIO(raw_zip)) as archive:
            entries = archive.infolist()
            if len(entries) != 1:
                return None
            entry = entries[0]
            if entry.is_dir() or entry.filename != CI_CONTEXT_FILENAME or entry.file_size > CI_CONTEXT_MAX_JSON_BYTES:
                return None
            raw = archive.read(entry)
            if len(raw) > CI_CONTEXT_MAX_JSON_BYTES:
                return None
            data = json.loads(raw.decode('utf-8'))
    except (zipfile.BadZipFile, UnicodeDecodeError, json.JSONDecodeError, ValueError):
        return None
    if not isinstance(data, dict) or set(data) != CI_CONTEXT_KEYS:
        return None
    return data

def verify_merged_pr_ci(api, sha, target_branch='main', before=None):
    expected_repo = getattr(api, 'repository', None)
    if not isinstance(expected_repo, str) or not REPO_RE.fullmatch(expected_repo):
        return {'verified': False, 'detail': 'repository_identity_unavailable'}
    pulls = api.request('GET', f'/commits/{sha}/pulls?per_page=100')
    if not isinstance(pulls, list):
        return {'verified': False, 'detail': 'commit_pulls_unavailable'}
    matching_prs = [
        pr for pr in pulls
        if isinstance(pr, dict)
        and bool(pr.get('merged_at'))
        and isinstance(pr.get('base'), dict)
        and pr['base'].get('ref') == target_branch
        and pr.get('merge_commit_sha') == sha
        and isinstance(pr.get('head'), dict)
        and isinstance(pr['head'].get('sha'), str)
        and bool(SHA_RE.fullmatch(pr['head']['sha']))
    ]
    if len(matching_prs) != 1:
        return {'verified': False, 'detail': 'no_unique_merged_pr'}
    pr = matching_prs[0]
    pr_number = pr.get('number')
    pr_base_sha = pr['base'].get('sha')
    pr_head_ref = pr['head'].get('ref')
    pr_head_repo_obj = pr['head'].get('repo')
    pr_head_repo = pr_head_repo_obj.get('full_name') if isinstance(pr_head_repo_obj, dict) else None
    head_sha = pr['head']['sha']
    if type(pr_number) is not int or pr_number <= 0:
        return {'verified': False, 'detail': 'pr_metadata_incomplete'}
    if not isinstance(pr_base_sha, str) or not SHA_RE.fullmatch(pr_base_sha):
        return {'verified': False, 'detail': 'pr_base_sha_unavailable', 'pr_number': pr_number}
    if not isinstance(pr_head_ref, str) or not pr_head_ref:
        return {'verified': False, 'detail': 'pr_head_ref_unavailable', 'pr_number': pr_number}
    if not isinstance(pr_head_repo, str) or not REPO_RE.fullmatch(pr_head_repo):
        return {'verified': False, 'detail': 'pr_head_repo_unavailable', 'pr_number': pr_number}
    query = urllib.parse.urlencode({'head_sha': head_sha, 'event': 'pull_request', 'per_page': 100})
    runs_payload = api.request('GET', f'/actions/workflows/ci.yml/runs?{query}')
    if not isinstance(runs_payload, dict) or not isinstance(runs_payload.get('workflow_runs'), list):
        return {'verified': False, 'detail': 'ci_workflow_runs_unavailable', 'pr_number': pr_number}
    candidate_runs = [
        run for run in runs_payload['workflow_runs']
        if isinstance(run, dict)
        and run.get('event') == 'pull_request'
        and run.get('head_sha') == head_sha
        and run.get('path') == REQUIRED_CI_WORKFLOW
        and type(run.get('id')) is int
    ]
    if not candidate_runs:
        return {'verified': False, 'detail': 'ci_run_missing', 'pr_number': pr_number}
    latest_run = max(candidate_runs, key=lambda run: (str(run.get('created_at') or ''), run['id']))
    run_id = latest_run['id']
    run_attempt = latest_run.get('run_attempt')
    if type(run_attempt) is not int or run_attempt <= 0:
        return {'verified': False, 'detail': 'ci_run_attempt_invalid', 'pr_number': pr_number, 'ci_run_id': run_id}
    run_repo = latest_run.get('repository')
    run_repo_name = run_repo.get('full_name') if isinstance(run_repo, dict) else None
    run_repo_id = run_repo.get('id') if isinstance(run_repo, dict) else None
    if run_repo_name != expected_repo:
        return {'verified': False, 'detail': 'ci_run_repository_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    run_head_repo_obj = latest_run.get('head_repository')
    run_head_repo = run_head_repo_obj.get('full_name') if isinstance(run_head_repo_obj, dict) else None
    if run_head_repo != pr_head_repo:
        return {'verified': False, 'detail': 'ci_head_repository_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if latest_run.get('head_branch') != pr_head_ref:
        return {'verified': False, 'detail': 'ci_head_ref_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if latest_run.get('status') != 'completed':
        return {
            'verified': False,
            'detail': f'ci_run_not_completed:{latest_run.get("status")}',
            'pr_number': pr_number,
            'ci_run_id': run_id,
        }
    if latest_run.get('conclusion') != 'success':
        return {
            'verified': False,
            'detail': f'ci_run_conclusion:{latest_run.get("conclusion")}',
            'pr_number': pr_number,
            'ci_run_id': run_id,
        }
    jobs_payload = api.request('GET', f'/actions/runs/{run_id}/jobs?filter=latest&per_page=100')
    if not isinstance(jobs_payload, dict) or not isinstance(jobs_payload.get('jobs'), list):
        return {
            'verified': False,
            'detail': 'ci_jobs_unavailable',
            'pr_number': pr_number,
            'ci_run_id': run_id,
        }
    required_jobs = [
        job for job in jobs_payload['jobs']
        if isinstance(job, dict) and job.get('name') == REQUIRED_CI_JOB
    ]
    if not required_jobs or any(
        job.get('status') != 'completed'
        or job.get('conclusion') != 'success'
        or job.get('run_attempt') != run_attempt
        for job in required_jobs
    ):
        return {
            'verified': False,
            'detail': 'ci_required_job_not_succeeded',
            'pr_number': pr_number,
            'ci_run_id': run_id,
        }
    if not before or not SHA_RE.fullmatch(before) or before == ZERO_SHA:
        return {'verified': False, 'detail': 'ci_workflow_untrusted', 'pr_number': pr_number, 'ci_run_id': run_id}
    before_ci_bytes = read_remote_blob_bytes(api, before, REQUIRED_CI_WORKFLOW)
    current_ci_bytes = read_remote_blob_bytes(api, sha, REQUIRED_CI_WORKFLOW)
    if before_ci_bytes is None or current_ci_bytes is None:
        return {'verified': False, 'detail': 'ci_workflow_untrusted', 'pr_number': pr_number, 'ci_run_id': run_id}
    if before_ci_bytes != current_ci_bytes:
        return {'verified': False, 'detail': 'ci_workflow_modified', 'pr_number': pr_number, 'ci_run_id': run_id}
    if CI_TEST_COMMAND_IDENTITY.encode('utf-8') not in current_ci_bytes:
        return {'verified': False, 'detail': 'ci_workflow_untrusted', 'pr_number': pr_number, 'ci_run_id': run_id}
    expected_ci_workflow_sha256 = hashlib.sha256(current_ci_bytes).hexdigest()
    before_contract_blobs = {REQUIRED_CI_WORKFLOW: before_ci_bytes}
    for rel_path in CI_TEST_CONTRACT_PATHS:
        if rel_path == REQUIRED_CI_WORKFLOW:
            continue
        before_blob = read_remote_blob_bytes(api, before, rel_path)
        current_blob = read_remote_blob_bytes(api, sha, rel_path)
        if before_blob is None or current_blob is None:
            return {'verified': False, 'detail': 'ci_test_contract_untrusted', 'pr_number': pr_number, 'ci_run_id': run_id}
        if before_blob != current_blob:
            return {'verified': False, 'detail': 'ci_test_contract_modified', 'pr_number': pr_number, 'ci_run_id': run_id}
        before_contract_blobs[rel_path] = before_blob
    expected_ci_test_contract_sha256 = compute_ci_test_contract_sha256(before_contract_blobs)
    artifacts_payload = api.request('GET', f'/actions/runs/{run_id}/artifacts?per_page=100')
    if not isinstance(artifacts_payload, dict) or not isinstance(artifacts_payload.get('artifacts'), list):
        return {'verified': False, 'detail': 'ci_context_artifacts_unavailable', 'pr_number': pr_number, 'ci_run_id': run_id}
    expected_artifact_name = f'{CI_CONTEXT_ARTIFACT_PREFIX}{run_id}-{run_attempt}'
    matching_artifacts = [
        item for item in artifacts_payload['artifacts']
        if isinstance(item, dict) and item.get('name') == expected_artifact_name
    ]
    if len(matching_artifacts) != 1:
        return {'verified': False, 'detail': 'ci_context_artifact_missing', 'pr_number': pr_number, 'ci_run_id': run_id}
    artifact = matching_artifacts[0]
    artifact_id = artifact.get('id')
    if artifact.get('expired') is not False or type(artifact_id) is not int or artifact_id <= 0:
        return {'verified': False, 'detail': 'ci_context_artifact_invalid', 'pr_number': pr_number, 'ci_run_id': run_id}
    artifact_run = artifact.get('workflow_run')
    if not isinstance(artifact_run, dict) or artifact_run.get('id') != run_id:
        return {'verified': False, 'detail': 'ci_context_artifact_run_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if type(run_repo_id) is int and artifact_run.get('repository_id') != run_repo_id:
        return {'verified': False, 'detail': 'ci_context_artifact_repo_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if artifact_run.get('head_sha') != head_sha or artifact_run.get('head_branch') != pr_head_ref:
        return {'verified': False, 'detail': 'ci_context_artifact_head_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    download_url = artifact.get('archive_download_url')
    expected_download_url = f'https://api.github.com/repos/{expected_repo}/actions/artifacts/{artifact_id}/zip'
    if download_url is not None and download_url != expected_download_url:
        return {'verified': False, 'detail': 'ci_context_artifact_repo_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    raw_zip = api.download_artifact(artifact_id)
    if raw_zip is None:
        return {'verified': False, 'detail': 'ci_context_artifact_missing', 'pr_number': pr_number, 'ci_run_id': run_id}
    expected_digest = artifact.get('digest')
    if expected_digest is not None:
        actual_digest = 'sha256:' + hashlib.sha256(raw_zip).hexdigest()
        if expected_digest != actual_digest:
            return {'verified': False, 'detail': 'ci_context_artifact_digest_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    receipt = _parse_ci_context_zip(raw_zip)
    if receipt is None:
        return {'verified': False, 'detail': 'ci_context_invalid', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['repository'] != expected_repo:
        return {'verified': False, 'detail': 'ci_context_repository_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if type(receipt['run_id']) is not int or receipt['run_id'] != run_id:
        return {'verified': False, 'detail': 'ci_context_run_id_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if type(receipt['run_attempt']) is not int or receipt['run_attempt'] != run_attempt:
        return {'verified': False, 'detail': 'ci_context_run_attempt_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if type(receipt['pull_request_number']) is not int or receipt['pull_request_number'] != pr_number:
        return {'verified': False, 'detail': 'ci_context_pr_number_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['base_ref'] != target_branch:
        return {'verified': False, 'detail': 'ci_context_base_ref_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if (not isinstance(receipt['base_sha'], str) or not SHA_RE.fullmatch(receipt['base_sha'])
            or receipt['base_sha'] != pr_base_sha or receipt['base_sha'] != before):
        return {'verified': False, 'detail': 'ci_context_base_sha_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['head_repository'] != pr_head_repo:
        return {'verified': False, 'detail': 'ci_context_head_repository_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['head_ref'] != pr_head_ref:
        return {'verified': False, 'detail': 'ci_context_head_ref_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['head_sha'] != head_sha:
        return {'verified': False, 'detail': 'ci_context_head_sha_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if not isinstance(receipt['checkout_sha'], str) or not SHA_RE.fullmatch(receipt['checkout_sha']):
        return {'verified': False, 'detail': 'ci_context_checkout_invalid', 'pr_number': pr_number, 'ci_run_id': run_id}
    if not isinstance(receipt['checkout_tree_sha'], str) or not SHA_RE.fullmatch(receipt['checkout_tree_sha']):
        return {'verified': False, 'detail': 'ci_context_checkout_invalid', 'pr_number': pr_number, 'ci_run_id': run_id}
    parents = receipt['checkout_parent_shas']
    if (not isinstance(parents, list) or len(parents) < 2
            or any(not isinstance(parent, str) or not SHA_RE.fullmatch(parent) for parent in parents)
            or parents[0] != receipt['base_sha'] or parents[1] != receipt['head_sha']):
        return {'verified': False, 'detail': 'ci_context_parents_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if (not isinstance(receipt['ci_workflow_sha256'], str)
            or not SHA256_RE.fullmatch(receipt['ci_workflow_sha256'])
            or receipt['ci_workflow_sha256'] != expected_ci_workflow_sha256):
        return {'verified': False, 'detail': 'ci_workflow_hash_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if (not isinstance(receipt['ci_test_contract_sha256'], str)
            or not SHA256_RE.fullmatch(receipt['ci_test_contract_sha256'])
            or receipt['ci_test_contract_sha256'] != expected_ci_test_contract_sha256):
        return {'verified': False, 'detail': 'ci_test_contract_hash_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['test_command_identity'] != CI_TEST_COMMAND_IDENTITY:
        return {'verified': False, 'detail': 'ci_test_command_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    ctest_error = _validate_ci_ctest_evidence(
        receipt['ctest_inventory'], receipt['ctest_results'], receipt['ctest_junit_sha256']
    )
    if ctest_error:
        return {'verified': False, 'detail': ctest_error, 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['job_name'] != REQUIRED_CI_JOB:
        return {'verified': False, 'detail': 'ci_context_job_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['test_result'] != 'success':
        return {'verified': False, 'detail': 'ci_context_result_not_success', 'pr_number': pr_number, 'ci_run_id': run_id}
    main_commit = api.request('GET', f'/git/commits/{sha}')
    main_tree = main_commit.get('tree') if isinstance(main_commit, dict) else None
    main_tree_sha = main_tree.get('sha') if isinstance(main_tree, dict) else None
    if (not isinstance(main_commit, dict) or main_commit.get('sha') != sha
            or not isinstance(main_tree_sha, str) or not SHA_RE.fullmatch(main_tree_sha)):
        return {'verified': False, 'detail': 'main_commit_tree_unavailable', 'pr_number': pr_number, 'ci_run_id': run_id}
    if receipt['checkout_tree_sha'] != main_tree_sha:
        return {'verified': False, 'detail': 'ci_tree_sha_mismatch', 'pr_number': pr_number, 'ci_run_id': run_id}
    return {
        'verified': True,
        'pr_number': pr_number,
        'ci_run_id': run_id,
        'ci_run_attempt': run_attempt,
        'tree_sha': main_tree_sha,
        'ci_test_contract_sha256': expected_ci_test_contract_sha256,
    }

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
            raise ValueError('version_tag_conflict: Tag belongs to another source commit')
        if not release:
            raise ValueError('version_tag_conflict: Tag exists without a release; review it before publishing')
    if release and (marker(config) not in (release.get('body') or '')
                    or release.get('target_commitish') != config['sha']
                    or release.get('tag_name') != config['tag']):
        raise ValueError('Draft source is missing or differs; refusing to modify it')
    return release

def plan_release(api, config, verify=False, event_name='workflow_dispatch', before=None, target_branch='main'):
    if verify:
        return {'run_release': True, 'run_full_tests': True, 'publish': False, 'reason': 'verify_requested'}
    import release_evidence as evidence
    if event_name not in ('push', 'workflow_dispatch'):
        raise ValueError(f'Unsupported release plan event: {event_name!r}')
    if event_name == 'push' and before and not SHA_RE.fullmatch(before):
        raise ValueError(f'before_commit_unavailable: invalid before commit SHA {before!r}')
    current_channel = channel_policy_from_config(config)
    release = existing(api, config)
    if release and not release['draft']:
        if event_name == 'push':
            before_channel = read_before_channel(api, before)
            before_version = before_channel['version']
            if before_version != config['version']:
                raise ValueError(
                    f'version_tag_conflict: version changed from {before_version} to {config["version"]}, '
                    f'but published release {config["tag"]} already exists'
                )
            compare_release_channel_policy(before_channel, current_channel, config['tag'])
            trust = verify_merged_pr_ci(api, config['sha'], target_branch=target_branch, before=before)
            if not trust['verified']:
                result = {
                    'run_release': True,
                    'run_full_tests': True,
                    'publish': False,
                    'reason': 'pr_ci_unverified_release_required',
                    'before_version': before_version,
                    'ci_trust_detail': trust['detail'],
                }
                if trust.get('pr_number') is not None:
                    result['pr_number'] = trust['pr_number']
                if trust.get('ci_run_id') is not None:
                    result['ci_run_id'] = trust['ci_run_id']
                return result
            try:
                unchanged = evidence.remote_fingerprint(api, before) == evidence.remote_fingerprint(api, config['sha'])
            except (ValueError, OSError, KeyError) as error:
                return {'run_release': True, 'run_full_tests': True, 'publish': False,
                        'reason': 'test_inputs_unverified_release_required', 'detail': str(error)}
            if not unchanged:
                return {'run_release': True, 'run_full_tests': True, 'publish': False,
                        'reason': 'product_changes_require_full_release'}
            return {
                'run_release': False,
                'run_full_tests': False,
                'publish': False,
                'reason': 'existing_published_tag_no_release_needed',
                'before_version': before_version,
                'pr_number': trust['pr_number'],
                'ci_run_id': trust['ci_run_id'],
                'ci_run_attempt': trust['ci_run_attempt'],
                'tree_sha': trust['tree_sha'],
                'ci_test_contract_sha256': trust['ci_test_contract_sha256'],
            }
        return {
            'run_release': False,
            'run_full_tests': False,
            'publish': False,
            'reason': 'existing_published_tag_no_release_needed',
        }
    if release and release['draft']:
        return {'run_release': True, 'run_full_tests': True, 'publish': True, 'reason': 'draft_release_resume_required'}
    # Until the new binaries and actual runner are available, Full is mandatory.
    # A pure-version main push may subsequently prove a narrow Core reuse scope.
    result = {'run_release': True, 'run_full_tests': True, 'publish': True, 'reason': 'new_release_required'}
    if event_name == 'push':
        before_channel = read_before_channel(api, before)
        pulls = api.request('GET', f'/commits/{config["sha"]}/pulls?per_page=100')
        merged = [pr for pr in pulls if isinstance(pr, dict) and pr.get('merged_at')
                  and pr.get('merge_commit_sha') == config['sha'] and pr.get('base', {}).get('ref') == 'main'] if isinstance(pulls, list) else []
        if target_branch != 'main' or len(merged) != 1:
            result.update(publish=False, reason='unverified_main_push_full_release')
        elif before_channel['version'] != config['version']:
            result['reuse_source_commit'] = before
    return result

def plan(api, config, verify, event_name='workflow_dispatch', before=None, target_branch='main'):
    return plan_release(
        api, config, verify=verify, event_name=event_name, before=before, target_branch=target_branch
    )['publish']

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
    parser.add_argument('--event-name', default=os.environ.get('GITHUB_EVENT_NAME', ''))
    parser.add_argument('--before', default='')
    parser.add_argument('--branch', default=os.environ.get('GITHUB_REF_NAME') or 'main')
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
    if args.mode == 'plan':
        api = None if args.verify else GitHub(os.environ['GITHUB_REPOSITORY'], os.environ['GH_TOKEN'])
        decision = plan_release(
            api, config, verify=args.verify, event_name=args.event_name,
            before=args.before, target_branch=args.branch,
        )
        outputs = {
            'tag': config['tag'],
            'build_linux': config['build_linux'],
            'run_release': decision['run_release'],
            'run_full_tests': decision['run_full_tests'],
            'reuse_source_commit': decision.get('reuse_source_commit', ''),
            'publish': decision['publish'],
            'reason': decision['reason'],
        }
        summary = (f"Release plan: tag={outputs['tag']} run_release={str(outputs['run_release']).lower()} "
                   f"run_full_tests={str(outputs['run_full_tests']).lower()} publish={str(outputs['publish']).lower()} reason={outputs['reason']}")
        if 'before_version' in decision:
            summary += f" before_version={decision['before_version']}"
        if 'pr_number' in decision and decision['pr_number'] is not None:
            summary += f" pr=#{decision['pr_number']}"
        if 'ci_run_id' in decision and decision['ci_run_id'] is not None:
            summary += f" ci_run={decision['ci_run_id']}"
        if 'ci_run_attempt' in decision and decision['ci_run_attempt'] is not None:
            summary += f" ci_attempt={decision['ci_run_attempt']}"
        if 'tree_sha' in decision:
            summary += f" tree={decision['tree_sha']}"
        if 'ci_test_contract_sha256' in decision:
            summary += f" contract={decision['ci_test_contract_sha256'][:12]}"
        if 'ci_trust_detail' in decision:
            summary += f" ci_trust_detail={decision['ci_trust_detail']}"
        print(summary)
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as stream:
            for key, value in outputs.items():
                print(f'{key}={str(value).lower() if isinstance(value, bool) else value}', file=stream)
    else:
        api = GitHub(os.environ['GITHUB_REPOSITORY'], os.environ['GH_TOKEN'])
        publish(api, config, args.folder)

if __name__ == '__main__':
    main()
