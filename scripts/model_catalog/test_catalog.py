import copy,unittest,uuid,pathlib,urllib.error,contextlib
from unittest.mock import patch
import catalog as c
@contextlib.contextmanager
def fixture():
    # Keep failure evidence in the ignored build tree instead of deleting fixtures.
    root=c.ROOT/"build/model-catalog-tests";root.mkdir(parents=True,exist_ok=True)
    path=root/uuid.uuid4().hex;path.mkdir();yield path
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
    def publication(self,folder):
        catalog=self.catalog(self.bundle());state={"format":1,"sources":[],"pending":[]}
        c.write(folder/"publication/MODEL-CATALOG-V2.json",catalog)
        c.write(folder/"publication/MODEL-CATALOG-STATE.json",state)
        files={name:c.filehash(folder/"publication"/name) for name in ("MODEL-CATALOG-V2.json","MODEL-CATALOG-STATE.json")}
        plan={"head":"a"*40,"source_commit":"1"*40,"files":files,"sources":[]};c.write(folder/"publication-plan.json",plan)
        return catalog,state,plan
    def test_missing_or_duplicate_architecture_evidence_stops_assembly(self):
        with fixture() as tmp:
            p=pathlib.Path(tmp);m=self.bundle()
            c.write(p/"plan.json",{"candidates":[m]});r={"id":m["id"],"target":"windows-x64"}
            c.write(p/"windows-x64/results.json",[r])
            with self.assertRaisesRegex(ValueError,"architecture"):c.assemble(p)
            c.write(p/"windows-x86/results.json",[dict(r,target="windows-x86"),r])
            with self.assertRaisesRegex(ValueError,"architecture"):c.assemble(p)
    def test_semantic_rerun_does_not_write(self):
        with fixture() as tmp:
            p=pathlib.Path(tmp);catalog,state,_=self.publication(p)
            with patch.object(c,"previous",return_value=("a"*40,catalog,state)),patch.object(c,"api") as api:
                c.publish(p);api.assert_not_called()
    def test_changed_upstream_asset_blocks_before_write(self):
        with fixture() as tmp:
            p=pathlib.Path(tmp);_,_,plan=self.publication(p)
            plan["sources"]=[{"asset_id":123,"release":"SJ-new","filename":"bigram.sjlm","size":128,"digest":"sha256:"+"a"*64}]
            c.write(p/"publication-plan.json",plan)
            changed={"browser_download_url":f"https://github.com/{c.UPSTREAM}/releases/download/SJ-new/bigram.sjlm","name":"bigram.sjlm","size":128,"digest":"sha256:"+"b"*64}
            with patch.object(c,"api",return_value=changed) as api:
                with self.assertRaisesRegex(ValueError,"changed"):c.publish(p)
                self.assertEqual(api.call_count,1)
    def test_forbidden_write_fails_without_force_or_retry(self):
        self.publication_failure(403,1)
    def test_branch_race_rechecks_before_retry(self):
        self.publication_failure(422,3)
    def publication_failure(self,code,attempts):
        with fixture() as tmp:
            p=pathlib.Path(tmp);_,_,_=self.publication(p);writes=[]
            def api(path,method="GET",body=None):
                if method=="PATCH":
                    writes.append(body);raise urllib.error.HTTPError(path,code,"test",{},None)
                if "/git/commits/" in path:return {"tree":{"sha":"b"*40}}
                return {"sha":"c"*40}
            old={"format":2,"models":[],"revision":"old"}
            with patch.object(c,"previous",return_value=("a"*40,old,{})) as prior,patch.object(c,"api",side_effect=api):
                with self.assertRaises(urllib.error.HTTPError):c.publish(p)
                self.assertEqual(len(writes),attempts);self.assertEqual(prior.call_count,attempts)
                self.assertTrue(all(x["force"] is False for x in writes))
if __name__=="__main__":unittest.main()
