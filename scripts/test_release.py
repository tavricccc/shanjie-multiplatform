"""Release transactions against an in-memory GitHub API; never contacts GitHub."""
import copy, json, pathlib, shutil, tempfile, unittest
import release as r

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
