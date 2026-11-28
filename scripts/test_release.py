"""Release transactions against an in-memory GitHub API; never contacts GitHub."""
import base64, copy, hashlib, io, json, os, pathlib, re, shutil, subprocess, sys, tarfile, tempfile, unittest, urllib.error, urllib.parse, zipfile
from unittest import mock
import release as r
import release_ci as ci
import release_evidence as ev
import release_artifacts as artifacts


class LinuxReleaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.bash = shutil.which('bash')
        if os.name == 'nt':
            git = shutil.which('git')
            cls.bash = str(pathlib.Path(git).parent.parent / 'bin/bash.exe') if git else None
        if not cls.bash or not pathlib.Path(cls.bash).is_file():
            raise unittest.SkipTest('Bash is required for Linux package gate self-tests')

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='shanjie-linux-gate-')
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name) / 'source tree'
        scripts = self.root / 'linux/scripts'
        scripts.mkdir(parents=True)
        for name in ['package.sh', 'test.sh']:
            shutil.copy2(r.ROOT / 'linux/scripts' / name, scripts / name)
            (scripts / name).chmod(0o755)
        helpers = self.root / 'scripts'
        helpers.mkdir()
        for name in ['release.py', 'release_linux.py']:
            shutil.copy2(r.ROOT / 'scripts' / name, helpers / name)
        ibus = self.root / 'linux/ibus'
        ibus.mkdir()
        (ibus / 'Cargo.toml').write_text('[package]\nname = "fixture"\nversion = "0.0.1"\n', encoding='utf-8')
        package = self.root / 'linux/package'
        package.mkdir()
        (package / 'README.txt').write_text('synthetic package fixture', encoding='utf-8')
        (scripts / 'build.sh').write_text('''#!/usr/bin/env bash
set -euo pipefail
echo build >> "$SHANJIE_GATE_LOG"
mkdir -p "$1/files/libexec" "$1/files/share/shanjie/data"
printf 'synthetic engine' > "$1/files/libexec/ibus-engine-shanjie"
printf '0.0.1\n' > "$1/VERSION"
''', encoding='utf-8', newline='\n')
        (scripts / 'build.sh').chmod(0o755)
        fakebin = self.root / 'fakebin'
        fakebin.mkdir()
        (fakebin / 'cmake').write_text('''#!/usr/bin/env bash
set -euo pipefail
echo configure >> "$SHANJIE_GATE_LOG"
exit "${SHANJIE_FAKE_CONFIGURE_EXIT:-0}"
''', encoding='utf-8', newline='\n')
        (fakebin / 'ctest').write_text('''#!/usr/bin/env bash
set -euo pipefail
if [[ " $* " = *" --show-only=json-v1 "* ]]; then
  printf '{"tests":[{"name":"synthetic.linux"}]}\n'
  exit 0
fi
echo ctest >> "$SHANJIE_GATE_LOG"
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test "$SHANJIE_TEST_PROFILE" = 1
case "$XDG_DATA_HOME" in "$fixture"/build/reports/*/profile/data) ;; *) exit 91 ;; esac
while (( $# )); do
  if [[ "$1" = --output-junit ]]; then
    shift
    printf '<testsuite><testcase name="synthetic.linux" time="0.1"/></testsuite>\n' > "$1"
  elif [[ "$1" = --output-log ]]; then
    shift
    printf 'synthetic CTest log\n' > "$1"
  fi
  shift
done
exit "${SHANJIE_FAKE_CTEST_EXIT:-0}"
''', encoding='utf-8', newline='\n')
        (fakebin / 'python3').write_text('''#!/usr/bin/env bash
exec "$SHANJIE_PYTHON" "$@"
''', encoding='utf-8', newline='\n')
        for file in fakebin.iterdir():
            file.chmod(0o755)
        self.events = self.root / 'gate.log'
        self.environment = dict(os.environ, SHANJIE_GATE_LOG=str(self.events), SHANJIE_PYTHON=sys.executable,
                                GITHUB_SHA='a'*40, GITHUB_RUN_ID='synthetic-linux-run',
                                PATH=str(fakebin) + os.pathsep + os.environ['PATH'])

    def run_package(self, *arguments, **environment):
        # Only these temporary fixture scripts run; no Cargo, IBus or user data.
        return subprocess.run([self.bash, 'linux/scripts/package.sh', *map(str, arguments)], cwd=self.root,
                              env=self.environment | environment, capture_output=True, timeout=30)

    def test_tests_finish_before_an_archive_is_created(self):
        result = self.run_package()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
        self.assertEqual(self.events.read_text().splitlines(), ['build', 'configure', 'ctest'])
        archives = list((self.root / 'build/linux/dist').glob('*.tar.gz'))
        self.assertEqual(len(archives), 1)
        with tarfile.open(archives[0]) as archive:
            self.assertTrue(any(name.endswith('/README.txt') for name in archive.getnames()))
        self.assertIn(r.filehash(archives[0]), archives[0].with_name(archives[0].name + '.sha256').read_text())

    def test_a_failed_ctest_blocks_packaging_and_keeps_reports(self):
        result = self.run_package(SHANJIE_FAKE_CTEST_EXIT='8')
        self.assertEqual(result.returncode, 8)
        self.assertEqual(self.events.read_text().splitlines(), ['build', 'configure', 'ctest'])
        self.assertEqual(list((self.root / 'build/linux/dist').glob('*.tar.gz')), [])
        self.assertEqual(len(list((self.root / 'build/reports').rglob('junit.xml'))), 1)

    def test_a_failed_configuration_stops_before_tests_and_packaging(self):
        result = self.run_package(SHANJIE_FAKE_CONFIGURE_EXIT='1')
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.events.read_text().splitlines(), ['build', 'configure'])
        self.assertEqual(list((self.root / 'build/linux/dist').glob('*.tar.gz')), [])

    def test_packaging_a_transferred_payload_does_not_repeat_build_or_tests(self):
        first = self.run_package()
        self.assertEqual(first.returncode, 0, first.stderr.decode(errors='replace'))
        payload = next((self.root / 'build/linux/dist').glob('shanjie-linux-*/VERSION')).parent
        receipt = self.root / 'build/linux/tested.json'
        report = json.loads(receipt.read_text(encoding='utf-8'))['report']
        transfer = self.root / 'tested.tar'
        artifacts.main(['pack', str(transfer), '--root', str(self.root), '--paths',
                        payload.relative_to(self.root).as_posix(), 'build/linux/tested.json',
                        report + '/inventory.json', report + '/junit.xml', report + '/ctest.log'])
        consumer = pathlib.Path(self.temporary.name) / 'package job'
        for name in ['linux/scripts/package.sh', 'linux/ibus/Cargo.toml', 'scripts/release.py', 'scripts/release_linux.py']:
            target = consumer / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.root / name, target)
        artifacts.main(['unpack', str(transfer), '--root', str(consumer)])
        result = subprocess.run([self.bash, 'linux/scripts/package.sh', '--tested',
                                 str(consumer / payload.relative_to(self.root)), str(consumer / 'build/linux/tested.json')],
                                cwd=consumer, env=self.environment, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
        self.assertEqual(self.events.read_text().splitlines(), ['build', 'configure', 'ctest'])
        self.assertEqual(len(list((consumer / 'build/linux/dist').glob('*.tar.gz'))), 1)

    def test_changed_payload_or_another_run_cannot_reuse_the_receipt(self):
        first = self.run_package()
        self.assertEqual(first.returncode, 0, first.stderr.decode(errors='replace'))
        payload = next((self.root / 'build/linux/dist').glob('shanjie-linux-*/VERSION')).parent
        receipt = self.root / 'build/linux/tested.json'
        archive = next((self.root / 'build/linux/dist').glob('*.tar.gz'))
        accepted_hash = r.filehash(archive)
        result = self.run_package('--tested', payload, receipt, GITHUB_RUN_ID='another-run')
        self.assertNotEqual(result.returncode, 0)
        (payload / 'files/libexec/ibus-engine-shanjie').write_bytes(b'changed after CTest')
        result = self.run_package('--tested', payload, receipt)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(r.filehash(archive), accepted_hash)

    def test_skipped_linux_junit_cannot_authorize_packaging(self):
        first = self.run_package()
        self.assertEqual(first.returncode, 0, first.stderr.decode(errors='replace'))
        payload = next((self.root / 'build/linux/dist').glob('shanjie-linux-*/VERSION')).parent
        receipt = self.root / 'build/linux/tested.json'
        record = json.loads(receipt.read_text(encoding='utf-8'))
        junit = self.root / record['report'] / 'junit.xml'
        junit.write_text('<testsuite><testcase name="synthetic.linux"><skipped/></testcase></testsuite>', encoding='utf-8')
        record['reports']['junit.xml'] = r.filehash(junit)
        receipt.write_text(json.dumps(record), encoding='utf-8')
        result = self.run_package('--tested', payload, receipt)
        self.assertNotEqual(result.returncode, 0)

    def test_ctest_inventory_contains_all_unit_suites_and_ibus_scenarios(self):
        cmake, ctest = shutil.which('cmake'), shutil.which('ctest')
        if not cmake or not ctest:
            self.skipTest('CMake and CTest are required for inventory validation')
        binary = self.root / 'inventory'
        payload = self.root / 'payload'
        payload.mkdir()
        result = subprocess.run([cmake, '-S', str(r.ROOT / 'linux/tests'), '-B', str(binary),
                                 '-DSHANJIE_LINUX_PAYLOAD=' + str(payload)], capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
        result = subprocess.run([ctest, '--test-dir', str(binary), '-C', 'Release', '--show-only=json-v1'],
                                check=True, capture_output=True, timeout=30)
        inventory = json.loads(result.stdout)
        units = {'linux.rust.' + name for name in ['shared', 'common', 'ibus', 'settings']}
        scenarios = {'linux.ibus.' + file.stem for file in (r.ROOT / 'linux/tests/scenarios').glob('*.json')}
        names = [test['name'] for test in inventory['tests']]
        self.assertEqual(set(names), units | scenarios)
        self.assertEqual(len(names), len(units | scenarios))
        for test in inventory['tests']:
            if test['name'] in units:
                self.assertIn('--locked', test['command'])
                self.assertIn('--test-threads=1', test['command'])
                self.assertNotIn('--ignored', test['command'])
            else:
                self.assertEqual(pathlib.Path(test['command'][-2]).parent.parent, payload / 'files')


class ReleaseArtifactTests(unittest.TestCase):
    def test_round_trip_preserves_unicode_spaces_and_binary_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source, output = root / 'source tree', root / 'restored tree'
            paths = ['dist/setup/安裝善解.exe', 'dist/ui/語系 file.dll',
                     'dist/data/models/builtin/model', 'native/x64/shanjie_windows.lib',
                     'build/parallel/build-x64.json']
            expected = {}
            for index, name in enumerate(paths):
                file = source / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(bytes(range(256)) * (index + 1))
                expected[name] = r.filehash(file)
            archive = root / 'release inputs.tar'
            artifacts.main(['pack', str(archive), '--root', str(source), '--paths',
                            'dist', 'native', 'build/parallel/build-x64.json'])
            artifacts.main(['unpack', str(archive), '--root', str(output)])
            actual = {file.relative_to(output).as_posix(): r.filehash(file)
                      for file in output.rglob('*') if file.is_file()}
            self.assertEqual(actual, expected)

    def test_missing_build_input_fails_the_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            with self.assertRaises(FileNotFoundError):
                artifacts.main(['pack', str(root / 'inputs.tar'), '--root', str(root),
                                '--paths', 'missing.dll'])

    def test_invalid_archive_cannot_be_extracted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            archive = root / 'inputs.tar'
            archive.write_bytes(b'not a release archive')
            with self.assertRaises(tarfile.ReadError):
                artifacts.main(['unpack', str(archive), '--root', str(root / 'output')])

    def test_archive_cannot_write_outside_the_artifact_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            archive = root / 'inputs.tar'
            with tarfile.open(archive, 'w') as bundle:
                entry = tarfile.TarInfo('../outside.txt')
                entry.size = 7
                bundle.addfile(entry, io.BytesIO(b'outside'))
            with self.assertRaises(tarfile.OutsideDestinationError):
                artifacts.main(['unpack', str(archive), '--root', str(root / 'output')])
            self.assertFalse((root / 'outside.txt').exists())

DEFAULT_CI_WORKFLOW_TEXT = (
    'name: ci\n'
    'on:\n'
    '  pull_request:\n'
    '    branches: [main]\n'
    'jobs:\n'
    '  unit-tests:\n'
    '    runs-on: windows-2025-vs2026\n'
    '    steps:\n'
    '      - run: pwsh -File scripts/test.ps1 -Level CI -CiSafe\n'
)
DEFAULT_CI_CONTRACT_FILES = {
    path: (DEFAULT_CI_WORKFLOW_TEXT if path == r.REQUIRED_CI_WORKFLOW else f'# synthetic contract {path}\n')
    for path in r.CI_TEST_CONTRACT_PATHS
}
DEFAULT_CI_CTEST_RESULTS = [
    {'name': name, 'preset': 'windows-x64', 'status': 'passed', 'duration_seconds': round(1.0 + index, 3)}
    for index, name in enumerate(r.REQUIRED_CI_CTEST_NAMES)
]
DEFAULT_CI_CTEST_JUNIT_SHA256 = {'junit-windows-x64.xml': 'a' * 64}

def make_ci_context_zip(receipt, filename=r.CI_CONTEXT_FILENAME):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        payload = receipt if isinstance(receipt, (bytes, str)) else json.dumps(receipt, indent=2) + '\n'
        archive.writestr(filename, payload)
    return buffer.getvalue()

def write_synthetic_ci_report(root, checkout_sha, test_names=r.REQUIRED_CI_CTEST_NAMES,
                              status='passed', junit_cases=None, run_dir_name='ci-20261128-210000-12345678',
                              level='CI', ci_safe=True, plan_only=False):
    report_dir = pathlib.Path(root) / 'build/reports' / run_dir_name
    report_dir.mkdir(parents=True, exist_ok=True)
    junit_path = report_dir / 'junit-windows-x64.xml'
    cases_for_xml = list(test_names) if junit_cases is None else list(junit_cases)
    xml_lines = ['<testsuite name="windows-x64">']
    for index, item in enumerate(cases_for_xml):
        if isinstance(item, tuple):
            name, extra = item
        else:
            name, extra = item, ''
        xml_lines.append(f'  <testcase name="{name}" classname="{name}" time="{1.0 + index:.3f}" status="run">{extra}</testcase>')
    xml_lines.append('</testsuite>\n')
    junit_path.write_text('\n'.join(xml_lines), encoding='utf-8')
    selected = [{'name': name, 'preset': 'windows-x64', 'labels': ['ci'], 'reason': 'Fixed CI baseline'} for name in test_names]
    results = [
        {'name': name, 'class': name, 'preset': 'windows-x64', 'status': status,
         'duration_seconds': round(1.0 + index, 3), 'failure': None, 'reason': None}
        for index, name in enumerate(test_names)
    ]
    report = {
        'schema': 3,
        'version': '0.16.1-beta.1',
        'commit': checkout_sha,
        'worktree_dirty': False,
        'level': level,
        'requested_level': level,
        'architecture': 'x64',
        'plan_only': plan_only,
        'status': status,
        'ci_safe': ci_safe,
        'selection': {'selected': selected, 'not_selected': []},
        'results': results,
        'ctest': [{'preset': 'windows-x64', 'wave': '', 'attempt': 1, 'junit': str(junit_path), 'diagnostic': False}],
    }
    report_file = report_dir / 'report.json'
    report_file.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    return report_file, junit_path

class FakeGitHub:
    def __init__(self):
        self.repository = 'tavricccc/shanjie-multiplatform'
        self.repository_id = 1418506561
        self.release = None
        self.extra = []
        self.assets = []
        self.ref = None
        self.files = {}
        self.commit_pulls = {}
        self.ci_runs = {}
        self.run_jobs = {}
        self.run_artifacts = {}
        self.artifact_zips = {}
        self.git_commits = {}
        self.fail_get = None
        self.fail_download = None
        self.writes = []
        self.uploads = []
        self.fail_upload = None
        self.fail_publish = False
        self.release_runs = []
    def set_commit_files(self, sha, version, prerelease=None, recommended_stable='0.16.0',
                         build_linux=False, extra_cargo='', extra_channel=None, channel_override=None,
                         ci_workflow=DEFAULT_CI_WORKFLOW_TEXT, contract_files=None):
        if prerelease is None:
            prerelease = '-beta.' in version
        channel_data = {
            'version': version,
            'prerelease': prerelease,
            'recommended_stable': recommended_stable,
            'build_linux': build_linux,
        }
        if extra_channel:
            channel_data.update(extra_channel)
        if channel_override is not None:
            channel_data = copy.deepcopy(channel_override)
        channel = json.dumps(channel_data, indent=2) + '\n'
        cargo = (f'[package]\nname = "shanjie-windows"\nversion = "{version}"\nedition = "2021"\n\n'
                 f'[features]\ninput-diagnostics = []\n{extra_cargo}')
        lock = f'version = 4\n\n[[package]]\nname = "shanjie-windows"\nversion = "{version}"\n'
        csproj = f'<Project Sdk="Microsoft.NET.Sdk">\n  <PropertyGroup>\n    <Version>{version}</Version>\n  </PropertyGroup>\n</Project>\n'
        self.files[sha] = {
            'RELEASE-CHANNEL.json': channel,
            'windows/Cargo.toml': cargo,
            'windows/Cargo.lock': lock,
            'ui/Shanjie.UI.csproj': csproj,
            **DEFAULT_CI_CONTRACT_FILES,
        }
        self.files[sha][r.REQUIRED_CI_WORKFLOW] = ci_workflow
        if contract_files:
            self.files[sha].update(contract_files)
    def set_merged_pr_ci(self, sha, head_sha, pr_number=138, base_ref='main',
                         base_sha='c' * 40, head_ref='feat/sync-upstream-reading-refills',
                         head_repository='HolyBearTW/shanjie-multiplatform',
                         merged_at='2026-11-27T15:37:17Z', merge_commit_sha=None,
                         run_id=46025531322, run_attempt=1, run_status='completed', run_conclusion='success',
                         run_event='pull_request', run_path='.github/workflows/ci.yml',
                         created_at='2026-11-27T15:30:00Z', jobs=None,
                         checkout_sha='f' * 40, tree_sha='2' * 40, main_tree_sha=None,
                         checkout_parent_shas=None, ci_workflow=DEFAULT_CI_WORKFLOW_TEXT,
                         contract_files=None, ctest_inventory=None, ctest_results=None,
                         ctest_junit_sha256=None,
                         artifact_id=15211522986, receipt_overrides=None,
                         artifact_overrides=None, include_artifact=True):
        head_files = self.files.setdefault(sha, {})
        for rel_path in ev.VERSION_FILES:
            if rel_path in self.files.get(base_sha, {}):
                head_files.setdefault(rel_path, self.files[base_sha][rel_path])
        for rel_path, default_content in DEFAULT_CI_CONTRACT_FILES.items():
            head_files.setdefault(rel_path, default_content)
        head_files[r.REQUIRED_CI_WORKFLOW] = ci_workflow
        if contract_files:
            head_files.update(contract_files)
        if base_sha in self.files:
            for rel_path, default_content in DEFAULT_CI_CONTRACT_FILES.items():
                self.files[base_sha].setdefault(rel_path, ci_workflow if rel_path == r.REQUIRED_CI_WORKFLOW else default_content)
        self.commit_pulls[sha] = [{
            'number': pr_number,
            'state': 'closed',
            'merged_at': merged_at,
            'merge_commit_sha': sha if merge_commit_sha is None else merge_commit_sha,
            'base': {'ref': base_ref, 'sha': base_sha},
            'head': {'ref': head_ref, 'sha': head_sha, 'repo': {'full_name': head_repository}},
        }]
        self.ci_runs[head_sha] = [{
            'id': run_id,
            'run_attempt': run_attempt,
            'name': 'ci',
            'path': run_path,
            'event': run_event,
            'status': run_status,
            'conclusion': run_conclusion,
            'head_sha': head_sha,
            'head_branch': head_ref,
            'repository': {'id': self.repository_id, 'full_name': self.repository},
            'head_repository': {'id': 1445984360, 'full_name': head_repository},
            'created_at': created_at,
            'pull_requests': [],
        }]
        self.run_jobs[run_id] = (
            [{'id': 140595826542, 'name': 'unit-tests', 'run_attempt': run_attempt,
              'status': 'completed', 'conclusion': 'success'}]
            if jobs is None else copy.deepcopy(jobs)
        )
        self.git_commits[sha] = {
            'sha': sha,
            'tree': {'sha': tree_sha if main_tree_sha is None else main_tree_sha},
            'parents': [{'sha': base_sha}, {'sha': head_sha}],
        }
        if include_artifact:
            wf_bytes = ci_workflow.encode('utf-8') if isinstance(ci_workflow, str) else ci_workflow
            contract_blobs = {
                rel_path: (head_files[rel_path].encode('utf-8') if isinstance(head_files[rel_path], str) else head_files[rel_path])
                for rel_path in r.CI_TEST_CONTRACT_PATHS
                if rel_path in head_files and isinstance(head_files[rel_path], (str, bytes, bytearray))
            }
            contract_sha256 = (
                r.compute_ci_test_contract_sha256(contract_blobs)
                if len(contract_blobs) == len(r.CI_TEST_CONTRACT_PATHS)
                else '0' * 64
            )
            receipt = {
                'repository': self.repository,
                'pull_request_number': pr_number,
                'base_ref': base_ref,
                'base_sha': base_sha,
                'head_repository': head_repository,
                'head_ref': head_ref,
                'head_sha': head_sha,
                'checkout_sha': checkout_sha,
                'checkout_tree_sha': tree_sha,
                'checkout_parent_shas': [base_sha, head_sha] if checkout_parent_shas is None else list(checkout_parent_shas),
                'ci_workflow_sha256': hashlib.sha256(wf_bytes).hexdigest(),
                'ci_test_contract_sha256': contract_sha256,
                'test_command_identity': r.CI_TEST_COMMAND_IDENTITY,
                'ctest_inventory': list(r.REQUIRED_CI_CTEST_NAMES) if ctest_inventory is None else copy.deepcopy(ctest_inventory),
                'ctest_results': copy.deepcopy(DEFAULT_CI_CTEST_RESULTS) if ctest_results is None else copy.deepcopy(ctest_results),
                'ctest_junit_sha256': copy.deepcopy(DEFAULT_CI_CTEST_JUNIT_SHA256) if ctest_junit_sha256 is None else copy.deepcopy(ctest_junit_sha256),
                'run_id': run_id,
                'run_attempt': run_attempt,
                'job_name': r.REQUIRED_CI_JOB,
                'test_result': 'success',
            }
            if receipt_overrides:
                receipt.update(receipt_overrides)
            raw_zip = make_ci_context_zip(receipt)
            artifact = {
                'id': artifact_id,
                'name': f'{r.CI_CONTEXT_ARTIFACT_PREFIX}{run_id}-{run_attempt}',
                'expired': False,
                'digest': 'sha256:' + hashlib.sha256(raw_zip).hexdigest(),
                'size_in_bytes': len(raw_zip),
                'archive_download_url': f'https://api.github.com/repos/{self.repository}/actions/artifacts/{artifact_id}/zip',
                'workflow_run': {
                    'id': run_id,
                    'repository_id': self.repository_id,
                    'head_repository_id': 1445984360,
                    'head_branch': head_ref,
                    'head_sha': head_sha,
                },
            }
            if artifact_overrides:
                artifact.update(artifact_overrides)
            self.run_artifacts[run_id] = [artifact]
            self.artifact_zips[artifact_id] = raw_zip
        else:
            self.run_artifacts[run_id] = []
    def download_artifact(self, artifact_id, max_bytes=r.CI_CONTEXT_MAX_ZIP_BYTES):
        if self.fail_download and self.fail_download(artifact_id):
            raise self.fail_download(artifact_id)
        return self.artifact_zips.get(artifact_id)
    def request(self, method, path, body=None):
        if method == 'GET':
            if self.fail_get and self.fail_get(path):
                raise self.fail_get(path)
            if path.startswith('/git/trees/'):
                commit = path.split('/git/trees/', 1)[1].split('?', 1)[0]
                files = self.files.get(commit)
                if files is None:
                    return None
                return {'truncated': False, 'tree': [
                    {'path': p, 'mode': '100644', 'type': 'blob',
                     'sha': hashlib.sha1(data.encode() if isinstance(data, str) else data).hexdigest()}
                    for p, data in sorted(files.items())
                ]}
            # GitHub's tag lookup returns only published releases; drafts have no tag yet.
            if '/releases/tags/' in path:
                return copy.deepcopy(self.release) if self.release and not self.release['draft'] else None
            if path.startswith('/releases?'):
                page = int(path.split('page=')[-1]) if '&page=' in path else 1
                items = [copy.deepcopy(r) for r in ([self.release] if self.release else []) + self.extra]
                return items if page == 1 else []
            if '/git/ref/tags/' in path: return self.ref
            if '/assets?' in path: return copy.deepcopy(self.assets)
            if path.startswith('/commits/') and '/pulls' in path:
                parsed = urllib.parse.urlsplit(path)
                sha = parsed.path.removeprefix('/commits/').removesuffix('/pulls')
                pulls = self.commit_pulls.get(sha, [])
                return copy.deepcopy(pulls)
            if path.startswith('/actions/workflows/ci.yml/runs'):
                parsed = urllib.parse.urlsplit(path)
                params = urllib.parse.parse_qs(parsed.query)
                head_sha = params.get('head_sha', [''])[0]
                event = params.get('event', [''])[0]
                runs = self.ci_runs.get(head_sha)
                if runs is None:
                    return {'workflow_runs': []}
                if isinstance(runs, dict):
                    return copy.deepcopy(runs)
                filtered = [run for run in runs if not event or run.get('event') == event]
                return {'workflow_runs': copy.deepcopy(filtered)}
            if path.startswith('/actions/workflows/release.yml/runs'):
                return {'workflow_runs': copy.deepcopy(self.release_runs)}
            if path.startswith('/actions/runs/') and '/jobs' in path:
                parsed = urllib.parse.urlsplit(path)
                run_id = int(parsed.path.removeprefix('/actions/runs/').split('/')[0])
                jobs = self.run_jobs.get(run_id)
                if jobs is None:
                    return None
                if isinstance(jobs, dict):
                    return copy.deepcopy(jobs)
                return {'jobs': copy.deepcopy(jobs)}
            if path.startswith('/actions/runs/') and '/artifacts' in path:
                parsed = urllib.parse.urlsplit(path)
                run_id = int(parsed.path.removeprefix('/actions/runs/').removesuffix('/artifacts'))
                items = self.run_artifacts.get(run_id)
                if items is None:
                    return {'total_count': 0, 'artifacts': []}
                if isinstance(items, dict):
                    return copy.deepcopy(items)
                return {'total_count': len(items), 'artifacts': copy.deepcopy(items)}
            if path.startswith('/git/commits/'):
                sha = path.removeprefix('/git/commits/').split('?', 1)[0]
                commit = self.git_commits.get(sha)
                return copy.deepcopy(commit) if commit is not None else None
            if path.startswith('/contents/'):
                parsed = urllib.parse.urlsplit(path)
                rel = urllib.parse.unquote(parsed.path.removeprefix('/contents/'))
                ref = urllib.parse.parse_qs(parsed.query).get('ref', [''])[0]
                raw = self.files.get(ref, {}).get(rel)
                if raw is None:
                    return None
                if isinstance(raw, dict):
                    return copy.deepcopy(raw)
                raw_bytes = raw.encode('utf-8') if isinstance(raw, str) else raw
                encoded = base64.encodebytes(raw_bytes).decode('ascii')
                return {'type': 'file', 'encoding': 'base64', 'content': encoded}
            raise AssertionError(path)
        self.writes.append((method, path, body))
        if method == 'POST':
            if self.release: raise AssertionError('second draft created')
            self.release = dict(body, id=1)
        elif method == 'PATCH':
            if self.fail_publish: raise OSError('publication failed')
            self.release.update(body)
        else: raise AssertionError(method)
        return copy.deepcopy(self.release)
    def upload(self, release, path):
        if self.fail_upload == path.name: raise OSError('upload failed')
        self.uploads.append(path.name)
        asset = dict(name=path.name, size=path.stat().st_size, digest='sha256:' + r.filehash(path), state='uploaded')
        self.assets.append(asset)
        return copy.deepcopy(asset)

class ReleaseTests(unittest.TestCase):

    def test_windows_release_cargo_diagnostics_are_opt_in(self):
        parent = (r.ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        windows = (r.ROOT / '.github/workflows/release-windows.yml').read_text(encoding='utf-8')
        self.assertRegex(parent, r'(?m)^      cargo_diagnostics:\n        description: [^\n]+\n        type: boolean\n        default: false$')
        self.assertRegex(windows, r'(?m)^      cargo_diagnostics:\n        description: [^\n]+\n        type: boolean\n        default: false$')
        call = parent.split('\n  windows:\n', 1)[1].split('\n  z-linux:\n', 1)[0]
        forwarded = re.search(r"(?m)^      cargo_diagnostics: \$\{\{ github\.event_name == '([^']+)' && inputs\.cargo_diagnostics \}\}$", call)
        self.assertIsNotNone(forwarded)
        build_sections = re.split(r'^  ([a-zA-Z][\w-]*):\n', windows.split('\njobs:\n', 1)[1], flags=re.M)
        jobs = dict(zip(build_sections[1::2], build_sections[2::2]))
        for arch in ('x64', 'x86'):
            job = jobs['build-' + arch]
            build = job.split('      - name: Build ', 1)[1].split('\n      - ', 1)[0]
            gate = r"\$\{\{ inputs\.cargo_diagnostics && '([^']+)' \|\| '' \}\}"
            log = re.search(fr'(?m)^          CARGO_LOG: {gate}$', build)
            option = re.search(fr'(?m)^        run: pwsh -NoProfile -File scripts/release-pipeline\.ps1 -Phase Build -Architecture {arch} {gate}$', build)
            self.assertIsNotNone(log)
            self.assertIsNotNone(option)
            self.assertNotIn('cargo_diagnostics', job.split('      - name: Build ', 1)[0])
            for event, manual, expected in (
                ('push', True, ('', '')),
                ('workflow_dispatch', False, ('', '')),
                ('workflow_dispatch', True, ('cargo::compiler::fingerprint=trace', '-CargoVerbose')),
            ):
                with self.subTest(arch=arch, event=event, diagnostics=manual):
                    enabled = event == forwarded[1] and manual
                    self.assertEqual((log[1], option[1]) if enabled else ('', ''), expected)

    def setUp(self):
        self.folder = pathlib.Path(tempfile.mkdtemp(prefix='shanjie-release-test-'))
        # Keep scratch recoverable, including failed tests; no permanent deletion.
        self.addCleanup(self.archive)
        self.config = dict(sha='a'*40, version='0.16.0-beta.2', tag='v0.16.0-beta.2',
                           prerelease=True, recommended_stable='0.16.0',
                           build_linux=False, notes='Public release notes')
        self.api = FakeGitHub()
        self.package = self.folder / 'windows' / 'Shanjie-Windows-Setup.exe'
        self.package.parent.mkdir()
        self.package.write_bytes(b'public installer fixture')
        self.package.with_suffix('.exe.sha256').write_text(f'{r.filehash(self.package)}  {self.package.name}\n', encoding='utf-8')
        r.manifest(self.config, 'windows', self.package.parent)
    def archive(self):
        # Scratch lives in the system temp folder; failed runs print its path for inspection.
        if any(error for _, error in getattr(self._outcome, 'errors', []) if error):
            print('Kept failed release test scratch:', self.folder)
        else:
            def make_writable(func, path, _exc):
                os.chmod(path, 0o700)
                func(path)
            shutil.rmtree(self.folder, onexc=make_writable)
    def test_verify_does_not_contact_or_mutate_github(self):
        self.api.request = lambda *args: self.fail('verify mode contacted GitHub')
        self.assertFalse(r.plan(self.api, self.config, True))
        decision = r.plan_release(self.api, self.config, verify=True, event_name='workflow_dispatch')
        self.assertEqual(decision, {'run_release': True, 'run_full_tests': True, 'publish': False, 'reason': 'verify_requested'})
    def _write_repo_version_tree(self, root, version, prerelease=None, recommended_stable='0.16.0',
                                 build_linux=False, extra_cargo='', extra_channel=None, channel_override=None):
        if prerelease is None:
            prerelease = '-beta.' in version
        (root / 'windows').mkdir(parents=True, exist_ok=True)
        (root / 'ui').mkdir(parents=True, exist_ok=True)
        (root / 'docs/releases').mkdir(parents=True, exist_ok=True)
        channel_data = {
            'version': version, 'prerelease': prerelease,
            'recommended_stable': recommended_stable, 'build_linux': build_linux,
        }
        if extra_channel:
            channel_data.update(extra_channel)
        if channel_override is not None:
            channel_data = copy.deepcopy(channel_override)
        (root / 'RELEASE-CHANNEL.json').write_text(json.dumps(channel_data, indent=2) + '\n', encoding='utf-8')
        (root / 'windows/Cargo.toml').write_text(
            f'[package]\nname = "shanjie-windows"\nversion = "{version}"\nedition = "2021"\n\n'
            f'[features]\ninput-diagnostics = []\n{extra_cargo}', encoding='utf-8')
        (root / 'windows/Cargo.lock').write_text(
            f'version = 4\n\n[[package]]\nname = "shanjie-windows"\nversion = "{version}"\n', encoding='utf-8')
        (root / 'ui/Shanjie.UI.csproj').write_text(
            f'<Project Sdk="Microsoft.NET.Sdk">\n  <PropertyGroup>\n    <Version>{version}</Version>\n  </PropertyGroup>\n</Project>\n',
            encoding='utf-8')
        (root / f'docs/releases/RELEASE-NOTES-{version}.md').write_text(f'# {version}\n', encoding='utf-8')
    def _run_plan_cli(self, repo_root, sha, argv, api=None):
        output_file = self.folder / f'github-output-{len(list(self.folder.glob("github-output-*")))}.txt'
        env = {
            'GITHUB_SHA': sha,
            'GITHUB_REF_NAME': 'main',  # Simulated branch, independent of the runner's checkout.
            'GITHUB_REPOSITORY': 'tavricccc/shanjie-multiplatform',
            'GH_TOKEN': 'synthetic-token',
            'GITHUB_OUTPUT': str(output_file),
        }
        with mock.patch.dict(os.environ, env, clear=False), \
             mock.patch.object(sys, 'argv', ['release.py', 'plan', '--root', str(repo_root), *argv]), \
             mock.patch.object(r, 'GitHub', return_value=api if api is not None else self.api) as gh_cls:
            r.main()
        parsed = dict(line.split('=', 1) for line in output_file.read_text(encoding='utf-8').splitlines() if '=' in line)
        return parsed, gh_cls
    def _eval_parent_workflow(self, run_release, publish_requested, linux_enabled, windows_result=None, linux_result=None):
        parent = (r.ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        sections = re.split(r'^  ([A-Za-z][A-Za-z0-9-]*):\n', parent.split('\njobs:\n', 1)[1], flags=re.M)
        callers = dict(zip(sections[1::2], sections[2::2]))
        def eval_expr(raw, values):
            expr = ' '.join(raw.removeprefix('${{').removesuffix('}}').split())
            expr = expr.replace('always()', 'True')
            expr = expr.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
            for key, token in values:
                expr = expr.replace(key, token)
            return bool(eval(expr, {'__builtins__': {}}, {}))
        win_if = re.search(r'^    if: (.+)$', callers['windows'], re.M)[1].strip()
        linux_if = re.search(r'^    if: (.+)$', callers['z-linux'], re.M)[1].strip()
        pub_if = callers['publish'].split('    if: >-\n', 1)[1].split('\n    runs-on:', 1)[0].strip()
        run_str = "'true'" if run_release else "'false'"
        linux_str = "'true'" if linux_enabled else "'false'"
        win_runs = eval_expr(win_if, [("needs.release.outputs.run_release", run_str)])
        linux_runs = eval_expr(linux_if, [
            ("needs.release.outputs.run_release", run_str),
            ("needs.release.outputs.build_linux", linux_str),
        ])
        actual_win_result = windows_result if windows_result is not None else ('success' if win_runs else 'skipped')
        actual_linux_result = linux_result if linux_result is not None else ('success' if linux_runs else 'skipped')
        # Skipped reusable workflows emit empty strings for all outputs in GitHub Actions.
        win_out_run = ("'true'" if run_release else "'false'") if actual_win_result == 'success' else "''"
        win_out_pub = ("'true'" if publish_requested else "'false'") if actual_win_result == 'success' else "''"
        win_out_linux = ("'true'" if linux_enabled else "'false'") if actual_win_result == 'success' else "''"
        pub_runs = eval_expr(pub_if, [
            ("needs.windows.outputs.run_release", win_out_run),
            ("needs.windows.outputs.publish", win_out_pub),
            ("needs.windows.outputs.build_linux", win_out_linux),
            ("needs.windows.result", repr(actual_win_result)),
            ("needs.z-linux.result", repr(actual_linux_result)),
        ])
        return {'windows': win_runs, 'z-linux': linux_runs, 'publish': pub_runs}
    def test_check_assets_does_not_construct_a_github_client(self):
        with mock.patch.object(sys, 'argv', ['release.py', 'check', '--folder', str(self.folder)]), \
             mock.patch.object(r, 'configuration', return_value=self.config), \
             mock.patch.object(r, 'GitHub', side_effect=AssertionError('Asset readiness must not contact GitHub')):
            r.main()
        self.assertEqual(self.api.writes, [])
    def test_check_assets_requires_enabled_linux_and_matching_source(self):
        self.config['build_linux'] = True
        with self.assertRaises(ValueError):
            r.assets_for(self.config, self.folder)
        linux = self.folder / 'linux'
        linux.mkdir()
        package = linux / 'shanjie-linux-fixture.tar.gz'
        package.write_bytes(b'synthetic Linux package')
        checksum = package.with_name(package.name + '.sha256')
        checksum.write_text(f'{r.filehash(package)}  {package.name}\n', encoding='utf-8')
        r.manifest(self.config, 'linux', linux)
        self.assertEqual(len(r.assets_for(self.config, self.folder)), 4)
        source = linux / 'release-source.json'
        record = json.loads(source.read_text(encoding='utf-8'))
        record['sha'] = 'b'*40
        source.write_text(json.dumps(record), encoding='utf-8')
        with self.assertRaises(ValueError):
            r.assets_for(self.config, self.folder)
    def test_check_assets_rejects_changed_packages_and_unexpected_linux(self):
        self.package.write_bytes(b'changed after packaging')
        with self.assertRaises(ValueError):
            r.assets_for(self.config, self.folder)
        self.package.write_bytes(b'public installer fixture')
        linux = self.folder / 'linux'
        linux.mkdir()
        package = linux / 'shanjie-linux-fixture.tar.gz'
        package.write_bytes(b'synthetic Linux package')
        checksum = package.with_name(package.name + '.sha256')
        checksum.write_text(f'{r.filehash(package)}  {package.name}\n', encoding='utf-8')
        r.manifest(self.config, 'linux', linux)
        with self.assertRaises(ValueError):
            r.assets_for(self.config, self.folder)
    def test_publication_output_requires_successful_required_platform_jobs(self):
        parent = (r.ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        sections = re.split(r'^  ([A-Za-z][A-Za-z0-9-]*):\n', parent.split('\njobs:\n', 1)[1], flags=re.M)
        callers = dict(zip(sections[1::2], sections[2::2]))
        self.assertIn('run_release: ${{ steps.release.outputs.run_release }}', callers['release'])
        self.assertIn('reason: ${{ steps.release.outputs.reason }}', callers['release'])
        self.assertIn('github.event.before', callers['release'])
        self.assertIn('pull-requests: read', callers['release'])
        self.assertIn('actions: read', callers['release'])
        for name, file in [('windows', 'release-windows.yml'), ('z-linux', 'release-linux.yml')]:
            self.assertIn('needs: release', callers[name])
            self.assertIn('uses: ./.github/workflows/' + file, callers[name])
        self.assertIn("if: needs.release.outputs.run_release == 'true'", callers['windows'])
        self.assertIn("run_release: ${{ needs.release.outputs.run_release == 'true' }}", callers['windows'])
        self.assertIn("if: needs.release.outputs.run_release == 'true' && needs.release.outputs.build_linux == 'true'", callers['z-linux'])
        publish = callers['publish']
        self.assertIn('needs: [windows, z-linux]', publish)
        self.assertIn('needs.windows.outputs.artifact_name', publish)
        self.assertIn('needs.z-linux.outputs.artifact_name', publish)
        raw = publish.split('    if: >-\n', 1)[1].split('\n    runs-on:', 1)[0].strip()
        expression = ' '.join(raw.removeprefix('${{').removesuffix('}}').split())
        expression = expression.replace('always()', 'True')
        expression = expression.replace("needs.windows.outputs.run_release == 'true'", 'run_release_requested')
        expression = expression.replace("needs.windows.outputs.publish == 'true'", 'publish_requested')
        expression = expression.replace("needs.windows.outputs.build_linux == 'true'", 'linux_enabled')
        expression = expression.replace("needs.windows.outputs.build_linux == 'false'", '(not linux_enabled)')
        expression = expression.replace('needs.windows.result', 'windows_result').replace('needs.z-linux.result', 'linux_result')
        expression = expression.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
        for run_release_requested in [False, True]:
            for publish_requested in [False, True]:
                for linux_enabled in [False, True]:
                    for windows_result in ['success', 'failure', 'cancelled', 'skipped']:
                        for linux_result in ['success', 'failure', 'cancelled', 'skipped']:
                            expected = (run_release_requested and publish_requested and windows_result == 'success'
                                        and linux_result == ('success' if linux_enabled else 'skipped'))
                            with self.subTest(run_release=run_release_requested, publish=publish_requested,
                                              linux=linux_enabled, windows=windows_result, result=linux_result):
                                self.assertEqual(eval(expression, {'__builtins__': {}}, {
                                    'run_release_requested': run_release_requested,
                                    'publish_requested': publish_requested, 'linux_enabled': linux_enabled,
                                    'windows_result': windows_result, 'linux_result': linux_result}), expected)
        windows = (r.ROOT / '.github/workflows/release-windows.yml').read_text(encoding='utf-8').split('\njobs:\n', 1)[0]
        linux = (r.ROOT / '.github/workflows/release-linux.yml').read_text(encoding='utf-8').split('\njobs:\n', 1)[0]
        self.assertIn('jobs.package.outputs.artifact_name', windows)
        self.assertIn('jobs.package.outputs.run_release', windows)
        self.assertIn('jobs.package.outputs.publish', windows)
        self.assertIn('jobs.package.outputs.build_linux', windows)
        self.assertIn('jobs.z-linux-package.outputs.artifact_name', linux)
    def test_pr138_same_version_cargo_feature_push_requires_full_release(self):
        repo = self.folder / 'pr138'
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=138, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1')
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=138)
        self.api.files[head_sha]['windows/Cargo.toml'] += '# new Cargo feature contract\nrerank-timing = []\n'
        self.api.files[head_sha]['windows/src/engine.rs'] = '// new product behavior\n'
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(outputs, {
            'tag': 'v0.16.1-beta.1',
            'build_linux': 'false',
            'run_release': 'true',
            'run_full_tests': 'true',
            'reuse_source_commit': '',
            'publish': 'false',
            'reason': 'product_changes_require_full_release',
        })
        for linux_enabled in [False, True]:
            flow = self._eval_parent_workflow(
                run_release=(outputs['run_release'] == 'true'),
                publish_requested=(outputs['publish'] == 'true'),
                linux_enabled=linux_enabled,
            )
            self.assertEqual(flow, {'windows': True, 'z-linux': linux_enabled, 'publish': False})
    def test_release_channel_prerelease_change_on_published_tag_fails_closed(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=140)
        for label, version, before_pre, head_pre in [
            ('beta_to_non_prerelease', '0.16.1-beta.1', True, False),
            ('stable_to_prerelease', '0.16.0', False, True),
        ]:
            with self.subTest(case=label):
                repo = self.folder / f'channel-prerelease-{label}'
                self._write_repo_version_tree(repo, version, prerelease=head_pre)
                self.api.release = dict(draft=False, id=140, tag_name='v' + version, body='published')
                self.api.set_commit_files(before_sha, version, prerelease=before_pre)
                with self.assertRaisesRegex(ValueError, 'release_channel_policy_conflict'):
                    self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        # Legacy before commit with mismatched prerelease flag must also fail closed, never skip.
        legacy_repo = self.folder / 'channel-prerelease-legacy'
        self._write_repo_version_tree(legacy_repo, '0.15.3', prerelease=False, recommended_stable='0.15.3')
        self.api.release = dict(draft=False, id=141, tag_name='v0.15.3', body='published')
        self.api.set_commit_files(before_sha, '0.15.3', prerelease=True, recommended_stable='0.15.1')
        with self.assertRaisesRegex(ValueError, 'release_channel_policy_conflict'):
            self._run_plan_cli(legacy_repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        # Direct compare check: prerelease difference cannot be ignored even if suffix validation were bypassed.
        with self.assertRaisesRegex(ValueError, 'release_channel_policy_conflict'):
            r.compare_release_channel_policy(
                {'version': '0.16.1-beta.1', 'prerelease': True, 'recommended_stable': '0.16.0', 'build_linux': False},
                {'version': '0.16.1-beta.1', 'prerelease': False, 'recommended_stable': '0.16.0', 'build_linux': False},
                'v0.16.1-beta.1',
            )
        self.assertEqual(self.api.writes, [])
    def test_release_channel_recommended_stable_change_on_published_tag_fails_closed(self):
        repo = self.folder / 'channel-recommended-stable'
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self._write_repo_version_tree(repo, '0.16.1-beta.1', recommended_stable='0.16.1')
        self.api.release = dict(draft=False, id=142, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1', recommended_stable='0.16.0')
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=142)
        with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*recommended_stable.*bump to a new version'):
            self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(self.api.writes, [])
    def test_release_channel_build_linux_change_on_published_tag_fails_closed(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=143)
        for label, before_linux, head_linux in [('enable_linux', False, True), ('disable_linux', True, False)]:
            with self.subTest(case=label):
                repo = self.folder / f'channel-build-linux-{label}'
                self._write_repo_version_tree(repo, '0.16.1-beta.1', build_linux=head_linux)
                self.api.release = dict(draft=False, id=143, tag_name='v0.16.1-beta.1', body='published beta.1')
                self.api.set_commit_files(before_sha, '0.16.1-beta.1', build_linux=before_linux)
                with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*build_linux.*bump to a new version'):
                    self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(self.api.writes, [])
    def test_release_channel_unknown_field_on_published_tag_fails_closed(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self.api.release = dict(draft=False, id=144, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=144)
        # Unknown field added in current commit.
        repo = self.folder / 'channel-unknown-current'
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_channel={'rollout_tier': 'canary'})
        self.api.set_commit_files(before_sha, '0.16.1-beta.1')
        with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*unknown.*rollout_tier'):
            self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        # Direct plan_release call with extra unknown key must also fail closed.
        config_with_unknown = dict(self.config, rollout_tier='canary')
        with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*unknown.*rollout_tier'):
            r.plan_release(self.api, config_with_unknown, verify=False, event_name='push', before=before_sha)
        # Unknown field present in before commit must also fail closed.
        repo_clean = self.folder / 'channel-unknown-before'
        self._write_repo_version_tree(repo_clean, '0.16.1-beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1', extra_channel={'legacy_flag': True})
        with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*unknown.*legacy_flag'):
            self._run_plan_cli(repo_clean, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(self.api.writes, [])
    def test_release_channel_other_non_version_setting_changes_fail_closed(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self.api.release = dict(draft=False, id=145, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=145)
        # Multiple non-version settings changed together.
        repo_multi = self.folder / 'channel-multi-change'
        self._write_repo_version_tree(repo_multi, '0.16.1-beta.1', recommended_stable='0.16.1', build_linux=True)
        self.api.set_commit_files(before_sha, '0.16.1-beta.1', recommended_stable='0.16.0', build_linux=False)
        with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*build_linux, recommended_stable'):
            self._run_plan_cli(repo_multi, head_sha, ['--event-name', 'push', '--before', before_sha])
        # Missing required non-version field or invalid value fails closed.
        repo_missing = self.folder / 'channel-missing-field'
        self._write_repo_version_tree(repo_missing, '0.16.1-beta.1', channel_override={
            'version': '0.16.1-beta.1', 'prerelease': True, 'build_linux': False,
        })
        with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*missing.*recommended_stable'):
            self._run_plan_cli(repo_missing, head_sha, ['--event-name', 'push', '--before', before_sha])
        # Generic full-object comparison: even if a future non-version policy key is allowlisted, changing it fails closed.
        extended_keys = r.RELEASE_CHANNEL_KEYS | {'update_channel_note'}
        with mock.patch.object(r, 'RELEASE_CHANNEL_KEYS', extended_keys):
            repo_ext = self.folder / 'channel-future-setting'
            self._write_repo_version_tree(repo_ext, '0.16.1-beta.1', extra_channel={'update_channel_note': 'hotfix'})
            self.api.set_commit_files(before_sha, '0.16.1-beta.1', extra_channel={'update_channel_note': 'default'})
            with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*update_channel_note'):
                self._run_plan_cli(repo_ext, head_sha, ['--event-name', 'push', '--before', before_sha])
            # Strict type comparison: False vs 0 must not be treated as equal.
            repo_type = self.folder / 'channel-future-type'
            self._write_repo_version_tree(repo_type, '0.16.1-beta.1', extra_channel={'update_channel_note': 0})
            self.api.set_commit_files(before_sha, '0.16.1-beta.1', extra_channel={'update_channel_note': False})
            with self.assertRaisesRegex(ValueError, r'release_channel_policy_conflict.*update_channel_note'):
                self._run_plan_cli(repo_type, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(self.api.writes, [])
    def test_main_push_without_merged_pr_cannot_skip_release(self):
        repo = self.folder / 'direct-push-main'
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=150, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1')
        cases = [
            ('direct_push_no_pr', []),
            ('unmerged_pr', [{'number': 150, 'merged_at': None, 'merge_commit_sha': head_sha,
                              'base': {'ref': 'main'}, 'head': {'sha': pr_head_sha}}]),
            ('wrong_base_branch', [{'number': 150, 'merged_at': '2026-11-27T15:37:17Z', 'merge_commit_sha': head_sha,
                                    'base': {'ref': 'develop'}, 'head': {'sha': pr_head_sha}}]),
            ('mismatched_merge_sha', [{'number': 150, 'merged_at': '2026-11-27T15:37:17Z', 'merge_commit_sha': '9' * 40,
                                       'base': {'ref': 'main'}, 'head': {'sha': pr_head_sha}}]),
            ('ambiguous_multiple_prs', [
                {'number': 150, 'merged_at': '2026-11-27T15:37:17Z', 'merge_commit_sha': head_sha,
                 'base': {'ref': 'main'}, 'head': {'sha': pr_head_sha}},
                {'number': 151, 'merged_at': '2026-11-27T15:38:17Z', 'merge_commit_sha': head_sha,
                 'base': {'ref': 'main'}, 'head': {'sha': '2' * 40}},
            ]),
            ('pulls_lookup_404', None),
        ]
        for label, pulls in cases:
            with self.subTest(case=label):
                self.api.commit_pulls[head_sha] = pulls
                outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
                self.assertEqual(outputs, {
                    'tag': 'v0.16.1-beta.1',
                    'build_linux': 'false',
                    'run_release': 'true',
                    'run_full_tests': 'true',
                    'reuse_source_commit': '',
                    'publish': 'false',
                    'reason': 'pr_ci_unverified_release_required',
                })
                self.assertEqual(
                    self._eval_parent_workflow(True, False, linux_enabled=False),
                    {'windows': True, 'z-linux': False, 'publish': False},
                )
                self.assertEqual(
                    self._eval_parent_workflow(True, False, linux_enabled=True),
                    {'windows': True, 'z-linux': True, 'publish': False},
                )
    def test_main_push_requires_successful_pr_ci_before_skipping_release(self):
        repo = self.folder / 'pr-ci-trust'
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        head_ref = 'feat/pr-ci-trust'
        head_repo = 'HolyBearTW/shanjie-multiplatform'
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=160, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1')
        self.api.files.setdefault(head_sha, {})[r.REQUIRED_CI_WORKFLOW] = DEFAULT_CI_WORKFLOW_TEXT
        def make_run(run_id, status='completed', conclusion='success', event='pull_request',
                     created_at='2026-11-27T15:30:00Z', run_attempt=1):
            return {
                'id': run_id, 'run_attempt': run_attempt, 'path': '.github/workflows/ci.yml',
                'event': event, 'head_sha': pr_head_sha, 'head_branch': head_ref,
                'repository': {'id': self.api.repository_id, 'full_name': self.api.repository},
                'head_repository': {'id': 1445984360, 'full_name': head_repo},
                'status': status, 'conclusion': conclusion, 'created_at': created_at,
            }
        unverified_cases = [
            ('ci_run_missing', [], {}),
            ('ci_run_in_progress', [make_run(10, status='in_progress', conclusion=None)], {}),
            ('ci_run_failed', [make_run(11, conclusion='failure')], {}),
            ('ci_run_cancelled', [make_run(12, conclusion='cancelled')], {}),
            ('latest_ci_run_failed_after_earlier_success', [
                make_run(13, conclusion='success', created_at='2026-11-27T15:20:00Z'),
                make_run(14, conclusion='failure', created_at='2026-11-27T15:35:00Z'),
            ], {13: [{'name': 'unit-tests', 'run_attempt': 1, 'status': 'completed', 'conclusion': 'success'}]}),
            ('only_workflow_dispatch_run', [make_run(15, event='workflow_dispatch')],
             {15: [{'name': 'unit-tests', 'run_attempt': 1, 'status': 'completed', 'conclusion': 'success'}]}),
            ('required_job_missing', [make_run(16)],
             {16: [{'name': 'other-job', 'run_attempt': 1, 'status': 'completed', 'conclusion': 'success'}]}),
            ('required_job_skipped', [make_run(17)],
             {17: [{'name': 'unit-tests', 'run_attempt': 1, 'status': 'completed', 'conclusion': 'skipped'}]}),
            ('jobs_payload_unavailable', [make_run(18)], {}),
        ]
        for label, runs, jobs_map in unverified_cases:
            with self.subTest(case=label):
                self.api.commit_pulls[head_sha] = [{
                    'number': 160, 'state': 'closed', 'merged_at': '2026-11-27T15:37:17Z',
                    'merge_commit_sha': head_sha, 'base': {'ref': 'main', 'sha': before_sha},
                    'head': {'ref': head_ref, 'sha': pr_head_sha, 'repo': {'full_name': head_repo}},
                }]
                self.api.ci_runs[pr_head_sha] = runs
                self.api.run_jobs = dict(jobs_map)
                outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
                self.assertEqual(outputs['run_release'], 'true')
                self.assertEqual(outputs['publish'], 'false')
                self.assertEqual(outputs['reason'], 'pr_ci_unverified_release_required')
        # Positive case: squash-merged PR where earlier CI failed and latest rerun passed allows skipping.
        self.api.set_merged_pr_ci(
            head_sha, pr_head_sha, pr_number=136, base_sha=before_sha,
            head_ref=head_ref, head_repository=head_repo, run_id=101,
            created_at='2026-11-24T17:15:00Z',
        )
        self.api.ci_runs[pr_head_sha].insert(0, make_run(100, conclusion='failure', created_at='2026-11-24T17:00:00Z'))
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(outputs['run_release'], 'false')
        self.assertEqual(outputs['publish'], 'false')
        self.assertEqual(outputs['reason'], 'existing_published_tag_no_release_needed')
    def test_pr_ci_lookup_api_failure_fails_closed(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=170, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=170, base_sha=before_sha, run_id=17001, artifact_id=17099)
        http_403 = urllib.error.HTTPError('https://api.github.com/repos/o/r', 403, 'Forbidden', {}, None)
        for label, target in [
            ('pulls_api_error', '/pulls'),
            ('ci_runs_api_error', '/actions/workflows/ci.yml/runs'),
            ('ci_jobs_api_error', '/actions/runs/17001/jobs'),
            ('ci_workflow_api_error', '/contents/.github/workflows/ci.yml'),
            ('ci_contract_api_error', '/contents/scripts/test.ps1'),
            ('ci_artifacts_api_error', '/actions/runs/17001/artifacts'),
            ('main_commit_api_error', f'/git/commits/{head_sha}'),
        ]:
            with self.subTest(case=label):
                self.api.fail_get = lambda path, needle=target: http_403 if needle in path else None
                with self.assertRaises(urllib.error.HTTPError):
                    r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.api.fail_get = None
        self.api.fail_download = lambda artifact_id: http_403 if artifact_id == 17099 else None
        with self.assertRaises(urllib.error.HTTPError):
            r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.api.fail_download = None
    def test_ci_workflow_records_and_uploads_ci_context_receipt(self):
        workflow = (r.ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        jobs_section = workflow.split('\njobs:\n', 1)[1]
        job_ids = re.findall(r'^  ([A-Za-z][A-Za-z0-9-]*):\n', jobs_section, flags=re.M)
        self.assertEqual(job_ids, ['unit-tests'])
        self.assertIn('Write CI context receipt', workflow)
        self.assertIn('Upload CI context receipt', workflow)
        self.assertIn("if: success() && github.event_name == 'pull_request'", workflow)
        self.assertIn('python scripts/release.py ci-context --output build/ci-context/ci-context.json', workflow)
        self.assertIn('name: ci-context-${{ github.run_id }}-${{ github.run_attempt }}', workflow)
        self.assertIn('path: build/ci-context/ci-context.json', workflow)
        self.assertIn('retention-days: 14', workflow)
        self.assertLess(workflow.index('scripts/test.ps1 -Level CI -CiSafe'), workflow.index('Write CI context receipt'))
        self.assertLess(workflow.index('Write CI context receipt'), workflow.index('Upload CI context receipt'))
    def test_build_ci_context_in_shallow_merge_checkout_records_all_16_fields(self):
        git_env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_CONFIG')}
        git_env.update({
            'GIT_AUTHOR_NAME': 'Test', 'GIT_AUTHOR_EMAIL': 'test@example.com',
            'GIT_COMMITTER_NAME': 'Test', 'GIT_COMMITTER_EMAIL': 'test@example.com',
        })
        origin = self.folder / 'ci-origin'
        origin.mkdir()
        def git(repo_dir, *args):
            return subprocess.check_output(['git', '-C', str(repo_dir), *args], env=git_env, text=True).strip()
        git(origin, 'init', '-b', 'main')
        git(origin, 'config', 'core.autocrlf', 'false')
        for rel_path, content in DEFAULT_CI_CONTRACT_FILES.items():
            target = origin / rel_path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content.encode('utf-8'))
        (origin / 'app.txt').write_text('base\n', encoding='utf-8')
        git(origin, 'add', '.')
        git(origin, 'commit', '-m', 'base')
        base_sha = git(origin, 'rev-parse', 'HEAD')
        git(origin, 'checkout', '-b', 'pr-branch')
        (origin / 'app.txt').write_text('feature\n', encoding='utf-8')
        git(origin, 'add', 'app.txt')
        git(origin, 'commit', '-m', 'pr change')
        head_sha = git(origin, 'rev-parse', 'HEAD')
        git(origin, 'checkout', 'main')
        git(origin, 'merge', '--no-ff', 'pr-branch', '-m', 'synthetic pr merge')
        merge_sha = git(origin, 'rev-parse', 'HEAD')
        merge_tree_sha = git(origin, 'rev-parse', 'HEAD^{tree}')
        shallow = self.folder / 'ci-shallow'
        subprocess.check_call(
            ['git', 'clone', '-c', 'core.autocrlf=false', '--depth', '1', origin.as_uri(), str(shallow)],
            env=git_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        self.assertTrue((shallow / '.git/shallow').is_file())
        report_file, junit_file = write_synthetic_ci_report(shallow, merge_sha)
        output = shallow / 'build/ci-context/ci-context.json'
        env = {
            'CI_REPOSITORY': 'tavricccc/shanjie-multiplatform',
            'CI_PULL_REQUEST_NUMBER': '138',
            'CI_BASE_REF': 'main',
            'CI_BASE_SHA': base_sha,
            'CI_HEAD_REPOSITORY': 'HolyBearTW/shanjie-multiplatform',
            'CI_HEAD_REF': 'feat/sync-upstream-reading-refills',
            'CI_HEAD_SHA': head_sha,
            'CI_RUN_ID': '46025531322',
            'CI_RUN_ATTEMPT': '1',
            'CI_JOB_NAME': 'unit-tests',
        }
        with mock.patch.dict(os.environ, git_env | env, clear=True):
            with mock.patch.object(sys, 'argv', ['release.py', 'ci-context', '--root', str(shallow), '--output', str(output)]):
                r.main()
            receipt = json.loads(output.read_text(encoding='utf-8'))
            self.assertEqual(set(receipt), r.CI_CONTEXT_KEYS)
            expected_wf_sha = hashlib.sha256(DEFAULT_CI_WORKFLOW_TEXT.encode('utf-8')).hexdigest()
            expected_contract_sha = r.compute_ci_test_contract_sha256(
                {path: text.encode('utf-8') for path, text in DEFAULT_CI_CONTRACT_FILES.items()}
            )
            self.assertEqual(receipt, {
                'repository': 'tavricccc/shanjie-multiplatform',
                'pull_request_number': 138,
                'base_ref': 'main',
                'base_sha': base_sha,
                'head_repository': 'HolyBearTW/shanjie-multiplatform',
                'head_ref': 'feat/sync-upstream-reading-refills',
                'head_sha': head_sha,
                'checkout_sha': merge_sha,
                'checkout_tree_sha': merge_tree_sha,
                'checkout_parent_shas': [base_sha, head_sha],
                'ci_workflow_sha256': expected_wf_sha,
                'ci_test_contract_sha256': expected_contract_sha,
                'test_command_identity': r.CI_TEST_COMMAND_IDENTITY,
                'ctest_inventory': list(r.REQUIRED_CI_CTEST_NAMES),
                'ctest_results': DEFAULT_CI_CTEST_RESULTS,
                'ctest_junit_sha256': {'junit-windows-x64.xml': r.filehash(junit_file)},
                'run_id': 46025531322,
                'run_attempt': 1,
                'job_name': 'unit-tests',
                'test_result': 'success',
            })
            # CRLF working copy on Windows runners must still match the LF Git blob hash.
            (shallow / '.github/workflows/ci.yml').write_bytes(DEFAULT_CI_WORKFLOW_TEXT.replace('\n', '\r\n').encode('utf-8'))
            (shallow / 'scripts/test.ps1').write_bytes(DEFAULT_CI_CONTRACT_FILES['scripts/test.ps1'].replace('\n', '\r\n').encode('utf-8'))
            crlf_receipt = r.build_ci_context(shallow, env)
            self.assertEqual(crlf_receipt['ci_workflow_sha256'], expected_wf_sha)
            self.assertEqual(crlf_receipt['ci_test_contract_sha256'], expected_contract_sha)
            # Tampering with a test contract script before writing the receipt must be rejected.
            (shallow / 'scripts/test.ps1').write_text('# tampered test driver\n', encoding='utf-8')
            with self.assertRaisesRegex(ValueError, 'Working tree CI test contract file differs'):
                r.build_ci_context(shallow, env)
            (shallow / 'scripts/test.ps1').write_bytes(DEFAULT_CI_CONTRACT_FILES['scripts/test.ps1'].encode('utf-8'))
            # Missing CTest inventory or failed/skipped JUnit testcases must be rejected when generating a receipt.
            bad_report, _ = write_synthetic_ci_report(
                shallow, merge_sha, test_names=r.REQUIRED_CI_CTEST_NAMES[:-1],
                run_dir_name='ci-20261128-210100-missing1',
            )
            with self.assertRaisesRegex(ValueError, 'incomplete or unexpected'):
                r.build_ci_context(shallow, env, report_path=bad_report)
            skipped_report, _ = write_synthetic_ci_report(
                shallow, merge_sha,
                junit_cases=[(name, '<skipped/>' if index == 0 else '') for index, name in enumerate(r.REQUIRED_CI_CTEST_NAMES)],
                run_dir_name='ci-20261128-210200-skipped1',
            )
            with self.assertRaisesRegex(ValueError, 'failed or was skipped'):
                r.build_ci_context(shallow, env, report_path=skipped_report)
            # Tampering with the checked-out CI workflow before writing the receipt must be rejected.
            (shallow / '.github/workflows/ci.yml').write_text('name: tampered\n', encoding='utf-8')
            with self.assertRaisesRegex(ValueError, 'Working tree CI workflow differs'):
                r.build_ci_context(shallow, env)
    def test_github_download_artifact_strips_authorization_on_redirect(self):
        client = r.GitHub('tavricccc/shanjie-multiplatform', 'secret-token')
        redirect_url = 'https://pipelines.actions.githubusercontent.com/signed-zip'
        captured_requests = []
        class FakeResponse(io.BytesIO):
            def __enter__(self): return self
            def __exit__(self, *args): self.close()
        class FakeOpener:
            def open(self, req, timeout=60):
                captured_requests.append(('api', req.full_url, dict(req.header_items())))
                raise urllib.error.HTTPError(
                    req.full_url, 302, 'Found', {'Location': redirect_url}, io.BytesIO(b'')
                )
        def fake_urlopen(req, timeout=60):
            captured_requests.append(('blob', req.full_url, dict(req.header_items())))
            return FakeResponse(b'PK-zip-bytes')
        with mock.patch.object(r.urllib.request, 'build_opener', return_value=FakeOpener()), \
             mock.patch.object(r.urllib.request, 'urlopen', side_effect=fake_urlopen):
            data = client.download_artifact(15211522986)
        self.assertEqual(data, b'PK-zip-bytes')
        self.assertEqual(captured_requests[0][1], 'https://api.github.com/repos/tavricccc/shanjie-multiplatform/actions/artifacts/15211522986/zip')
        self.assertIn('Authorization', captured_requests[0][2])
        self.assertEqual(captured_requests[1][1], redirect_url)
        self.assertNotIn('Authorization', captured_requests[1][2])
        class MissingOpener:
            def open(self, req, timeout=60):
                raise urllib.error.HTTPError(req.full_url, 404, 'Not Found', {}, io.BytesIO(b''))
        with mock.patch.object(r.urllib.request, 'build_opener', return_value=MissingOpener()):
            self.assertIsNone(client.download_artifact(15211522986))
        class ForbiddenOpener:
            def open(self, req, timeout=60):
                raise urllib.error.HTTPError(req.full_url, 403, 'Forbidden', {}, io.BytesIO(b''))
        with mock.patch.object(r.urllib.request, 'build_opener', return_value=ForbiddenOpener()):
            with self.assertRaises(urllib.error.HTTPError):
                client.download_artifact(15211522986)
    def test_pr_ci_matching_merge_tree_cannot_replace_functional_release(self):
        repo = self.folder / 'pr138-exact-tree'
        before_sha = 'c1cc71f36489f035d08edf34ca51b5fc6fb95870'
        head_sha = 'ef1856d50bb4f0ac1c9a3f90004743b3b4394a2a'
        pr_head_sha = '1c56c4efb5ac8431555cc1153c4a1caf424e4c40'
        ci_checkout_sha = 'f8840f76d43f8e826dc62631b6255f7646635be8'
        shared_tree_sha = 'c2f4d6ce3f312faece6d5e54bbe3accf5af89452'
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=138, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1')
        self.api.set_merged_pr_ci(
            head_sha, pr_head_sha, pr_number=138, base_sha=before_sha,
            checkout_sha=ci_checkout_sha, tree_sha=shared_tree_sha, main_tree_sha=shared_tree_sha,
        )
        self.api.files[head_sha]['windows/Cargo.toml'] += '# new Cargo feature contract\nrerank-timing = []\n'
        self.api.files[head_sha]['windows/src/engine.rs'] = '// new product behavior\n'
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(outputs, {
            'tag': 'v0.16.1-beta.1',
            'build_linux': 'false',
            'run_release': 'true',
            'run_full_tests': 'true',
            'reuse_source_commit': '',
            'publish': 'false',
            'reason': 'product_changes_require_full_release',
        })
    def test_pr_head_same_but_main_advanced_after_ci_requires_full_release(self):
        repo = self.folder / 'main-advanced-after-ci'
        old_base_sha, advanced_before_sha = 'a' * 40, 'b' * 40
        head_sha, pr_head_sha = 'e' * 40, '1' * 40
        old_tree_sha, new_tree_sha = '3' * 40, '4' * 40
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=181, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(advanced_before_sha, '0.16.1-beta.1')
        for label, pr_base_sha in [('pr_base_updated', advanced_before_sha), ('pr_base_stale', old_base_sha)]:
            with self.subTest(case=label):
                self.api.set_merged_pr_ci(
                    head_sha, pr_head_sha, pr_number=181, base_sha=pr_base_sha,
                    tree_sha=old_tree_sha, main_tree_sha=new_tree_sha,
                    receipt_overrides={'base_sha': old_base_sha, 'checkout_parent_shas': [old_base_sha, pr_head_sha]},
                )
                outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', advanced_before_sha])
                self.assertEqual(outputs['run_release'], 'true')
                self.assertEqual(outputs['publish'], 'false')
                self.assertEqual(outputs['reason'], 'pr_ci_unverified_release_required')
    def test_pr_ci_artifact_missing_or_expired_requires_full_release(self):
        repo = self.folder / 'artifact-missing'
        before_sha, head_sha, pr_head_sha = 'c' * 40, 'e' * 40, '1' * 40
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=182, tag_name='v0.16.1-beta.1', body='published beta.1')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1')
        # Case A: no ci-context artifact uploaded.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=182, base_sha=before_sha, include_artifact=False)
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual((outputs['run_release'], outputs['publish'], outputs['reason']),
                         ('true', 'false', 'pr_ci_unverified_release_required'))
        # Case B: artifact expired.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=182, base_sha=before_sha,
                                  artifact_overrides={'expired': True})
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual((outputs['run_release'], outputs['publish'], outputs['reason']),
                         ('true', 'false', 'pr_ci_unverified_release_required'))
        # Case C: artifact download returns 404/None.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=182, base_sha=before_sha, artifact_id=18201)
        self.api.artifact_zips.pop(18201, None)
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual((outputs['run_release'], outputs['publish'], outputs['reason']),
                         ('true', 'false', 'pr_ci_unverified_release_required'))
    def test_pr_ci_artifact_run_id_mismatch_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=183, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        # Receipt run_id differs from workflow run ID.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=183, base_sha=before_sha,
                                  run_id=18301, receipt_overrides={'run_id': 99999})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_run_id_mismatch'))
        # Artifact metadata workflow_run.id differs.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=183, base_sha=before_sha, run_id=18301)
        self.api.run_artifacts[18301][0]['workflow_run']['id'] = 99999
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_artifact_run_mismatch'))
    def test_pr_ci_artifact_attempt_mismatch_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=184, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        # Receipt inside attempt 2 artifact claims attempt 1.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=184, base_sha=before_sha,
                                  run_id=18401, run_attempt=2, receipt_overrides={'run_attempt': 1})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_run_attempt_mismatch'))
        # Only attempt 1 artifact exists while latest run is attempt 2.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=184, base_sha=before_sha,
                                  run_id=18401, run_attempt=2, artifact_overrides={'name': 'ci-context-18401-1'})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_artifact_missing'))
    def test_pr_ci_artifact_pr_number_mismatch_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=185, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=185, base_sha=before_sha,
                                  receipt_overrides={'pull_request_number': 999})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_pr_number_mismatch'))
    def test_pr_ci_artifact_tree_sha_mismatch_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=186, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=186, base_sha=before_sha,
                                  tree_sha='5' * 40, main_tree_sha='6' * 40)
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_tree_sha_mismatch'))
        # Artifact zip digest mismatch must also refuse skipping.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=186, base_sha=before_sha,
                                  run_id=18602, tree_sha='5' * 40, main_tree_sha='5' * 40,
                                  artifact_overrides={'digest': 'sha256:' + '0' * 64})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_artifact_digest_mismatch'))
    def test_pr_ci_head_repository_mismatch_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=187, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        # Case A: workflow run head_repository differs from PR head repository.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=187, base_sha=before_sha, run_id=18701)
        self.api.ci_runs[pr_head_sha][0]['head_repository'] = {'id': 999, 'full_name': 'OtherUser/shanjie-multiplatform'}
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_head_repository_mismatch'))
        # Case B: receipt head_repository differs from PR head repository.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=187, base_sha=before_sha, run_id=18702,
                                  receipt_overrides={'head_repository': 'OtherUser/shanjie-multiplatform'})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_head_repository_mismatch'))
        # Case C: artifact repository_id differs from target repository.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=187, base_sha=before_sha, run_id=18703)
        self.api.run_artifacts[18703][0]['workflow_run']['repository_id'] = 999999
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_context_artifact_repo_mismatch'))
    def test_pr_ci_workflow_hash_mismatch_or_modified_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=188, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        # Case A: receipt ci_workflow_sha256 does not match trusted main ci.yml hash.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=188, base_sha=before_sha, run_id=18801,
                                  receipt_overrides={'ci_workflow_sha256': '0' * 64})
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_workflow_hash_mismatch'))
        # Case B: PR itself modified .github/workflows/ci.yml compared to before commit.
        modified_ci = DEFAULT_CI_WORKFLOW_TEXT + '# modified in PR\n'
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=188, base_sha=before_sha, run_id=18802,
                                  ci_workflow=modified_ci)
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_workflow_modified'))
    def test_pr_ci_latest_attempt_failed_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=189, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        # Case A: attempt 1 succeeded with an artifact, but latest attempt 2 failed.
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=189, base_sha=before_sha,
                                  run_id=18901, run_attempt=1, run_conclusion='success')
        self.api.ci_runs[pr_head_sha][0]['run_attempt'] = 2
        self.api.ci_runs[pr_head_sha][0]['conclusion'] = 'failure'
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                         (True, False, 'pr_ci_unverified_release_required', 'ci_run_conclusion:failure'))
        # Case B: run reports attempt 2 success, but latest unit-tests job for attempt 2 failed (or only attempt 1 job exists).
        for label, jobs in [
            ('attempt2_job_failed', [{'id': 2, 'name': 'unit-tests', 'run_attempt': 2, 'status': 'completed', 'conclusion': 'failure'}]),
            ('stale_attempt1_job_only', [{'id': 1, 'name': 'unit-tests', 'run_attempt': 1, 'status': 'completed', 'conclusion': 'success'}]),
        ]:
            with self.subTest(case=label):
                self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=189, base_sha=before_sha,
                                          run_id=18902, run_attempt=2, jobs=jobs)
                decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
                self.assertEqual((decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                                 (True, False, 'pr_ci_unverified_release_required', 'ci_required_job_not_succeeded'))
    def test_ci_test_contract_paths_and_ctest_names_match_repository(self):
        required_contract_files = {
            '.github/workflows/ci.yml',
            'CMakeLists.txt',
            'CMakePresets.json',
            'scripts/test.ps1',
            'scripts/test-plan.psm1',
            'scripts/test-support.psm1',
            'scripts/ctest-case.ps1',
            'scripts/release.py',
        }
        self.assertTrue(required_contract_files.issubset(set(r.CI_TEST_CONTRACT_PATHS)))
        self.assertEqual(list(r.CI_TEST_CONTRACT_PATHS), sorted(set(r.CI_TEST_CONTRACT_PATHS)))
        for rel_path in r.CI_TEST_CONTRACT_PATHS:
            self.assertTrue((r.ROOT / rel_path).is_file(), f'Missing contract file: {rel_path}')
        # The CI fixture executes these overlay generators before Cargo tests.
        prepare_core = (r.ROOT / 'scripts/prepare-core.ps1').read_text(encoding='utf-8')
        prepare_delegates = re.findall(r'& "\$PSScriptRoot/(prepare-[\w-]+\.ps1)"', prepare_core)
        self.assertTrue(prepare_delegates)
        self.assertTrue({f'scripts/{name}' for name in prepare_delegates}.issubset(r.CI_TEST_CONTRACT_PATHS))
        cmake = (r.ROOT / 'CMakeLists.txt').read_text(encoding='utf-8')
        cmake_ci_tests = sorted(re.findall(r'set_tests_properties\(([^\s\)]+) PROPERTIES LABELS "ci(?:;|")', cmake))
        self.assertEqual(tuple(cmake_ci_tests), r.REQUIRED_CI_CTEST_NAMES)
        plan_module = (r.ROOT / 'scripts/test-plan.psm1').read_text(encoding='utf-8')
        fn_block = plan_module.split('function Assert-FixedCiSelection', 1)[1].split('\n}', 1)[0]
        plan_ci_tests = sorted(re.findall(r"'([^']+)'", fn_block.split('\n', 2)[1]))
        self.assertEqual(tuple(plan_ci_tests), r.REQUIRED_CI_CTEST_NAMES)
    def test_pr_ci_contract_modification_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=190, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        for index, rel_path in enumerate(r.CI_TEST_CONTRACT_PATHS, start=1):
            if rel_path == r.REQUIRED_CI_WORKFLOW:
                continue  # The existing ci.yml mutation test covers its specific failure reason.
            with self.subTest(path=rel_path):
                self.api.files.pop(head_sha, None)
                modified = DEFAULT_CI_CONTRACT_FILES[rel_path] + '# altered in PR\n'
                self.api.set_merged_pr_ci(
                    head_sha, pr_head_sha, pr_number=190, base_sha=before_sha,
                    run_id=19000 + index, contract_files={rel_path: modified},
                )
                decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
                self.assertEqual(
                    (decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                    (True, False, 'pr_ci_unverified_release_required', 'ci_test_contract_modified'),
                )
        # Receipt claiming a different ci_test_contract_sha256 than the trusted baseline must be rejected.
        self.api.files.pop(head_sha, None)
        self.api.set_merged_pr_ci(
            head_sha, pr_head_sha, pr_number=190, base_sha=before_sha,
            run_id=19090, receipt_overrides={'ci_test_contract_sha256': '0' * 64},
        )
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual(
            (decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
            (True, False, 'pr_ci_unverified_release_required', 'ci_test_contract_hash_mismatch'),
        )
        # Missing contract file at before commit must fail conservatively without skipping Release.
        self.api.files.pop(head_sha, None)
        self.api.set_merged_pr_ci(head_sha, pr_head_sha, pr_number=190, base_sha=before_sha, run_id=19091)
        self.api.files[before_sha].pop('scripts/ctest-case.ps1', None)
        decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual(
            (decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
            (True, False, 'pr_ci_unverified_release_required', 'ci_test_contract_untrusted'),
        )
    def test_pr_ci_ctest_inventory_or_results_incomplete_requires_full_release(self):
        before_sha, head_sha, pr_head_sha = 'c' * 40, self.config['sha'], '1' * 40
        self.api.release = dict(draft=False, id=191, tag_name=self.config['tag'], body='published')
        self.api.set_commit_files(before_sha, self.config['version'], recommended_stable=self.config['recommended_stable'])
        bad_cases = [
            ('missing_inventory_item', {'ctest_inventory': list(r.REQUIRED_CI_CTEST_NAMES[:-1])}, 'ci_ctest_inventory_incomplete'),
            ('extra_inventory_item', {'ctest_inventory': list(r.REQUIRED_CI_CTEST_NAMES) + ['full.core-suite']}, 'ci_ctest_inventory_incomplete'),
            ('duplicate_inventory_item', {'ctest_inventory': list(r.REQUIRED_CI_CTEST_NAMES[:-1]) + [r.REQUIRED_CI_CTEST_NAMES[0]]}, 'ci_ctest_inventory_incomplete'),
            ('missing_result_item', {'ctest_results': copy.deepcopy(DEFAULT_CI_CTEST_RESULTS[:-1])}, 'ci_ctest_inventory_incomplete'),
            ('failed_result_item', {'ctest_results': [dict(item, status='failed') if item['name'] == 'fast.rust.windows' else dict(item) for item in DEFAULT_CI_CTEST_RESULTS]}, 'ci_ctest_results_invalid'),
            ('skipped_result_item', {'ctest_results': [dict(item, status='not-run') if item['name'] == 'fast.infrastructure' else dict(item) for item in DEFAULT_CI_CTEST_RESULTS]}, 'ci_ctest_results_invalid'),
            ('wrong_preset_result', {'ctest_results': [dict(item, preset='windows-x86') if item['name'] == 'fast.rust.shared' else dict(item) for item in DEFAULT_CI_CTEST_RESULTS]}, 'ci_ctest_results_invalid'),
            ('negative_duration_result', {'ctest_results': [dict(item, duration_seconds=-1.0) if item['name'] == 'fast.prepare-core' else dict(item) for item in DEFAULT_CI_CTEST_RESULTS]}, 'ci_ctest_results_invalid'),
            ('nan_duration_result', {'ctest_results': [dict(item, duration_seconds=float('nan')) if item['name'] == 'fast.prepare-core' else dict(item) for item in DEFAULT_CI_CTEST_RESULTS]}, 'ci_ctest_results_invalid'),
            ('empty_junit_hashes', {'ctest_junit_sha256': {}}, 'ci_ctest_results_invalid'),
            ('invalid_junit_hash', {'ctest_junit_sha256': {'junit-windows-x64.xml': 'not-a-sha256'}}, 'ci_ctest_results_invalid'),
        ]
        for index, (label, overrides, expected_detail) in enumerate(bad_cases, start=1):
            with self.subTest(case=label):
                self.api.files.pop(head_sha, None)
                self.api.set_merged_pr_ci(
                    head_sha, pr_head_sha, pr_number=191, base_sha=before_sha,
                    run_id=19100 + index, receipt_overrides=overrides,
                )
                decision = r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
                self.assertEqual(
                    (decision['run_release'], decision['publish'], decision['reason'], decision['ci_trust_detail']),
                    (True, False, 'pr_ci_unverified_release_required', expected_detail),
                )
    def test_pr138_same_contract_and_matching_tree_still_requires_full_release(self):
        repo = self.folder / 'pr138-contract-verified'
        before_sha = 'c1cc71f36489f035d08edf34ca51b5fc6fb95870'
        head_sha = 'ef1856d50bb4f0ac1c9a3f90004743b3b4394a2a'
        pr_head_sha = '1c56c4efb5ac8431555cc1153c4a1caf424e4c40'
        ci_checkout_sha = 'f8840f76d43f8e826dc62631b6255f7646635be8'
        shared_tree_sha = 'c2f4d6ce3f312faece6d5e54bbe3accf5af89452'
        self._write_repo_version_tree(repo, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.release = dict(draft=False, id=138, tag_name='v0.16.1-beta.1', body='published beta.1')
        real_contract = {path: (r.ROOT / path).read_bytes() for path in r.CI_TEST_CONTRACT_PATHS}
        self.api.set_commit_files(
            before_sha, '0.16.1-beta.1',
            ci_workflow=real_contract[r.REQUIRED_CI_WORKFLOW], contract_files=real_contract,
        )
        self.api.set_merged_pr_ci(
            head_sha, pr_head_sha, pr_number=138, base_sha=before_sha,
            checkout_sha=ci_checkout_sha, tree_sha=shared_tree_sha, main_tree_sha=shared_tree_sha,
            ci_workflow=real_contract[r.REQUIRED_CI_WORKFLOW], contract_files=real_contract,
        )
        self.api.files[head_sha]['windows/Cargo.toml'] += '# new Cargo feature contract\nrerank-timing = []\n'
        self.api.files[head_sha]['windows/src/engine.rs'] = '// new product behavior\n'
        outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(outputs, {
            'tag': 'v0.16.1-beta.1',
            'build_linux': 'false',
            'run_release': 'true',
            'run_full_tests': 'true',
            'reuse_source_commit': '',
            'publish': 'false',
            'reason': 'product_changes_require_full_release',
        })
        self.assertEqual(
            self._eval_parent_workflow(True, False, linux_enabled=False),
            {'windows': True, 'z-linux': False, 'publish': False},
        )
    def test_pr139_version_bump_to_new_tag_runs_full_release_and_publishes(self):
        repo = self.folder / 'pr139'
        before_sha, head_sha = 'e' * 40, 'f' * 40
        self._write_repo_version_tree(repo, '0.16.1-beta.2', extra_cargo='rerank-timing = []\n')
        self.api.set_commit_files(before_sha, '0.16.1-beta.1', extra_cargo='rerank-timing = []\n')
        self.api.set_merged_pr_ci(head_sha, '6'*40, pr_number=139, base_sha=before_sha)
        with mock.patch.dict(os.environ, {'GITHUB_REF_NAME': 'pr/release-workflow-parallelization'}):
            outputs, _ = self._run_plan_cli(repo, head_sha, ['--event-name', 'push', '--before', before_sha])
        self.assertEqual(outputs, {
            'tag': 'v0.16.1-beta.2',
            'build_linux': 'false',
            'run_release': 'true',
            'run_full_tests': 'true',
            'reuse_source_commit': before_sha,
            'publish': 'true',
            'reason': 'new_release_required',
        })
        self.assertEqual(
            self._eval_parent_workflow(True, True, linux_enabled=False),
            {'windows': True, 'z-linux': False, 'publish': True},
        )
        self.assertEqual(
            self._eval_parent_workflow(True, True, linux_enabled=True),
            {'windows': True, 'z-linux': True, 'publish': True},
        )
        self.assertEqual(
            self._eval_parent_workflow(True, True, linux_enabled=True, linux_result='failure'),
            {'windows': True, 'z-linux': True, 'publish': False},
        )
    def test_non_product_notes_can_skip_but_unknown_input_tree_requires_full(self):
        before,head=self.config['sha'], '6'*40
        self.api.set_commit_files(before,self.config['version'],recommended_stable=self.config['recommended_stable'])
        self.api.set_merged_pr_ci(head,'7'*40,base_sha=before)
        self.api.files[head]['Changelog.md']='approved non-product release notes'
        config=dict(self.config,sha=head)
        self.api.release=dict(draft=False,id=199,tag_name=config['tag'],body='published')
        decision=r.plan_release(self.api,config,event_name='push',before=before)
        self.assertEqual((decision['run_release'],decision['run_full_tests'],decision['publish']),(False,False,False))
        self.api.fail_get=lambda p: OSError('input tree unavailable') if p.startswith('/git/trees/') else None
        decision=r.plan_release(self.api,config,event_name='push',before=before)
        self.assertEqual((decision['run_release'],decision['run_full_tests'],decision['publish']),(True,True,False))
    def test_new_version_direct_push_cannot_publish_without_merged_pr_source(self):
        before='6'*40
        self.api.set_commit_files(before,'0.16.0-beta.1')
        decision=r.plan_release(self.api,self.config,event_name='push',before=before)
        self.assertEqual((decision['run_release'],decision['run_full_tests'],decision['publish']),(True,True,False))
    def test_manual_verify_runs_full_acceptance_even_when_tag_is_published(self):
        repo = self.folder / 'manual-verify'
        self._write_repo_version_tree(repo, '0.16.1-beta.2')
        self.api.release = dict(draft=False, id=139, tag_name='v0.16.1-beta.2', body='published beta.2')
        self.api.request = lambda *args: self.fail('verify mode must not contact GitHub')
        outputs, gh_cls = self._run_plan_cli(repo, 'a' * 40, ['--event-name', 'workflow_dispatch', '--verify'])
        gh_cls.assert_not_called()
        self.assertEqual(outputs, {
            'tag': 'v0.16.1-beta.2',
            'build_linux': 'false',
            'run_release': 'true',
            'run_full_tests': 'true',
            'reuse_source_commit': '',
            'publish': 'false',
            'reason': 'verify_requested',
        })
        self.assertEqual(
            self._eval_parent_workflow(True, False, linux_enabled=False),
            {'windows': True, 'z-linux': False, 'publish': False},
        )
        self.assertEqual(
            self._eval_parent_workflow(True, False, linux_enabled=True),
            {'windows': True, 'z-linux': True, 'publish': False},
        )
    def test_manual_publish_on_published_tag_does_not_rerun_release(self):
        repo = self.folder / 'manual-publish-existing'
        self._write_repo_version_tree(repo, '0.16.1-beta.2')
        self.api.release = dict(draft=False, id=139, tag_name='v0.16.1-beta.2', body='published beta.2')
        outputs, _ = self._run_plan_cli(repo, 'a' * 40, ['--event-name', 'workflow_dispatch'])
        self.assertEqual(outputs, {
            'tag': 'v0.16.1-beta.2',
            'build_linux': 'false',
            'run_release': 'false',
            'run_full_tests': 'false',
            'reuse_source_commit': '',
            'publish': 'false',
            'reason': 'existing_published_tag_no_release_needed',
        })
        self.assertEqual(
            self._eval_parent_workflow(False, False, linux_enabled=True),
            {'windows': False, 'z-linux': False, 'publish': False},
        )
    def test_version_change_pointing_to_existing_published_tag_is_refused(self):
        before_sha = 'b' * 40
        self.api.release = dict(draft=False, id=99, tag_name=self.config['tag'], body='already published')
        for before_version in ['0.16.0-beta.1', '0.16.1-beta.1']:
            with self.subTest(before_version=before_version):
                self.api.set_commit_files(before_sha, before_version)
                with self.assertRaisesRegex(ValueError, 'version_tag_conflict'):
                    r.plan_release(self.api, self.config, verify=False, event_name='push', before=before_sha)
        self.assertEqual(self.api.writes, [])
    def test_same_source_draft_release_runs_and_resumes_in_plan(self):
        self.api.release = dict(
            id=7, draft=True, tag_name=self.config['tag'],
            target_commitish=self.config['sha'],
            body='Public release notes\n\n' + r.marker(self.config),
        )
        for event_name, before in [('push', 'b' * 40), ('workflow_dispatch', '')]:
            with self.subTest(event_name=event_name):
                decision = r.plan_release(self.api, self.config, verify=False, event_name=event_name, before=before)
                self.assertEqual(decision, {'run_release': True, 'run_full_tests': True, 'publish': True, 'reason': 'draft_release_resume_required'})
    def test_plan_api_failure_fails_closed_without_skipping(self):
        http_502 = urllib.error.HTTPError('https://api.github.com/repos/o/r/releases/tags/v0.16.0-beta.2', 502, 'Bad Gateway', {}, None)
        self.api.fail_get = lambda path: http_502 if '/releases/tags/' in path else None
        with self.assertRaises(urllib.error.HTTPError):
            r.plan_release(self.api, self.config, verify=False, event_name='push', before='b' * 40)
        self.api.fail_get = None
        self.api.release = dict(draft=False, id=1, tag_name=self.config['tag'], body='published')
        self.api.fail_get = lambda path: OSError('network down') if path.startswith('/contents/') else None
        with self.assertRaises(OSError):
            r.plan_release(self.api, self.config, verify=False, event_name='push', before='b' * 40)
    def test_unavailable_before_commit_fails_closed_on_published_tag(self):
        self.api.release = dict(draft=False, id=1, tag_name=self.config['tag'], body='published')
        missing_sha = 'd' * 40
        for label, before in [
            ('empty', ''),
            ('zero_sha', '0' * 40),
            ('invalid_sha', 'not-a-commit'),
            ('missing_404', missing_sha),
        ]:
            with self.subTest(case=label):
                with self.assertRaisesRegex(ValueError, 'before_commit_unavailable'):
                    r.plan_release(self.api, self.config, verify=False, event_name='push', before=before)
        # Corrupted or mismatched version metadata at before commit must also fail closed.
        bad_sha = 'e' * 40
        self.api.set_commit_files(bad_sha, self.config['version'])
        self.api.files[bad_sha]['ui/Shanjie.UI.csproj'] = '<Project><PropertyGroup><Version>0.16.9</Version></PropertyGroup></Project>'
        with self.assertRaisesRegex(ValueError, 'before_commit_unavailable'):
            r.plan_release(self.api, self.config, verify=False, event_name='push', before=bad_sha)
        self.api.files[bad_sha]['ui/Shanjie.UI.csproj'] = {'type': 'file', 'encoding': 'none', 'content': 'raw'}
        with self.assertRaisesRegex(ValueError, 'before_commit_unavailable'):
            r.plan_release(self.api, self.config, verify=False, event_name='push', before=bad_sha)
        with self.assertRaisesRegex(ValueError, 'Unsupported release plan event'):
            r.plan_release(self.api, self.config, verify=False, event_name='pull_request', before='b' * 40)
    def test_platform_branches_remain_independent_and_keep_their_gates(self):
        jobs = {}
        for name in ['release-windows.yml', 'release-linux.yml']:
            workflow = (r.ROOT / '.github/workflows' / name).read_text(encoding='utf-8')
            sections = re.split(r'^  ([A-Za-z][A-Za-z0-9-]*):\n', workflow.split('\njobs:\n', 1)[1], flags=re.M)
            platform = dict(zip(sections[1::2], sections[2::2]))
            self.assertFalse(set(jobs) & set(platform))
            jobs.update(platform)
        dependencies = {}
        for name, body in jobs.items():
            match = re.search(r'^    needs: (.+)$', body, re.M)
            dependencies[name] = [job.strip() for job in match[1].strip('[]').split(',')] if match else []
        levels = {}
        while len(levels) < len(jobs):
            ready = {name: 1 + max((levels[parent] for parent in parents), default=0)
                     for name, parents in dependencies.items()
                     if name not in levels and all(parent in levels for parent in parents)}
            self.assertTrue(ready, 'Workflow dependencies must be complete and acyclic')
            levels.update(ready)
        self.assertEqual(levels['package'], 5)
        self.assertNotIn('release-ready', jobs)
        self.assertNotIn('z-linux-manifest', jobs)
        self.assertIn('name: CTest', jobs['seal'])
        self.assertIn('name: ${{ matrix.name }}', jobs['CTest'])
        self.assertIn('fromJSON(needs.seal.outputs.matrix)', jobs['CTest'])
        self.assertIn('max-parallel: 12', jobs['CTest'])
        self.assertIn('Upload FAST and x64 Cargo group evidence', jobs['seal'])
        self.assertIn('fail-fast: true', jobs['CTest'])
        self.assertIn("matrix.phase == 'Core' && 'Shard' || 'Verify'", jobs['CTest'])
        self.assertEqual(dependencies['aggregate'], ['CTest'])
        linux = {name for name in jobs if name.startswith('z-linux')}
        windows = set(jobs) - linux
        for platform, other_platform in [(linux, windows), (windows, linux)]:
            for name in platform:
                with self.subTest(job=name):
                    self.assertFalse(set(dependencies[name]) & other_platform)
        package = jobs['z-linux-package']
        self.assertIn('scripts/release.py manifest --platform linux', package)
        self.assertIn('build/linux/dist/release-source.json', package)
        self.assertNotIn('z-linux-artifact-gate', jobs)
        self.assertIn('scripts/release.py check --platform linux --folder build/linux-gate', package)
        self.assertLess(package.index('name: Upload Linux release assets'), package.index('path: build/linux-gate'))
        self.assertLess(package.index('path: build/linux-gate'), package.index('scripts/release.py check --platform linux'))
    def test_linux_artifact_gate_checks_only_linux_without_github(self):
        self.config['build_linux'] = True
        linux = self.folder / 'linux'
        linux.mkdir()
        package = linux / 'shanjie-linux-fixture.tar.gz'
        package.write_bytes(b'synthetic Linux package')
        checksum = package.with_name(package.name + '.sha256')
        checksum.write_text(f'{r.filehash(package)}  {package.name}\n', encoding='utf-8')
        r.manifest(self.config, 'linux', linux)
        with mock.patch.object(sys, 'argv', ['release.py', 'check', '--platform', 'linux', '--folder', str(linux)]), \
             mock.patch.object(r, 'configuration', return_value=self.config), \
             mock.patch.object(r, 'GitHub', side_effect=AssertionError('Linux gate must not contact GitHub')):
            r.main()
        for folder in [self.folder, self.package.parent]:
            with self.subTest(folder=folder), self.assertRaises(ValueError):
                r.assets_for(self.config, folder, 'linux')
        self.assertEqual(self.api.writes, [])
    def test_linux_artifact_gate_rejects_invalid_manifest_and_hashes(self):
        linux = self.folder / 'linux'
        linux.mkdir()
        package = linux / 'shanjie-linux-fixture.tar.gz'
        package.write_bytes(b'synthetic Linux package')
        checksum = package.with_name(package.name + '.sha256')
        checksum.write_text(f'{r.filehash(package)}  {package.name}\n', encoding='utf-8')
        r.manifest(self.config, 'linux', linux)
        source = linux / 'release-source.json'
        original = {path: path.read_bytes() for path in [package, checksum, source]}
        record = json.loads(original[source])
        cases = [(package, b'changed Linux package!!'), (checksum, b'0'*len(original[checksum]))]
        for field, value in [('sha', 'b'*40), ('tag', 'v0.0.0'), ('platform', 'windows')]:
            cases.append((source, json.dumps(record | {field: value}).encode()))
        for path, content in cases + [(path, None) for path in original]:
            with self.subTest(path=path.name, content=content):
                for target, data in original.items():
                    target.write_bytes(data)
                if content is None:
                    path.unlink()
                else:
                    path.write_bytes(content)
                with self.assertRaises(ValueError):
                    r.assets_for(self.config, linux, 'linux')
        for path, data in original.items():
            path.write_bytes(data)
        checksum.write_text(f'{"0"*64}  {package.name}\n', encoding='utf-8')
        record['assets'][1]['sha256'] = r.filehash(checksum)
        source.write_text(json.dumps(record), encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'checksum does not match'):
            r.assets_for(self.config, linux, 'linux')
    def test_first_publication_is_draft_until_both_assets_verify(self):
        r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes[0][2]['draft'], True)
        self.assertFalse(self.api.release['draft'])
        self.assertTrue(self.api.release['prerelease'])
        self.assertEqual(self.api.writes[-1][2]['make_latest'], 'false')
        self.assertEqual(len(self.api.uploads), 2)
    def test_partial_upload_and_whole_workflow_retry_resume(self):
        self.api.fail_upload = self.package.name + '.sha256'
        with self.assertRaises(OSError): r.publish(self.api, self.config, self.folder)
        self.assertTrue(self.api.release['draft'])
        self.assertTrue(r.plan(self.api, self.config, False))
        self.api.fail_upload = None
        r.publish(self.api, self.config, self.folder)
        self.assertEqual(len(self.api.uploads), 2)
        self.assertEqual(sum(m == 'POST' for m, _, _ in self.api.writes), 1)
        self.assertFalse(self.api.release['draft'])
    def test_publication_failure_retries_without_reuploading(self):
        self.api.fail_publish = True
        with self.assertRaises(OSError): r.publish(self.api, self.config, self.folder)
        self.assertTrue(self.api.release['draft'])
        self.api.fail_publish = False
        r.publish(self.api, self.config, self.folder)
        self.assertEqual(len(self.api.uploads), 2)
    def test_published_release_is_never_modified(self):
        self.api.release = dict(draft=False, id=1, body='existing public release')
        self.assertFalse(r.plan(self.api, self.config, False))
        r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes, [])
        self.assertEqual(self.api.uploads, [])
    def test_foreign_or_unmarked_draft_cannot_be_resumed(self):
        for body in ['', '<!-- shanjie-release-source:' + 'b'*40 + ' -->']:
            self.api.release = dict(draft=True, body=body, target_commitish=self.config['sha'], tag_name=self.config['tag'])
            with self.assertRaises(ValueError): r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes, [])
    def test_existing_tag_for_another_source_is_refused(self):
        self.api.ref = {'object': {'type': 'commit', 'sha': 'b'*40}}
        with self.assertRaises(ValueError): r.plan(self.api, self.config, False)
        self.assertEqual(self.api.writes, [])
    def test_incomplete_platforms_do_not_create_a_draft(self):
        self.config['build_linux'] = True
        with self.assertRaises(ValueError): r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes, [])
    def test_changed_artifacts_do_not_create_a_draft(self):
        self.package.write_bytes(b'changed')
        with self.assertRaises(ValueError): r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes, [])
    def test_conflicting_remote_asset_stays_private_without_overwrite(self):
        self.api.fail_publish = True
        with self.assertRaises(OSError): r.publish(self.api, self.config, self.folder)
        self.api.fail_publish = False
        self.api.assets[0]['digest'] = 'sha256:' + '0'*64
        count = len(self.api.writes)
        with self.assertRaises(ValueError): r.publish(self.api, self.config, self.folder)
        self.assertEqual(len(self.api.writes), count)
        self.assertEqual(len(self.api.uploads), 2)
        self.assertTrue(self.api.release['draft'])
    def test_draft_found_only_by_listing_is_resumed_not_recreated(self):
        self.api.fail_upload = self.package.name + '.sha256'
        with self.assertRaises(OSError): r.publish(self.api, self.config, self.folder)
        self.api.fail_upload = None
        r.publish(self.api, self.config, self.folder)  # the fake refuses a second POST
        self.assertEqual(sum(m == 'POST' for m, _, _ in self.api.writes), 1)
        self.assertFalse(self.api.release['draft'])
    def test_two_drafts_for_one_tag_require_review(self):
        body = 'notes ' + r.marker(self.config)
        self.api.release = dict(id=1, draft=True, body=body, target_commitish=self.config['sha'], tag_name=self.config['tag'])
        self.api.extra = [dict(self.api.release, id=2)]
        with self.assertRaises(ValueError): r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes, [])
    def test_stable_channel_becomes_latest_only_after_verification(self):
        self.config['prerelease'] = False
        r.publish(self.api, self.config, self.folder)
        self.assertEqual(self.api.writes[-1][2]['make_latest'], 'true')

