"""Release transactions against an in-memory GitHub API; never contacts GitHub."""
import copy, io, json, os, pathlib, re, shutil, subprocess, sys, tarfile, tempfile, unittest
from unittest import mock
import release as r
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

class FakeGitHub:
    def __init__(self):
        self.release = None
        self.extra = []
        self.assets = []
        self.ref = None
        self.writes = []
        self.uploads = []
        self.fail_upload = None
        self.fail_publish = False
    def request(self, method, path, body=None):
        if method == 'GET':
            # GitHub's tag lookup returns only published releases; drafts have no tag yet.
            if '/releases/tags/' in path:
                return copy.deepcopy(self.release) if self.release and not self.release['draft'] else None
            if path.startswith('/releases?'):
                page = int(path.split('page=')[-1]) if '&page=' in path else 1
                items = [copy.deepcopy(r) for r in ([self.release] if self.release else []) + self.extra]
                return items if page == 1 else []
            if '/git/ref/tags/' in path: return self.ref
            if '/assets?' in path: return copy.deepcopy(self.assets)
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
                           prerelease=True, build_linux=False, notes='Public release notes')
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
            shutil.rmtree(self.folder)
    def test_verify_does_not_contact_or_mutate_github(self):
        self.api.request = lambda *args: self.fail('verify mode contacted GitHub')
        self.assertFalse(r.plan(self.api, self.config, True))
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
        for name, file in [('windows', 'release-windows.yml'), ('z-linux', 'release-linux.yml')]:
            self.assertIn('needs: release', callers[name])
            self.assertIn('uses: ./.github/workflows/' + file, callers[name])
        self.assertIn("if: needs.release.outputs.build_linux == 'true'", callers['z-linux'])
        publish = callers['publish']
        self.assertIn('needs: [windows, z-linux]', publish)
        self.assertIn('needs.windows.outputs.artifact_name', publish)
        self.assertIn('needs.z-linux.outputs.artifact_name', publish)
        raw = publish.split('    if: >-\n', 1)[1].split('\n    runs-on:', 1)[0].strip()
        expression = ' '.join(raw.removeprefix('${{').removesuffix('}}').split())
        expression = expression.replace('always()', 'True')
        expression = expression.replace("needs.windows.outputs.publish == 'true'", 'publish_requested')
        expression = expression.replace("needs.windows.outputs.build_linux == 'true'", 'linux_enabled')
        expression = expression.replace("needs.windows.outputs.build_linux == 'false'", '(not linux_enabled)')
        expression = expression.replace('needs.windows.result', 'windows_result').replace('needs.z-linux.result', 'linux_result')
        expression = expression.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
        for publish_requested in [False, True]:
            for linux_enabled in [False, True]:
                for windows_result in ['success', 'failure', 'cancelled', 'skipped']:
                    for linux_result in ['success', 'failure', 'cancelled', 'skipped']:
                        expected = publish_requested and windows_result == 'success' and linux_result == ('success' if linux_enabled else 'skipped')
                        with self.subTest(publish=publish_requested, linux=linux_enabled, windows=windows_result, result=linux_result):
                            self.assertEqual(eval(expression, {'__builtins__': {}}, {
                                'publish_requested': publish_requested, 'linux_enabled': linux_enabled,
                                'windows_result': windows_result, 'linux_result': linux_result}), expected)
        windows = (r.ROOT / '.github/workflows/release-windows.yml').read_text(encoding='utf-8').split('\njobs:\n', 1)[0]
        linux = (r.ROOT / '.github/workflows/release-linux.yml').read_text(encoding='utf-8').split('\njobs:\n', 1)[0]
        self.assertIn('jobs.package.outputs.artifact_name', windows)
        self.assertIn('jobs.package.outputs.publish', windows)
        self.assertIn('jobs.package.outputs.build_linux', windows)
        self.assertIn('jobs.z-linux-package.outputs.artifact_name', linux)
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
        rows = re.findall(r'- name: ([^\n]+)\n\s+phase: (Verify|Shard)\n\s+architecture: ([^\n]+)\n\s+shard: ([^\n]+)', jobs['CTest'])
        self.assertEqual(len({name for name, _, _, _ in rows}), 6)
        self.assertCountEqual([(phase, arch.strip("'"), shard.strip("'")) for _, phase, arch, shard in rows],
                              [('Verify', 'x64', ''), ('Verify', 'x86', '')] + [('Shard', '', shard) for shard in ['cap', 'mixed', 'share', 'latency']])
        self.assertIn('fail-fast: true', jobs['CTest'])
        self.assertIn("if: matrix.phase == 'Verify'", jobs['CTest'])
        self.assertIn("if: matrix.phase == 'Shard'", jobs['CTest'])
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

if __name__ == '__main__':
    unittest.main()
