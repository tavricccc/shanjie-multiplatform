import copy,unittest
from unittest.mock import patch
import catalog as c
class Contract(unittest.TestCase):
    def bundle(self):
        a={"role":"bigram","repository":c.UPSTREAM,"release":"SJ-new-name","filename":"bigram.sjlm","format_magic":"SJLM0001","size":128,"sha256":"a"*64,"requires_assets":[]}
        m=c.model([a],"New naming supported")
        m["compatibility"]=[{"target":"windows-x64","profile_id":c.PROFILE,"source_commit":"1"*40,"state":"usable","checked_at":"2026-10-27T00:00:00Z","report":"reports/"+'b'*64+".json"}]
        return m
    def catalog(self,m):
        x={"format":2,"example_only":False,"models":[m],"generated_at":c.now()};x["revision"]=c.digest(c.semantic(x));return x
    def test_known_format_new_release_name(self):c.validate(self.catalog(self.bundle()))
    def test_regression_is_advisory_not_availability_gate(self):
        m=self.bundle();m["advisories"]=[{"code":"quality-regression","title":"Lower score","body":"Still usable","measurements":[{"sample_count":20,"unit":"sentences","baseline":20,"candidate":0}]}]
        m["advisory_revision"]=c.digest(c.semantic(m["advisories"]));c.validate(self.catalog(m))
    def test_unmeasured_is_still_usable(self):
        m=self.bundle();m["advisories"]=[{"code":"not-evaluated","title":"Unmeasured","body":"No comparison","measurements":[]}]
        m["advisory_revision"]=c.digest(c.semantic(m["advisories"]));c.validate(self.catalog(m))
    def test_clock_changes_do_not_change_revision(self):
        x=self.catalog(self.bundle());y=copy.deepcopy(x);y["generated_at"]="tomorrow";y["models"][0]["compatibility"][0]["checked_at"]="later"
        self.assertEqual(c.digest(c.semantic(x)),c.digest(c.semantic(y)))
    def test_example_cannot_publish(self):
        x=self.catalog(self.bundle());x["example_only"]=True
        with self.assertRaises(ValueError):c.validate(x)
    def test_changed_asset_cannot_keep_identity(self):
        m=self.bundle();m["assets"][0]["sha256"]="c"*64
        with self.assertRaises(ValueError):c.validate(self.catalog(m))
    def test_path_traversal_and_other_repository(self):
        for key,value in [("filename","../escape"),("repository","someone/else")]:
            m=self.bundle();m["assets"][0][key]=value
            m["id"]=c.model(m["assets"],m["name"])["id"]
            with self.assertRaises(ValueError):c.validate(self.catalog(m))
    def test_unknown_result_cannot_become_usable(self):
        m=self.bundle();m["compatibility"][0]["state"]="unverified"
        with self.assertRaises(ValueError):c.validate(self.catalog(m))
    def test_missing_dependency(self):
        m=self.bundle();m["assets"][0]["requires_assets"]=["c"*64];m["id"]=c.model(m["assets"],m["name"])["id"]
        with self.assertRaises(ValueError):c.validate(self.catalog(m))
    def test_incorrect_denominator(self):
        m=self.bundle();m["advisories"]=[{"code":"evaluation","title":"Bad","body":"Bad","measurements":[{"sample_count":20,"unit":"sentences","baseline":21,"candidate":0}]}]
        m["advisory_revision"]=c.digest(c.semantic(m["advisories"]))
        with self.assertRaises(ValueError):c.validate(self.catalog(m))
    def test_complete_release_pagination(self):
        with patch.object(c,"api",side_effect=[[{"draft":False}]*100,[{"draft":True},{"draft":False}]]) as call:
            self.assertEqual(len(c.releases()),101);self.assertEqual(call.call_count,2)
    def test_partial_snapshot_does_not_succeed(self):
        with patch.object(c,"api",side_effect=[["first"]*100,RuntimeError("rate limited")]):
            with self.assertRaises(Exception):c.releases()
if __name__=="__main__":unittest.main()