class ReleaseEvidenceTests(unittest.TestCase):
    """Offline provenance/equivalence fixtures; these are not product acceptance."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='release-evidence-')
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.old_commit, self.new_commit = '1'*40, '2'*40
        self.snapshot_id = 'a'*32
        event=self.root/'event.json';event.write_text(json.dumps(dict(before=self.old_commit)))
        event_env=mock.patch.dict(os.environ,dict(GITHUB_EVENT_PATH=str(event)))
        event_env.start();self.addCleanup(event_env.stop)
        self.api = FakeGitHub()
        self.api.set_commit_files(self.old_commit, '0.16.1-beta.1')
        self.api.set_commit_files(self.new_commit, '0.16.1-beta.2')
        workflow=b'trusted release workflow\n'
        for commit in (self.old_commit,self.new_commit):
            self.api.files[commit]['.github/workflows/release.yml']=workflow
        (self.root/'.github/workflows').mkdir(parents=True)
        (self.root/'.github/workflows/release.yml').write_bytes(workflow)
        self.fp = ev.remote_fingerprint(self.api, self.old_commit)
        self.assertEqual(self.fp, ev.remote_fingerprint(self.api, self.new_commit))
        self.environment = {
            'ImageVersion': '20261128.1', 'ImageOS': 'win2025', 'os': '10.0.26100',
            'machine': 'AMD64', 'workspace': 'D:/a/shanjie/shanjie', 'rustc': 'rustc fixture',
            'cargo': 'cargo fixture', 'cmake': 'cmake fixture', 'pwsh': 'pwsh fixture',
            'logical_processors': 4, 'native_toolchain': 'one toolchain',
            'system_dlls': {'ntdll.dll': 'd'*64},
            'cpu_id':'cpu-id', 'cpu_name':'CPU virtualized', 'cpu_max_clock_mhz':3000,
            'memory_total_bytes':16*1024**3, 'memory_available_bytes':10*1024**3,
            'background_cpu_percent':8, 'timing_runner_class':None,
        }
        names = ['core','engine_cap','engine_lm','engine_replay','engine_newword','kn_tiny'] + [f'suite_{i}' for i in range(31)]
        binaries = [{'name': n, 'executable': f'test/core/{n}.exe', 'cases': [n+'::case']} for n in names]
        shards = [{'id':'cap','tests':['engine_cap']},
                  {'id':'mixed','tests':['core'] + [n for n in names if n.startswith('suite_')]},
                  {'id':'share','tests':['kn_tiny']},
                  {'id':'latency','tests':['engine_lm','engine_replay','engine_newword']}]
        self.bundle = dict(schema=1, commit=self.old_commit, source_fingerprint='f'*64,
                           toolchain='same Rust, MSVC, SDK and Cargo build settings',
                           source_key='e'*64, manifest_directory='D:/a/shanjie/build/test/core',
                           skip_arguments=['--skip','online_only'], binaries=binaries, shards=shards,
                           files={b['executable']: ev.sha256(b['name'].encode()) for b in binaries})
        self.files = {'source-bundle.json': json.dumps(self.bundle).encode()}
        snapshot = dict(schema=1, id=self.snapshot_id,
                        source=dict(commit=self.old_commit, version='0.16.1-beta.1', clean=True),
                        files={'core-tests/'+p: d for p,d in self.bundle['files'].items()})
        snapshot['files']['core-tests/bundle.json'] = ev.sha256(self.files['source-bundle.json'])
        self.files['source-snapshot.json'] = json.dumps(snapshot).encode()
        seal = ev.sha256(self.files['source-snapshot.json'])
        prefix = f'build/reports/release-parallel-{self.snapshot_id}'
        for shard in shards:
            rows = []
            for name in shard['tests']:
                stdout = f'test {name}::case ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n'.encode()
                self.files[f'{prefix}/core/{shard["id"]}/{name}.txt'] = stdout
                self.files[f'{prefix}/core/{shard["id"]}/{name}.err'] = b''
                threads = 1 if name.startswith(('engine_lm','engine_replay','engine_newword')) else 4
                rows.append(dict(name=name, status='passed', exit_code=0, executed=True, reused=False,
                                 evidence_mode='executed', duration_seconds=1.25, threads=threads, processes=1,
                                 arguments=self.bundle['skip_arguments']+[f'--test-threads={threads}'],
                                 executable_sha256=self.bundle['files'][f'test/core/{name}.exe'],
                                 stdout=name+'.txt', stderr=name+'.err', stdout_sha256=ev.sha256(stdout), stderr_sha256=ev.sha256(b'')))
            receipt = dict(schema=1, shard=shard['id'], status='passed', commit=self.old_commit,
                           snapshot_id=self.snapshot_id, snapshot_sha256=seal,
                           bundle_sha256=ev.sha256(self.files['source-bundle.json']), run_id='100',
                           environment=self.environment, results=rows)
            self.files[f'{prefix}/core/{shard["id"]}/receipt.json'] = json.dumps(receipt).encode()
        ctest, selection, results, verification = [], [], [], []
        for preset, name, directory in [('windows-x64','full.dist-data','verify-x64'),
                                        ('windows-x86','release.rust.windows.x86','verify-x86'),
                                        ('windows-x64','full.core-suite','core-junit'),
                                        ('windows-x64','release.package','package-junit')]:
            key = f'{prefix}/{directory}/junit.xml'
            self.files[key] = f'<testsuite><testcase name="{name}" status="run" time="1"/></testsuite>'.encode()
            ctest.append(dict(preset=preset, junit=key, snapshot_id=self.snapshot_id, snapshot_sha256=seal))
            selection.append(dict(preset=preset, name=name))
            results.append(dict(preset=preset, name=name, status='passed', evidence_mode='executed'))
            if directory.startswith('verify-'):
                verification.append(dict(architecture=directory.split('-')[1], root=f'{prefix}/{directory}',
                                         status='passed', commit=self.old_commit, snapshot_id=self.snapshot_id,snapshot_sha256=seal, run_id='100',
                                         files={'junit.xml':ev.sha256(self.files[key])}))
                self.files[f'{prefix}/{directory}/evidence.json']=json.dumps(verification[-1]).encode()
        permit = dict(schema=4, status='tests-passed', release=dict(snapshot_sha256=seal,candidate_id=self.snapshot_id),selection=dict(selected=selection[:-1]))
        self.files[f'{prefix}/packaging-permit.json'] = json.dumps(permit).encode()
        report = dict(schema=4, level='RELEASE', ci_safe=True, status='passed', commit=self.old_commit, version='0.16.1-beta.1',worktree_dirty=False,
                      parallel=dict(run_id='100', core_evidence=f'{prefix}/core', verification=verification),
                      release=dict(candidate_id=self.snapshot_id, snapshot_sha256=seal, architectures=['x64','x86'],packaging_permit=f'{prefix}/packaging-permit.json'),
                      subcases=[dict(name='synthetic-harness',status='passed',attempts=[dict(status='passed')])],
                      ctest=ctest, results=results, selection=dict(selected=selection))
        self.report_key = f'{prefix}/report.json'
        self.files[self.report_key] = json.dumps(report).encode()
        self.att = dict(schema=1, scope='complete-ci-safe-release', evidence_mode='executed',
                        version='0.16.1-beta.1',workflow_sha256=ev.sha256(workflow),
                        test_inventory=[dict(preset=r['preset'],name=r['name'],evidence_mode='executed') for r in results],
                        repository=self.api.repository, commit=self.old_commit, tree='b'*40,
                        test_input_fingerprint=self.fp, snapshot_id=self.snapshot_id, snapshot_sha256=seal,
                        run_id=100, run_attempt=1, required_jobs=list(ev.REQUIRED_JOBS), report=self.report_key)
        self.api.release_runs = [dict(id=100, run_attempt=1, event='push', head_branch='main', head_sha=self.old_commit,
                                     repository=dict(full_name=self.api.repository, id=self.api.repository_id),
                                     head_repository=dict(full_name=self.api.repository), path='.github/workflows/release.yml',
                                     status='completed', conclusion='success')]
        self.api.run_jobs[100] = [dict(name=n, status='completed', conclusion='success', run_attempt=1) for n in ev.REQUIRED_JOBS]
        self.api.git_commits[self.old_commit] = dict(sha=self.old_commit, tree=dict(sha='b'*40))
        self.repack()

    def repack(self):
        self.att['files'] = {p: ev.sha256(data) for p,data in self.files.items()}
        buffer=io.BytesIO()
        with zipfile.ZipFile(buffer,'w',zipfile.ZIP_DEFLATED) as archive:
            for p,data in self.files.items(): archive.writestr(p,data)
            archive.writestr('attestation.json',json.dumps(self.att))
        self.raw=buffer.getvalue()
        self.api.artifact_zips[1000]=self.raw
        self.api.run_artifacts[100]=[dict(id=1000,name='release-test-attestation-100-1',expired=False,
                                        digest='sha256:'+ev.sha256(self.raw),workflow_run=dict(
                                            id=100,repository_id=self.api.repository_id,head_repository_id=self.api.repository_id,
                                            head_sha=self.old_commit,head_branch='main'))]

    def prepare(self):
        self.new_bundle = copy.deepcopy(self.bundle);self.new_bundle['commit']=self.new_commit
        bundle_root=self.root/'current-core';bundle_root.mkdir(exist_ok=True)
        (bundle_root/'bundle.json').write_text(json.dumps(self.new_bundle))
        (self.root/'RELEASE-CHANNEL.json').write_text(json.dumps(dict(version='0.16.1-beta.2',build_linux=False)))
        with mock.patch.dict(os.environ,dict(GITHUB_EVENT_NAME='push',GITHUB_REF='refs/heads/main')), \
             mock.patch.object(ev,'local_fingerprint',return_value=self.fp), \
             mock.patch.object(ev.subprocess,'check_output',return_value=self.new_commit):
            eligible=ev.prepare(self.api,self.root,self.old_commit,bundle_root,self.root/'snapshot/ci-reuse')
        dest=self.root/'snapshot/core-tests';dest.mkdir(parents=True,exist_ok=True)
        (dest/'bundle.json').write_text(json.dumps(self.new_bundle))
        return eligible

    def test_future_version_full_source_and_exact_inputs_permit_only_proven_reuse(self):
        self.assertEqual(self.prepare(), sorted(b['name'] for b in self.bundle['binaries']))
        source=self.root/'snapshot/ci-reuse/source.zip'
        self.assertEqual(source.read_bytes(),self.raw)
        output=self.root/'current-results';output.mkdir()
        row=ev.reuse_row(self.root/'snapshot','kn_tiny',self.environment,output)
        self.assertEqual((row['evidence_mode'],row['executed'],row['reused']),('reused',False,True))
        self.assertEqual((row['source']['run_id'],row['source']['run_attempt'],row['source']['commit']),(100,1,self.old_commit))
        self.assertEqual(row['source']['artifact_digest'],'sha256:'+ev.sha256(self.raw))
        self.assertEqual(source.read_bytes(),self.raw)
        evidence=self.root/'core/share';evidence.mkdir(parents=True)
        (evidence/'receipt.json').write_text(json.dumps(dict(environment=self.environment,results=[row])))
        ev.check_current_core(self.root,self.root/'snapshot',evidence.parent)
        row['source']['run_attempt']=2
        (evidence/'receipt.json').write_text(json.dumps(dict(environment=self.environment,results=[row])))
        with self.assertRaises(ValueError):ev.check_current_core(self.root,self.root/'snapshot',evidence.parent)

    def test_full_main_attestation_writer_preserves_original_receipts_and_rejects_local(self):
        for path,data in self.files.items():
            if path.startswith('build/reports/'):
                dest=self.root/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
        snapshot=self.root/'build/release-snapshots'/self.snapshot_id
        (snapshot/'core-tests').mkdir(parents=True)
        (snapshot/'snapshot.json').write_bytes(self.files['source-snapshot.json'])
        (snapshot/'core-tests/bundle.json').write_bytes(self.files['source-bundle.json'])
        (self.root/'UPSTREAM.json').write_text(json.dumps(dict(commit='3'*40)))
        (self.root/'MODEL-SOURCES.json').write_text('{}')
        env=dict(GITHUB_ACTIONS='true',GITHUB_EVENT_NAME='push',GITHUB_REF='refs/heads/main',
                 GITHUB_REPOSITORY=self.api.repository,GITHUB_RUN_ID='100',GITHUB_RUN_ATTEMPT='1')
        output=self.root/'written'
        with mock.patch.dict(os.environ,env),mock.patch.object(ev,'local_fingerprint',return_value=self.fp),mock.patch.object(ev.subprocess,'check_output',return_value='b'*40):
            self.assertTrue(ev.create(self.root,self.snapshot_id,output))
        for path,data in self.files.items():self.assertEqual((output/path).read_bytes(),data)
        with mock.patch.dict(os.environ,dict(env,GITHUB_ACTIONS='false')):
            with self.assertRaises(ValueError):ev.create(self.root,self.snapshot_id,self.root/'local-rejected')

    def test_pr139_historical_source_without_new_attestation_requires_full(self):
        self.api.run_artifacts[100]=[]
        with self.assertRaisesRegex(ValueError,'attestation missing'):self.prepare()
        self.assertFalse((self.root/'snapshot/ci-reuse/source.json').exists())

    def test_source_run_identity_attempt_status_and_jobs_are_mandatory(self):
        original=copy.deepcopy(self.api.release_runs[0])
        for field,value in [('event','pull_request'),('head_branch','feature'),('head_sha',self.new_commit),
                            ('status','in_progress'),('conclusion','failure'),('conclusion','cancelled'),
                            ('path','.github/workflows/ci.yml'),('repository',dict(full_name='foreign/fork')),
                            ('head_repository',dict(full_name='foreign/fork')),('run_attempt',2)]:
            with self.subTest(field=field,value=value):
                self.api.release_runs[0]=dict(original,**{field:value})
                with self.assertRaises((ValueError,KeyError)):ev.fetch_source(self.api,self.old_commit)
        self.api.release_runs[0]=original
        for conclusion in ('skipped','failure','cancelled'):
            self.api.run_jobs[100][-1]['conclusion']=conclusion
            with self.assertRaises(ValueError):ev.fetch_source(self.api,self.old_commit)
        self.api.run_jobs[100][-1]['conclusion']='success'
        self.api.run_jobs[100].pop(1)
        with self.assertRaises(ValueError):ev.fetch_source(self.api,self.old_commit)

    def test_artifact_provenance_expiry_and_digest_are_mandatory(self):
        original=copy.deepcopy(self.api.run_artifacts[100][0])
        mutations=[dict(expired=True),dict(digest=None),dict(digest='sha256:'+'0'*64),dict(name='old-format'),
                   dict(workflow_run=dict(original['workflow_run'],id=99)),
                   dict(workflow_run=dict(original['workflow_run'],repository_id=99)),
                   dict(workflow_run=dict(original['workflow_run'],head_repository_id=99)),
                   dict(workflow_run=dict(original['workflow_run'],head_sha=self.new_commit))]
        for change in mutations:
            with self.subTest(change=change):
                self.api.run_artifacts[100]=[dict(original,**change)]
                with self.assertRaises(ValueError):ev.fetch_source(self.api,self.old_commit)

    def test_incomplete_failed_reused_or_altered_original_results_are_rejected(self):
        original=copy.deepcopy(self.files)
        for case in ('missing_architecture','failed_junit','missing_package','wrong_snapshot','reused_core','missing_assertion','log_hash','core_count'):
            with self.subTest(case=case):
                self.files=copy.deepcopy(original)
                report=json.loads(self.files[self.report_key])
                if case=='missing_architecture':report['parallel']['verification'].pop()
                elif case=='failed_junit':self.files[report['ctest'][-1]['junit']]=b'<testsuite><testcase name="release.package"><skipped/></testcase></testsuite>'
                elif case=='missing_package':report['ctest'].pop()
                elif case=='wrong_snapshot':report['release']['snapshot_sha256']='0'*64
                else:
                    key=ev.report_key(report['parallel']['core_evidence'])+'/share/receipt.json'
                    receipt=json.loads(self.files[key])
                    if case=='reused_core':receipt['results'][0]['reused']=True
                    elif case=='log_hash':receipt['results'][0]['stdout_sha256']='0'*64
                    elif case=='core_count':receipt['results']=[]
                    else:
                        log=key.rsplit('/',1)[0]+'/'+receipt['results'][0]['stdout']
                        self.files[log]=b'test result: ok. 1 passed; 0 failed; 0 ignored;\n'
                        receipt['results'][0]['stdout_sha256']=ev.sha256(self.files[log])
                    self.files[key]=json.dumps(receipt).encode()
                self.files[self.report_key]=json.dumps(report).encode();self.repack()
                with self.assertRaises(ValueError):ev.fetch_source(self.api,self.old_commit)

    def test_version_normalization_preserves_all_other_product_and_policy_inputs(self):
        baseline=ev.remote_fingerprint(self.api,self.new_commit)
        files=copy.deepcopy(self.api.files[self.new_commit])
        for path,change in [('windows/src/engine.rs','new behavior'),('windows/Cargo.toml','\n[features]\nnew-feature=[]\n'),
                            ('windows/Cargo.lock','\n[[package]]\nname="dependency"\nversion="2"\n'),
                            ('CMakeLists.txt','\nadd_test(...)'),('scripts/test.ps1','\n# changed controller'),
                            ('scripts/release.py','\n# changed CI generator'),('UPSTREAM.json','new pin'),
                            ('MODEL-SOURCES.json','new model'),('RELEASE-CHANNEL.json','policy')]:
            with self.subTest(path=path):
                self.api.files[self.new_commit]=copy.deepcopy(files)
                if path=='RELEASE-CHANNEL.json':
                    channel=json.loads(files[path]);channel['build_linux']=True
                    self.api.files[self.new_commit][path]=json.dumps(channel)
                else:self.api.files[self.new_commit][path]=files.get(path,'')+change
                self.assertNotEqual(ev.remote_fingerprint(self.api,self.new_commit),baseline)
        self.api.files[self.new_commit]=copy.deepcopy(files)
        self.api.files[self.new_commit]['docs/releases/RELEASE-NOTES-0.16.1-beta.2.md']='approved notes'
        self.api.files[self.new_commit]['Changelog.md']='approved changelog'
        self.assertEqual(ev.remote_fingerprint(self.api,self.new_commit),baseline)
        for field,value in [('prerelease',False),('recommended_stable','0.16.1'),('unknown',True)]:
            self.api.files[self.new_commit]=copy.deepcopy(files)
            channel=json.loads(files['RELEASE-CHANNEL.json']);channel[field]=value
            self.api.files[self.new_commit]['RELEASE-CHANNEL.json']=json.dumps(channel)
            if field=='unknown':
                with self.assertRaises(ValueError):ev.remote_fingerprint(self.api,self.new_commit)
            else:self.assertNotEqual(ev.remote_fingerprint(self.api,self.new_commit),baseline)

    def test_changed_new_binary_source_or_test_inputs_require_full(self):
        bundle_root=self.root/'bundle';bundle_root.mkdir()
        (self.root/'RELEASE-CHANNEL.json').write_text('{"build_linux":false,"version":"0.16.1-beta.2"}')
        for field,value in [('files',{}),('source_key','changed'),('manifest_directory','other root'),('skip_arguments',[]),('toolchain','different MSVC/SDK'),('toolchain','')]:
            changed=dict(self.bundle,**{field:value})
            (bundle_root/'bundle.json').write_text(json.dumps(changed))
            with self.subTest(field=field),mock.patch.dict(os.environ,dict(GITHUB_EVENT_NAME='push',GITHUB_REF='refs/heads/main')),mock.patch.object(ev,'local_fingerprint',return_value=self.fp),mock.patch.object(ev.subprocess,'check_output',return_value=self.new_commit):
                with self.assertRaises(ValueError):ev.prepare(self.api,self.root,self.old_commit,bundle_root,self.root/'candidate')
        self.assertFalse((self.root/'candidate/source.json').exists())

    def test_environment_mismatch_and_unproven_timing_suites_execute(self):
        self.prepare()
        for environment in [dict(self.environment,ImageVersion='new image'),dict(self.environment,logical_processors=8),dict(self.environment,rustc='other rustc'),{}]:
            self.assertIsNone(ev.reuse_row(self.root/'snapshot','kn_tiny',environment,None))
        for name in ('engine_lm','engine_replay','engine_newword'):
            self.assertIsNone(ev.reuse_row(self.root/'snapshot',name,self.environment,None))
        for name in ('core','engine_cap'):
            self.assertIsNotNone(ev.reuse_row(self.root/'snapshot',name,self.environment,None))

    def test_single_core_executable_mismatch_only_removes_its_own_reuse(self):
        self.prepare()
        snapshot=self.root/'snapshot'
        before=ev.core_candidates(snapshot/'ci-reuse',self.new_bundle,self.environment)
        changed=copy.deepcopy(self.new_bundle)
        changed['files']['test/core/engine_cap.exe']='0'*64
        after=ev.core_candidates(snapshot/'ci-reuse',changed,self.environment)
        self.assertIn('engine_cap',before)
        self.assertEqual(set(after),set(before)-{'engine_cap'})
        changed['files']['test/data/fixture']='1'*64
        with self.assertRaisesRegex(ValueError,'input'):ev.core_candidates(snapshot/'ci-reuse',changed,self.environment)

    def test_timing_evidence_requires_cpu_load_memory_and_dedicated_runner(self):
        proven=dict(self.environment,timing_runner_class='dedicated-isolated')
        self.assertTrue(ci.environment_matches(proven,proven,timing=True))
        for changed in (dict(proven,cpu_name='different'),dict(proven,background_cpu_percent=50),
                        dict(proven,memory_available_bytes=1),dict(proven,timing_runner_class=None)):
            self.assertFalse(ci.environment_matches(proven,changed,timing=True))

    def test_manual_verify_linux_and_reused_baselines_cannot_authorize_reuse(self):
        with mock.patch.dict(os.environ,dict(GITHUB_EVENT_NAME='workflow_dispatch',GITHUB_REF='refs/heads/main')):
            with self.assertRaises(ValueError):ev.prepare(self.api,self.root,self.old_commit,self.root,self.root/'output')
        self.files[self.report_key]=json.dumps(dict(json.loads(self.files[self.report_key]),schema=5)).encode();self.repack()
        with self.assertRaises(ValueError):ev.fetch_source(self.api,self.old_commit)
        (self.root/'RELEASE-CHANNEL.json').write_text('{"build_linux":true}')
        with mock.patch.dict(os.environ,dict(GITHUB_EVENT_NAME='push',GITHUB_REF='refs/heads/main')):
            with self.assertRaisesRegex(ValueError,'Linux reuse is unproven'):ev.prepare(self.api,self.root,self.old_commit,self.root,self.root/'output')

class PartitionedReleaseTests(unittest.TestCase):
    """Exercise the schema 6 producer and consumer against isolated CTest-shaped data."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='partitioned-release-')
        self.addCleanup(directory.cleanup)
        self.root = pathlib.Path(directory.name)
        self.snapshot = self.root / 'build/release-snapshots' / ('a' * 32)
        self.snapshot.mkdir(parents=True)
        self.evidence = self.root / 'build/reports/release-parallel-fixture'
        self.evidence.mkdir(parents=True)
        self.identity = dict(commit='1'*40,version='0.16.1-beta.2',snapshot_id=self.snapshot.name,
                             run_id='123',run_attempt='2',test_input_fingerprint='c'*64)
        self.bundle = dict(binaries=[dict(name=n, executable=f'test/{n}.exe', cases=['pass']) for n in ci.COSTS['core_seconds']],
                           files={f'test/{n}.exe': ev.sha256(n.encode()) for n in ci.COSTS['core_seconds']},
                           skip_arguments=['--skip','platform_exception'],commit=self.identity['commit'])
        x64 = [self.row(n, 'ci;fast;full') for n in ci.FAST]
        x64 += [self.row('full.mixed-regression.x64','full;rust'),
                self.row('full.dist-data','full;data', case='dist-data'),
                self.row('full.payload','full;fixture',FIXTURES_SETUP='RuntimePayload'),
                self.row('full.native.x64','full;tsf',FIXTURES_REQUIRED='RuntimePayload',RESOURCE_LOCK='model-runtime-0',
                         PROCESSORS=2, subcases=[dict(name='full.native.x64/keyboard1',parameters={'keyboard':1})]),
                self.row('full.cleanup','full;fixture',FIXTURES_CLEANUP='RuntimePayload'),
                self.row('full.core-suite','full;core'), self.row('release.package','release;package'),
                self.row('release.ui.verify','release;ui')]
        x86 = [self.row('release.windows-unit.x86','release;rust'),
               self.row('release.mixed-regression.x86','release;rust'),
               self.row('release.native.x86','release;tsf'), self.row('release.other.x86','release;tsf')]
        self.inventories = {'windows-x64':{'tests':x64},'windows-x86':{'tests':x86}}
        self.plan = ci.make_plan(self.inventories,self.identity,self.bundle)
        self.plan['validation_environment']={}
        self.put(self.snapshot/'ci-plan.json', self.plan)
        self.put(self.snapshot/'core-tests/bundle.json',self.bundle)
        self.bundle_hash=ev.sha256((self.snapshot/'core-tests/bundle.json').read_bytes())
        self.put(self.snapshot/'snapshot.json',dict(schema=1,id=self.snapshot.name,
                 source=dict(commit=self.identity['commit'],version=self.identity['version'],clean=True),
                 files={'payload/data/model':'a'*64,'payload/profiles/windows.ini':'b'*64,
                        'core-tests/bundle.json':self.bundle_hash,
                        'ci-plan.json':ev.sha256((self.snapshot/'ci-plan.json').read_bytes()),
                        **{'core-tests/'+p:h for p,h in self.bundle['files'].items()}}))
        self.snapshot_hash=ev.sha256((self.snapshot/'snapshot.json').read_bytes())

    @staticmethod
    def row(name,labels,case='fixture',subcases=None,**properties):
        return dict(name=name,command=['pwsh.exe','-File','ctest-case.ps1','-CaseId',case,'-TestName',name],
                    properties=[dict(name=k,value=v) for k,v in dict(properties,LABELS=labels).items()],
                    subcases=subcases or [])

    @staticmethod
    def put(path,value):
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_text(json.dumps(value),encoding='utf-8')

    def write_evidence(self, reused=None, environment=None):
        reused=reused or {}
        for part in self.plan['native']:
            folder=self.evidence/('verify-'+part['id'])
            folder.mkdir()
            preset='windows-'+part['architecture']
            names=ci.fixture_closure(self.plan,part)
            reused_here={name:reused[name] for name in names if name in reused}
            self.put(folder/'ci-selection.json',{'reused':reused_here})
            self.put(folder/'ci-environment.json',environment or {})
            junit=folder/'junit.xml'
            log=folder/'ctest.log'
            junit.write_text('<testsuite>'+''.join(f'<testcase name="{n}" status="run"/>' for n in names if n not in reused_here)+'</testsuite>')
            log.write_text('actual fixture run')
            subcases=[dict(sub,preset=preset,parent=row['name'],selected=True,status='passed',attempts=[dict(status='passed')])
                      for row in self.plan['rows'] if row['preset']==preset and row['name'] in names for sub in row['subcases']]
            report=dict(schema=6,status='passed',commit=self.identity['commit'],release_verify=True,ci_safe=True,subcases=subcases,
                        results=[dict(name=n,preset=preset,status='passed',
                                      source=reused_here.get(n),
                                      attempts=[] if n in reused_here else [dict(log=str(log),junit=str(junit),status='passed')]) for n in names],
                        selection=dict(selected=[dict(name=n,preset=preset) for n in names],not_selected=[]),
                        ctest=[dict(preset=preset,junit=str(junit),log=str(log),snapshot_id=self.snapshot.name,snapshot_sha256=self.snapshot_hash)])
            self.put(folder/'report.json',report)
            ci.collect_group(self.root,self.snapshot,folder,part['id'],self.snapshot_hash)
            receipt=dict(schema=6,architecture=part['architecture'],partition=part['id'],status='passed',
                         run_id=self.identity['run_id'],run_attempt=self.identity['run_attempt'],
                         version=self.identity['version'],commit=self.identity['commit'],root=str(folder),
                         snapshot_id=self.snapshot.name,snapshot_sha256=self.snapshot_hash,
                         files={p.relative_to(folder).as_posix():ev.sha256(p.read_bytes()) for p in folder.rglob('*') if p.is_file()})
            self.put(folder/'evidence.json',receipt)
        for part in self.plan['core']:
            folder=self.evidence/'core'/part['id']
            folder.mkdir(parents=True)
            results=[]
            for name in part['tests']:
                stdout=f'{name}.txt'
                stderr=f'{name}.err'
                log=b'test pass ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n'
                (folder/stdout).write_bytes(log)
                (folder/stderr).write_bytes(b'')
                threads=1 if name in ci.LATENCY_ORDER else 4
                results.append(dict(name=name, status='passed', executed=True, reused=False, evidence_mode='executed',
                                    processes=1, exit_code=0, threads=threads, duration_seconds=1.0,
                                    arguments=self.bundle['skip_arguments']+[f'--test-threads={threads}'],
                                    executable_sha256=self.bundle['files'][f'test/{name}.exe'],
                                    stdout=stdout,stderr=stderr,stdout_sha256=ev.sha256(log),stderr_sha256=ev.sha256(b'')))
            self.put(folder/'receipt.json',dict(schema=6,shard=part['id'],status='passed',commit=self.identity['commit'],
                     run_id=self.identity['run_id'],run_attempt=self.identity['run_attempt'],snapshot_id=self.snapshot.name,
                     snapshot_sha256=self.snapshot_hash,bundle_sha256=self.bundle_hash,results=results,environment={}))
        (self.evidence/'junit-core-shards.xml').write_text('<testsuite><testcase name="full.core-suite" status="run"/></testsuite>')
        (self.evidence/'ctest-core-shards.log').write_text('validated complete Core fixture')
        return ci.aggregate(self.root,self.snapshot,self.evidence,self.snapshot_hash)

    def verify(self,report):
        return ci.validate_coverage(self.plan,report,lambda p:ci.read_local(self.root,p),self.bundle,
                                    self.snapshot_hash,self.bundle_hash,before_package=True)

    def test_full_fallback_partitions_exact_coverage_and_keeps_local_fixture_and_latency_order(self):
        self.assertEqual(len(self.plan['core']),6)
        self.assertEqual(sum(len(p['tests']) for p in self.plan['core']),37)
        holders=[p for p in self.plan['core'] if set(ci.LATENCY_ORDER)&set(p['tests'])]
        self.assertEqual([p['tests'] for p in holders],[list(ci.LATENCY_ORDER)])
        part=next(p for p in self.plan['native'] if 'full.native.x64' in p['tests'])
        self.assertTrue({'full.payload','full.cleanup','full.native.x64'}<=set(ci.fixture_closure(self.plan,part)))
        self.assertEqual(len([p for p in self.plan['matrix'] if p['phase']=='Verify']),6)
        report=self.write_evidence()
        native,core=self.verify(report)
        self.assertEqual(len(core),37)
        self.assertEqual(len(report['subcases']),1)
        self.assertIn('windows-x64::fast.infrastructure',native)
        self.assertNotIn('windows-x64::release.ui.verify',native)
        self.assertEqual(report['evidence_mode'],'executed')

    def test_aggregate_refuses_missing_duplicate_wrong_snapshot_attempt_and_changed_log(self):
        report=self.write_evidence()
        self.verify(report)
        for defect in ('missing','duplicate','wrong_attempt','wrong_snapshot','missing_subcase','forged_core_mode'):
            bad=copy.deepcopy(report)
            if defect=='missing':bad['ci']['groups'].pop()
            if defect=='duplicate':bad['ci']['groups'][-1]=bad['ci']['groups'][0]
            if defect=='wrong_attempt':bad['ci']['groups'][1]['run_attempt']='1'
            if defect=='wrong_snapshot':bad['ci']['groups'][1]['snapshot_sha256']='0'*64
            if defect=='missing_subcase':bad['subcases']=[]
            if defect=='forged_core_mode':bad['results'][-1]['evidence_mode']='reused'
            with self.subTest(defect=defect),self.assertRaises(ValueError):self.verify(bad)
        (self.evidence/'verify-x86-1/ctest.log').write_text('changed after receipt')
        with self.assertRaisesRegex(ValueError,'evidence file changed'):self.verify(report)

    def test_reuse_keeps_binary_partition_nonempty_and_never_schedules_an_empty_job(self):
        names={b['name'] for b in self.bundle['binaries']}
        plans=ci.make_plan(self.inventories,self.identity,self.bundle,{n:{'evidence_mode':'reused'} for n in names-ci.TIMING_CORE})
        self.assertEqual(len(names-ci.TIMING_CORE),29)
        self.assertCountEqual([n for p in plans['core'] for n in p['tests']],ci.TIMING_CORE)
        self.assertLess(len(plans['core']),6)
        plans=ci.make_plan(self.inventories,self.identity,self.bundle,{n:{'evidence_mode':'reused'} for n in names})
        self.assertEqual(plans['core'],[])
        self.assertEqual(len(plans['matrix']),6)

    def test_native_static_data_reuse_has_original_receipt_and_no_current_junit(self):
        environment=dict(ImageOS='win2025',ImageVersion='fixture',os='fixture',machine='AMD64',workspace=str(self.root),
                         rustc='rustc',cargo='cargo',cmake='cmake',pwsh='pwsh',
                         native_toolchain='native',logical_processors=4,system_dlls={'ntdll.dll':'a'*64},
                         pwsh_sha256='b'*64,git_sha256='c'*64)
        row=next(row for row in self.plan['rows'] if row['name']=='full.dist-data')
        bindings=ci.make_bindings(self.root,self.snapshot,self.plan,row,environment,ci.declared_invocation(row))
        name='build/reports/original/ci-data.json'
        original=dict(bindings=bindings,environment=environment,evidence_mode='executed')
        source=dict(origin=dict(run_id=100,run_attempt=1,commit='6'*40,snapshot_id='a'*32,
                                snapshot_sha256='b'*64,artifact_digest='sha256:'+'d'*64,
                                test_input_fingerprint=self.identity['test_input_fingerprint']),
                    report=dict(ci=dict(case_evidence={ci.key(row):dict(original,receipt_file=name)})),
                    files={name:json.dumps(original).encode()})
        proof=ci.reuse_case(row,bindings,environment,source)
        self.assertIsNotNone(proof)
        self.assertIsNone(ci.reuse_case(row,dict(bindings,dependency_sha256='0'*64),environment,source))
        self.assertIsNone(ci.reuse_case(row,bindings,dict(environment,ImageVersion='different'),source))
        version_sensitive=next(r for r in self.plan['rows'] if r['name']=='release.windows-unit.x86')
        self.assertIsNone(ci.reuse_case(version_sensitive,bindings,environment,source))
        report=self.write_evidence({row['name']:proof},environment)
        bind=lambda r,e:ci.make_bindings(self.root,self.snapshot,self.plan,r,e,ci.declared_invocation(r))
        cases,_=ci.validate_coverage(self.plan,report,lambda p:ci.read_local(self.root,p),self.bundle,
                   self.snapshot_hash,self.bundle_hash,source,before_package=True,bind_current=bind)
        self.assertEqual(cases[ci.key(row)]['evidence_mode'],'reused')
        self.assertEqual(cases[ci.key(row)]['logs'],{})
        with self.assertRaisesRegex(ValueError,'current sealed inputs'):
            ci.validate_coverage(self.plan,report,lambda p:ci.read_local(self.root,p),self.bundle,
                   self.snapshot_hash,self.bundle_hash,source,before_package=True,bind_current=lambda r,e:{})

    def test_schema_six_attests_full_release_after_packaging_and_uses_dynamic_job_list(self):
        report=self.write_evidence()
        permit=self.evidence/'packaging-permit.json'
        self.put(permit,report)
        junit=self.evidence/'junit-package.xml'
        junit.write_text('<testsuite><testcase name="release.package" status="run"/></testsuite>')
        report['status']='passed'
        report['release']['packaging_permit']=str(permit)
        report['selection']['selected'].append(dict(preset='windows-x64',name='release.package'))
        report['results'].append(dict(preset='windows-x64',name='release.package',status='passed',evidence_mode='executed'))
        report['ctest'].append(dict(preset='windows-x64',junit=str(junit),snapshot_id=self.snapshot.name,snapshot_sha256=self.snapshot_hash))
        report_path=self.evidence/'report.json'
        self.put(report_path,report)
        files={p.relative_to(self.root).as_posix():p.read_bytes() for p in self.evidence.rglob('*') if p.is_file()}
        files.update({'source-plan.json':(self.snapshot/'ci-plan.json').read_bytes(),
                      'source-bundle.json':(self.snapshot/'core-tests/bundle.json').read_bytes(),
                      'source-snapshot.json':(self.snapshot/'snapshot.json').read_bytes()})
        att=dict(schema=2,scope='complete-ci-safe-release',evidence_mode='executed',
                 version=self.identity['version'],commit=self.identity['commit'],run_id=123,run_attempt=2,
                 snapshot_id=self.snapshot.name,snapshot_sha256=self.snapshot_hash,
                 required_jobs=ci.required_jobs(self.plan),report=report_path.relative_to(self.root).as_posix())
        att['files']={p:ev.sha256(data) for p,data in files.items()}
        buffer=io.BytesIO()
        with zipfile.ZipFile(buffer,'w',zipfile.ZIP_DEFLATED) as archive:
            for path,data in files.items():archive.writestr(path,data)
            archive.writestr('attestation.json',json.dumps(att))
        restored=ev.validate_archive(buffer.getvalue())
        self.assertEqual(len(restored[2]),37)
        self.assertIn('Windows Release / CTest x86 3',restored[0]['required_jobs'])

if __name__ == '__main__':
    unittest.main()
