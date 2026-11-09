"""Build artifacts first; resume only a same-source draft and publish after every asset verifies."""
from __future__ import annotations
import argparse, hashlib, json, os, pathlib, re, sys, urllib.error, urllib.parse, urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]

def filehash(path):
    with pathlib.Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

class GitHub:
    def __init__(self, repository, token):
        if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
            raise ValueError('Invalid repository')
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

def marker(config):
    return f'<!-- shanjie-release-source:{config["sha"]} -->'

def existing(api, config):
    tag = urllib.parse.quote(config['tag'], safe='')
    release = api.request('GET', '/releases/tags/' + tag)
    if release and not release['draft']:
        return release  # Published releases are immutable, even after a complete rerun.
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

def assets_for(config, folder):
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
    if platforms != ({'windows', 'linux'} if config['build_linux'] else {'windows'}):
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
    parser.add_argument('mode', choices=['plan', 'manifest', 'publish'])
    parser.add_argument('--root', type=pathlib.Path, default=ROOT)
    parser.add_argument('--folder', type=pathlib.Path)
    parser.add_argument('--platform', choices=['windows', 'linux'])
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    config = configuration(args.root, os.environ.get('GITHUB_SHA', ''))
    if args.mode == 'manifest':
        manifest(config, args.platform, args.folder)
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
