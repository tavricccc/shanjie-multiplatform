"""Versioned CI partition plan and evidence ledger. Never authorizes local schema 4."""
from __future__ import annotations
import argparse, copy, functools, hashlib, json, pathlib, re
import xml.etree.ElementTree as ET
import release_evidence as evidence

SCHEMA = 6
COSTS = json.loads(pathlib.Path(__file__).with_name('release-costs.json').read_bytes())
TIMING_CORE = frozenset({'engine_fuzzy', 'engine_learn', 'engine_lm', 'engine_mixed',
                         'engine_newword', 'engine_predict', 'engine_replay', 'engine_rerank'})
LATENCY_ORDER = ('engine_lm', 'engine_replay', 'engine_newword')
FAST = ('fast.infrastructure','fast.prepare-core','fast.rust.shared','fast.rust.windows','fast.upstream-data')
DATA_CASES = frozenset({'fast.infrastructure','fast.upstream-data','full.dist-data'})
BOOTSTRAP = (*FAST, 'full.mixed-regression.x64')

def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()

def key(row):
    return row['preset'] + '::' + row['name']

def properties(test):
    return {p['name']:p['value'] for p in test.get('properties',[])}

def split_property(value):
    if value is None:return []
    if isinstance(value,list):return [part for item in value for part in str(item).split(';') if part]
    return [part for part in str(value).split(';') if part]

def classify(name, props):
    labels=split_property(props.get('LABELS'))
    if props.get('FIXTURES_SETUP'):
        return dict(category='A',policy='execute',reason='Required fixture initialization on each consuming runner')
    if name=='full.dist-data':
        return dict(category='A',policy='equivalence',reason='Pinned data, profiles and static verification under the same PowerShell and Git')
    if name in DATA_CASES or name=='fast.rust.shared':
        return dict(category='A',policy='execute',reason='Always execute as part of the per-run FAST gate')
    if name=='release.package' or 'smoke' in labels or name in ('fast.rust.windows','release.windows-unit.x86') or name.startswith('release.ui.'):
        return dict(category='B',policy='execute',reason='New-version surface, smoke, installer/version contract or UI artifact')
    return dict(category='C',policy='execute',reason='Dependency closure for the new native binaries is not yet proven equivalent')

def rows_from_inventories(inventories, require_both=True):
    rows=[]
    for preset, inventory in inventories.items():
        for test in inventory['tests']:
            p=properties(test);labels=split_property(p.get('LABELS'))
            required=('full' in labels or 'release' in labels) if preset=='windows-x64' else 'release' in labels
            if not required:continue
            ci_safe=not ('ui' in labels and 'ci-safe' not in labels)
            rows.append(dict(preset=preset,name=test['name'],command=test['command'],labels=labels,
                             locks=split_property(p.get('RESOURCE_LOCK')),processors=int(p.get('PROCESSORS',1)),
                             requires=split_property(p.get('FIXTURES_REQUIRED')),setup=split_property(p.get('FIXTURES_SETUP')),
                             cleanup=split_property(p.get('FIXTURES_CLEANUP')),depends=split_property(p.get('DEPENDS')),
                             cost=float(p.get('COST',1)),ci_required=ci_safe,classification=classify(test['name'],p),
                             subcases=test.get('subcases',[])))
    if len({key(r) for r in rows})!=len(rows) or (require_both and not all(any(r['preset']==p for r in rows) for p in ('windows-x64','windows-x86'))):
        raise ValueError('Both complete architecture inventories are required without duplicates')
    return rows

def case_cost(row):
    return COSTS['ctest_seconds'].get(key(row),row['cost'])

def native_partitions(rows):
    # Keep x64 unit and mixed tests on the same Cargo target to compile once.
    partitions=[dict(id='bootstrap',architecture='x64',name='FAST',phase='Bootstrap',tests=[r['name'] for r in rows if r['preset']=='windows-x64' and r['name'] in BOOTSTRAP])]
    if sorted(partitions[0]['tests'])!=sorted(BOOTSTRAP):raise ValueError('FAST and the x64 Cargo group are mandatory')
    for architecture in ('x64','x86'):
        remaining=[r for r in rows if r['ci_required'] and r['preset']=='windows-'+architecture and r['name'] not in ('full.core-suite','release.package') and not (architecture=='x64' and r['name'] in BOOTSTRAP)]
        bins=[[] for _ in range(3)];loads=[0.0]*3
        # The x86 unit suite and mixed regression share one Cargo compilation.
        cargo=[r for r in remaining if architecture=='x86' and (r['name']=='release.windows-unit.x86' or 'mixed-regression' in r['name'])]
        bins[0].extend(cargo);loads[0]=sum(map(case_cost,cargo))
        for row in sorted((r for r in remaining if r not in cargo),key=lambda r:(-case_cost(r),r['name'])):
            i=min(range(3),key=lambda j:(loads[j],j));bins[i].append(row);loads[i]+=case_cost(row)
        if any(not b for b in bins):raise ValueError('A native partition cannot be empty')
        for i,group in enumerate(bins,1):
            partitions.append(dict(id=f'{architecture}-{i}',architecture=architecture,name=f'CTest {architecture} {i}',phase='Verify',
                                   role='verify' if architecture=='x86' and i==1 else 'native',tests=[r['name'] for r in group]))
    assigned=[('windows-'+p['architecture'],n) for p in partitions for n in p['tests']]
    required=[(r['preset'],r['name']) for r in rows if r['ci_required'] and r['name'] not in ('full.core-suite','release.package')]
    if len(assigned)!=len(required) or set(assigned)!=set(required):raise ValueError('Native ownership must cover every required test exactly once')
    return partitions

def core_partitions(names, weights=None, maximum=6):
    if not names:return []
    weights=weights or COSTS['core_seconds']
    names=list(names)
    latency=[n for n in LATENCY_ORDER if n in names]
    atoms=[[n] for n in names if n not in latency and n!='engine_cap']
    # Preserve both the dedicated cap runner and the original sequential latency order.
    fixed=([['engine_cap']] if 'engine_cap' in names else [])+([latency] if latency else [])
    atom_weight=lambda a:sum(weights.get(n,5) for n in a)
    lower=max([atom_weight(a) for a in atoms+fixed],default=1)
    total=sum(weights.get(n,5) for n in names)
    best=None
    for count in range(max(1,len(fixed)),min(maximum,len(atoms)+len(fixed))+1):
        bins=copy.deepcopy(fixed)+[[] for _ in range(count-len(fixed))]
        available=list(range(len(fixed),count))
        if atoms and not available:continue
        loads=[atom_weight(a) for a in bins]
        for atom in sorted(atoms,key=lambda a:(-atom_weight(a),a[0])):
            i=min(available,key=lambda j:(loads[j],j));bins[i]+=atom;loads[i]+=atom_weight(atom)
        if any(not b for b in bins):continue
        best=bins
        if max(loads)<=lower*1.05:break
    if best is None:raise ValueError('Cannot schedule the Core remainder without empty jobs')
    if sorted(n for b in best for n in b)!=sorted(names):raise ValueError('Core ownership is incomplete or duplicated')
    return [dict(id=f'core-{i}',name=f'Core {i}',phase='Core',architecture='',role='core-run',tests=group) for i,group in enumerate(best,1)]

def environment_matches(old, current, timing=False):
    fields=('ImageVersion','ImageOS','os','machine','workspace','rustc','cargo','cmake','pwsh','native_toolchain',
            'NUMBER_OF_PROCESSORS','logical_processors','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUST_TEST_THREADS',
            'TZ','LANG','system_dlls','test_environment')
    required=('ImageVersion','ImageOS','os','machine','workspace','rustc','cargo','cmake','pwsh','native_toolchain','logical_processors','system_dlls')
    if not old or not current or any(not old.get(k) or not current.get(k) for k in required):
        return False
    if any(old.get(k)!=current.get(k) for k in fields):return False
    if not timing:return True
    # Same hardware and a recorded idle/load envelope. No old measurements means execution.
    for k in ('cpu_id','cpu_name','cpu_max_clock_mhz','memory_total_bytes','timing_runner_class'):
        if not old.get(k) or old.get(k)!=current.get(k):return False
    for e in (old,current):
        load=e.get('background_cpu_percent');free=e.get('memory_available_bytes')
        if type(load) not in (int,float) or not 0<=load<=20 or type(free) not in (int,float) or free<8*1024**3:
            return False
    return True

def make_plan(inventories, identity, bundle, reused_core=None):
    rows=rows_from_inventories(inventories);reused_core=reused_core or {}
    names=[b['name'] for b in bundle['binaries']]
    if len(names)!=37 or len(set(names))!=37 or not set(reused_core)<=set(names):raise ValueError('All 37 unique Core binaries are required')
    native=native_partitions(rows)
    core=core_partitions([n for n in names if n not in reused_core])
    return dict(schema=SCHEMA,source=identity,rows=rows,native=native,core=core,reused_core=reused_core,
                full_counts={p:sum(r['preset']==p for r in rows) for p in inventories},
                matrix=[{k:p.get(k,'') for k in ('id','name','phase','architecture','role')} for p in native if p['id']!='bootstrap']+
                       [{k:p[k] for k in ('id','name','phase','architecture','role')} for p in core])

def declared_invocation(row):
    result=dict(CaseId=None,Architecture=row['preset'].removeprefix('windows-'),Interaction='windows',Scenario=None,Mode=None,Keyboards=[1,2],HarnessPath=None,PayloadRoot=None)
    command=row['command']
    for i,arg in enumerate(command):
        if not isinstance(arg,str) or not arg.startswith('-') or arg[1:] not in result:continue
        name=arg[1:]
        if name=='Keyboards':
            values=[]
            for x in command[i+1:]:
                if str(x).startswith('-'):break
                values.append(int(x))
            result[name]=values
        elif i+1<len(command):result[name]=command[i+1]
    return result

def partition(plan, name):
    matches=[p for p in plan['native']+plan['core'] if p['id']==name]
    if len(matches)!=1:raise ValueError('Unknown or duplicate CI partition')
    return matches[0]

def fixture_closure(plan, part):
    preset='windows-'+part['architecture'];lookup={r['name']:r for r in plan['rows'] if r['preset']==preset}
    selected=set(part['tests'])
    while True:
        required={f for n in selected for f in lookup[n]['requires']}
        added={r['name'] for r in lookup.values() if required.intersection(r['setup']+r['cleanup'])}
        if added<=selected:break
        selected|=added
    if any(set(lookup[n]['depends'])-selected for n in selected):
        raise ValueError('CTest dependency crosses a runner boundary')
    return sorted(selected)

def actual_inventory_matches(plan, preset, inventory):
    current=rows_from_inventories({preset:inventory},require_both=False)
    actual=[r for r in current if r['preset']==preset]
    expected=[r for r in plan['rows'] if r['preset']==preset]
    fields=('name','command','labels','locks','processors','requires','setup','cleanup','depends')
    def normalize(rows):
        return sorted([{k:r[k] for k in fields} for r in rows],key=lambda r:r['name'])
    if normalize(actual)!=normalize(expected):raise ValueError('Current CTest parameters, inventory, locks or processor contract differs from the sealed plan')

def simulate_wave(rows):
    todo=copy.deepcopy(rows);done=set();active=[];clock=0.0
    while todo or active:
        used=sum(r['processors'] for r in active)
        for row in sorted(todo,key=lambda r:(-r['cost'],r['name'])):
            if used+row['processors']>4 or any(f not in done for f in row['requires']):continue
            if any(set(row['locks']).intersection(r['locks']) for r in active):continue
            todo.remove(row);active.append(dict(row,end=clock+row['duration']));used+=row['processors']
        if not active:raise ValueError('Unsatisfied CTest fixture or processor contract')
        clock=min(r['end'] for r in active)
        for row in active:
            if row['end']<=clock+1e-7:done.update(row['setup'])
        active=[r for r in active if r['end']>clock+1e-7]
    return clock

def load_source(snapshot_root, indexed=False):
    folder=snapshot_root/'ci-reuse'
    if not (folder/'source.json').is_file():return None
    origin=json.loads((folder/'source.json').read_bytes());raw=(folder/'source.zip').read_bytes()
    if origin['artifact_digest']!='sha256:'+evidence.sha256(raw):raise ValueError('Original source artifact digest changed')
    if indexed:
        blob=(folder/'source-index.json').read_bytes();manifest=json.loads((snapshot_root/'snapshot.json').read_bytes())
        if manifest['files']['ci-reuse/source-index.json']!=evidence.sha256(blob):raise ValueError('Sealed source projection changed')
        data=json.loads(blob)
        if data['origin']!=origin:raise ValueError('Source projection has another origin')
        return dict(origin=origin,report=dict(ci=dict(case_evidence=data['cases'])),receipt_hashes=data['receipt_hashes'])
    att,bundle,core,files=evidence.validate_archive(raw)
    for field in ('repository','commit','run_id','run_attempt','snapshot_id','snapshot_sha256','test_input_fingerprint'):
        if origin[field]!=att[field]:raise ValueError('Retained source identity differs from its immutable attestation')
    return dict(origin=origin,att=att,bundle=bundle,core=core,files=files,report=json.loads(files[att['report']]))

def reuse_case(row, bindings, env, source):
    if not source or row['classification']['policy']=='execute' or not bindings.get('complete'):return None
    if source['origin'].get('test_input_fingerprint')!=bindings['code']:return None
    original=source['report'].get('ci',{}).get('case_evidence',{}).get(key(row))
    if not original or original['evidence_mode']!='executed' or original['bindings']!=bindings:return None
    sensitive=row['classification']['category']=='C'
    if not environment_matches(original['environment'],env,timing=sensitive):return None
    filename=original['receipt_file']
    if 'files' in source:
        if json.loads(source['files'][filename])!={k:v for k,v in original.items() if k!='receipt_file'}:
            raise ValueError('Original per-test receipt differs from the attested file')
        receipt_hash=evidence.sha256(source['files'][filename])
    else:receipt_hash=source['receipt_hashes'][filename]
    return dict(source['origin'],case_key=key(row),case_receipt=filename,
                case_receipt_sha256=receipt_hash)

def make_bindings(root, snapshot_root, plan, row, env, invocation, binary=None):
    manifest=json.loads((snapshot_root/'snapshot.json').read_bytes());files=manifest['files']
    name=row['name'];architecture=row['preset'].removeprefix('windows-')
    code=plan['source']['test_input_fingerprint']
    # These controllers read no Windows package version. All other native cases
    # include their real executable and the product dependencies they can load.
    data_only=name in DATA_CASES or row['setup']
    prefixes=['payload/data/','payload/profiles/']
    exact=['payload/UPSTREAM.json','payload/MODEL-SOURCES.json','payload/MODEL-CATALOG.json']
    if not data_only and name!='fast.rust.shared':
        prefixes+=['payload/tools/']
        exact += [f'payload/{architecture}/Shanjie.dll']
        if any(t in name for t in ('.mode.','.interaction.','.ui.candidate.')):
            exact += [f'native/{architecture}/shanjie_windows.lib']
        if '.core.' in name:exact += [f'native/{architecture}/shanjie-reliability-check.exe']
        if name.startswith('release.ui.'):prefixes+=['payload/ui/']
    selected={p:h for p,h in files.items() if p in exact or any(p.startswith(prefix) for prefix in prefixes)}
    executable=None
    if binary:
        path=pathlib.Path(binary)
        if path.is_file():executable=evidence.filehash(path) if hasattr(evidence,'filehash') else evidence.sha256(path.read_bytes())
    complete=bool(name=='full.dist-data' and env.get('pwsh_sha256') and env.get('git_sha256') and
                  selected and any(p.startswith('payload/data/') for p in selected))
    args=copy.deepcopy(invocation)
    expected=declared_invocation(row)
    def paths(value):
        if isinstance(value,str):return value.replace('\\','/').replace(str(snapshot_root).replace('\\','/'),'$SNAPSHOT').replace(str(root).replace('\\','/'),'$REPO')
        return value
    if {k:paths(v) for k,v in invocation.items()}!={k:paths(v) for k,v in expected.items()}:
        raise ValueError('Actual CTest case, architecture, parameters or harness differ from the sealed command')
    for field,value in args.items():
        if isinstance(value,str):args[field]=value.replace('\\','/').replace(str(snapshot_root).replace('\\','/'),'$SNAPSHOT').replace(str(root).replace('\\','/'),'$REPO')
    return dict(schema=1,complete=complete,preset=row['preset'],test=name,code=code,invocation=args,
                executable_sha256=executable,dependency_sha256=digest(selected),
                files=selected if complete else None,pwsh_sha256=env.get('pwsh_sha256'),
                git_sha256=env.get('git_sha256') if name in DATA_CASES else None,
                python_sha256=env.get('python_sha256') if name=='fast.infrastructure' else None)

def collect_group(root, snapshot_root, folder, part_id, snapshot_hash):
    plan=json.loads((snapshot_root/'ci-plan.json').read_bytes())
    part=partition(plan,part_id)
    identity=plan['source']
    env=json.loads((folder/'ci-environment.json').read_bytes())
    selection=json.loads((folder/'ci-selection.json').read_bytes())
    report_path=folder/'report.json'
    report=json.loads(report_path.read_bytes())
    rows={row['name']:row for row in plan['rows'] if row['preset']=='windows-'+part['architecture']}
    case_dir=folder/'ci-cases'
    case_dir.mkdir(exist_ok=False)
    cases={}
    log_hash=functools.cache(lambda p:evidence.sha256(p.read_bytes()))
    for result in report['results']:
        name=result['name']
        row=rows[name]
        invocation=declared_invocation(row)
        binary=invocation['HarnessPath']
        if invocation['CaseId']=='core-recovery':
            binary=snapshot_root/'native'/part['architecture']/'shanjie-reliability-check.exe'
        bindings=make_bindings(root,snapshot_root,plan,row,env,invocation,binary)
        reuse=selection['reused'].get(name)
        mode='reused' if reuse else 'executed'
        logs={}
        if not reuse:
            for field in ('log','junit'):
                path=pathlib.Path(result['attempts'][-1][field])
                rel=path.relative_to(root).as_posix()
                logs[rel]=log_hash(path)
        case=dict(schema=SCHEMA,test=name,preset=row['preset'],partition=part_id,status=result['status'],
                  evidence_mode=mode,source=reuse,classification=row['classification'],
                  commit=identity['commit'],version=identity['version'],run_id=identity['run_id'],
                  run_attempt=identity['run_attempt'],snapshot_id=identity['snapshot_id'],snapshot_sha256=snapshot_hash,
                  bindings=bindings,environment=env,logs=logs)
        path=case_dir/(evidence.sha256(name.encode())+'.json')
        path.write_text(json.dumps(case,ensure_ascii=False),encoding='utf-8')
        cases[name]=dict(case,receipt_file=path.relative_to(root).as_posix())
        result['evidence_mode']=mode
    report['ci']=dict(partition=part_id,case_evidence=cases)
    report_path.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')

def validate_subcases(row, observed, mode):
    expected=row.get('subcases',[])
    selected=[c for c in observed if c.get('parent')==row['name'] and c.get('preset')==row['preset'] and c.get('selected')]
    if len(selected)!=len(expected):raise ValueError('Harness combination coverage is incomplete: '+key(row))
    by_name={c['name']:c for c in selected}
    if len(by_name)!=len(selected):raise ValueError('Duplicate harness combination')
    for case in expected:
        actual=by_name.get(case['name'])
        if not actual or actual['parameters']!=case['parameters'] or actual['status']!='passed':raise ValueError('Harness parameters or result differ')
        if mode=='executed' and (not actual.get('attempts') or actual['attempts'][-1]['status']!='passed'):
            raise ValueError('Executed harness combination did not execute')
        if mode=='reused' and (actual.get('evidence_mode')!='reused' or actual.get('attempts')):
            raise ValueError('Reused harness combination cannot claim a new executed attempt')

def validate_group(plan, receipt, read, snapshot_hash):
    identity=plan['source'];part=partition(plan,receipt['partition'])
    if (receipt['schema']!=SCHEMA or receipt['status']!='passed' or receipt['commit']!=identity['commit']
            or receipt['version']!=identity['version'] or receipt['run_id']!=identity['run_id']
            or receipt['run_attempt']!=identity['run_attempt'] or receipt['snapshot_id']!=identity['snapshot_id']
            or receipt['snapshot_sha256']!=snapshot_hash or receipt['architecture']!=part['architecture']):
        raise ValueError('CI partition belongs to another run, attempt, commit or Snapshot')
    prefix=evidence.report_key(receipt['root']).rstrip('/')+'/'
    if json.loads(read(prefix+'evidence.json'))!=receipt:
        raise ValueError('Original partition receipt was rewritten')
    for path,expected in receipt['files'].items():
        if evidence.sha256(read(prefix+path))!=expected:raise ValueError('Partition evidence file changed: '+path)
    report=json.loads(read(prefix+'report.json'))
    if (report['schema']!=SCHEMA or report['status']!='passed' or report['commit']!=identity['commit']
            or not report['release_verify'] or not report['ci_safe'] or report['ci']['partition']!=part['id']):
        raise ValueError('CI partition report is not a passing same-source verification')
    selected=fixture_closure(plan,part)
    cases=report['ci']['case_evidence']
    if sorted(cases)!=selected:raise ValueError('Every selected CTest needs a per-test executed/reused receipt')
    executed=sorted(n for n,c in cases.items() if c['evidence_mode']=='executed')
    actual=[]
    for entry in report['ctest']:
        if entry['snapshot_id']!=receipt['snapshot_id'] or entry['snapshot_sha256']!=receipt['snapshot_sha256']:
            raise ValueError('JUnit belongs to another Snapshot')
        for case in ET.fromstring(read(evidence.report_key(entry['junit']))).iter('testcase'):
            if case.get('status','run') not in ('run','passed') or any(case.find(t) is not None for t in ('failure','error','skipped')):
                raise ValueError('A partition JUnit failed, skipped or did not run')
            actual.append((entry['preset'],case.attrib['name']))
    json_names=[r['name'] for r in report['results'] if r['status']=='passed']
    expected_executed=[('windows-'+part['architecture'],n) for n in executed]
    if (sorted(actual)!=expected_executed or sorted(json_names)!=selected or len(json_names)!=len(report['results'])
            or sorted(r['name'] for r in report['selection']['selected'])!=selected):
        raise ValueError('Partition JUnit/JSON do not cover exactly the executed tests and required fixtures')
    expected_rows={r['name']:r for r in plan['rows'] if r['preset']=='windows-'+part['architecture']}
    for name,case in cases.items():
        if (case['status']!='passed' or case['preset']!=expected_rows[name]['preset'] or case['test']!=name
                or case['partition']!=part['id'] or case['schema']!=SCHEMA):
            raise ValueError('Per-test identity or result differs')
        if case['snapshot_sha256']!=snapshot_hash or case['snapshot_id']!=identity['snapshot_id']:raise ValueError('Per-test Snapshot differs')
        if case['commit']!=identity['commit'] or case['run_id']!=identity['run_id'] or case['run_attempt']!=identity['run_attempt']:
            raise ValueError('Per-test source or attempt differs')
        if case['evidence_mode'] not in ('executed','reused'):raise ValueError('Unknown test evidence mode')
        if expected_rows[name]['classification']['policy']=='execute' and case['evidence_mode']!='executed':raise ValueError('Mandatory new-version or fixture test was reused')
        if case['classification']!=expected_rows[name]['classification']:raise ValueError('Test classification changed')
        result=next(r for r in report['results'] if r['name']==name)
        if result['evidence_mode']!=case['evidence_mode'] or result['preset']!=case['preset']:
            raise ValueError('Per-test evidence and result mode differ')
        if case['evidence_mode']=='reused' and (result.get('attempts') or case['logs'] or result.get('source')!=case['source']):
            raise ValueError('Reused test claims a current execution or a different source')
        if case['evidence_mode']=='executed' and (not result.get('attempts') or not case['logs'] or case['source'] is not None):
            raise ValueError('Executed test has no current attempt or log')
        if not case['receipt_file'].startswith(prefix):raise ValueError('Per-test receipt belongs to a different partition')
        raw=json.loads(read(case['receipt_file']))
        if raw!={k:v for k,v in case.items() if k!='receipt_file'}:raise ValueError('Per-test receipt was rewritten')
        for path,h in case.get('logs',{}).items():
            if evidence.sha256(read(path))!=h:raise ValueError('Per-test log changed')
        validate_subcases(expected_rows[name],report['subcases'],case['evidence_mode'])
    return {name:cases[name] for name in part['tests']},report

def validate_core_results(plan, bundle, receipts, snapshot_hash, bundle_hash, source=None):
    expected={b['name']:b for b in bundle['binaries']};found={};source_results={}
    proven={}
    if plan['reused_core']:
        if not source or source['att']['test_input_fingerprint']!=plan['source']['test_input_fingerprint']:
            raise ValueError('Core reuse requires the immutable source with the same input fingerprint')
        proven=evidence.select_core(bundle,source['bundle'],plan['validation_environment'],source['origin'],source['core'],source['files'])
    for name,row in plan['reused_core'].items():
        if proven.get(name)!=row:raise ValueError('Reused Core row differs from the independently proven original')
        found[name]=row
    expected_groups={p['id']:p for p in plan['core']}
    if set(receipts)!=set(expected_groups):raise ValueError('Missing or duplicate Core partition')
    for group,receipt in receipts.items():
        identity=plan['source']
        if (receipt['schema']!=SCHEMA or receipt['status']!='passed' or receipt['commit']!=identity['commit']
                or receipt['run_id']!=identity['run_id'] or receipt['run_attempt']!=identity['run_attempt']
                or receipt['snapshot_id']!=identity['snapshot_id'] or receipt['snapshot_sha256']!=snapshot_hash
                or receipt['bundle_sha256']!=bundle_hash or [r['name'] for r in receipt['results']]!=expected_groups[group]['tests']):
            raise ValueError('Core partition source, execution or assignment differs')
        for row in receipt['results']:
            name=row['name']
            if (name in found or row['status']!='passed' or row['executed'] is not True or row['reused'] is not False
                    or row['evidence_mode']!='executed' or row['exit_code']!=0 or row['processes']!=1
                    or row['executable_sha256']!=bundle['files'][expected[name]['executable']]):
                raise ValueError('Core result is duplicated, unexecuted or has a different binary')
            if row['arguments']!=bundle['skip_arguments']+[f"--test-threads={row['threads']}"] or row['threads']<1 or (name in LATENCY_ORDER and row['threads']!=1):
                raise ValueError('Core parameters or isolated thread count changed')
            found[name]=row
    if len(found)!=37 or set(found)!=set(expected):raise ValueError('All 37 Core binaries must be executed or proven reused exactly once')
    return found

def validate_coverage(plan, report, read, bundle, snapshot_hash, bundle_hash, source=None, before_package=False, bind_current=None):
    if report['schema']!=SCHEMA or report['level']!='RELEASE' or not report['ci_safe'] or report['status'] not in ('tests-passed','passed'):
        raise ValueError('A completed CI-only partitioned RELEASE record is required')
    if report['commit']!=plan['source']['commit'] or report['version']!=plan['source']['version'] or report['worktree_dirty']:
        raise ValueError('CI release identity changed')
    if (report['release']['candidate_id']!=plan['source']['snapshot_id'] or report['release']['snapshot_sha256']!=snapshot_hash
            or sorted(report['release']['architectures'])!=['x64','x86']
            or report['status']!=('tests-passed' if before_package else 'passed')):
        raise ValueError('CI release Snapshot, architectures or stage differs')
    read=functools.cache(read)
    expected_parts={p['id'] for p in plan['native']}
    groups=report['ci']['groups']
    if len(groups)!=len(expected_parts) or {g['partition'] for g in groups}!=expected_parts:raise ValueError('Every native partition, including FAST, is mandatory')
    cases={};group_reports={};observed_subcases=[]
    inventory={key(row):row for row in plan['rows']}
    for receipt in groups:
        owned,raw=validate_group(plan,receipt,read,snapshot_hash);group_reports[receipt['partition']]=raw
        for name,case in owned.items():
            k=case['preset']+'::'+name
            if k in cases:raise ValueError('Duplicate native test owner')
            row=inventory[k]
            if case['evidence_mode']=='reused':
                if bind_current is None or bind_current(row,case['environment'])!=case['bindings']:
                    raise ValueError('Reused native test does not match the current sealed inputs')
                original=reuse_case(row,case['bindings'],case['environment'],source)
                if original is None or case.get('source')!=original or case['classification']['category']!='A':
                    raise ValueError('Reused native test is not supported by complete equivalent source evidence')
            cases[k]=case
            observed_subcases.extend(c for c in raw['subcases'] if c.get('parent')==name and c.get('preset')==case['preset'] and c.get('selected'))
    required={key(r) for r in plan['rows'] if r['ci_required'] and r['name'] not in ('full.core-suite','release.package')}
    if set(cases)!=required:raise ValueError('Complete native coverage is missing or duplicated')
    if report['ci']['case_evidence']!=cases:raise ValueError('Aggregate case ledger differs from partition evidence')
    if sorted(report['subcases'],key=key)!=sorted(observed_subcases,key=key):
        raise ValueError('Aggregate subcases differ from their original partitions')
    receipts={}
    for path in report['ci']['core_receipts']:
        receipt=json.loads(read(path));group=receipt['shard']
        if group in receipts:raise ValueError('Duplicate Core receipt')
        for row in receipt['results']:
            for stream in ('stdout','stderr'):
                filename=row[stream]
                if pathlib.PurePosixPath(filename).name!=filename:raise ValueError('Invalid Core log path')
                data=read(path.rsplit('/',1)[0]+'/'+filename)
                if evidence.sha256(data)!=row[stream+'_sha256']:raise ValueError('Core assertion log changed')
            log=read(path.rsplit('/',1)[0]+'/'+row['stdout']).decode('utf-8-sig')
            binary=next(b for b in bundle['binaries'] if b['name']==row['name'])
            summary=re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',log)
            if not summary or int(summary[1])+int(summary[2])!=len(binary['cases']):raise ValueError('Core assertion count is incomplete')
            for name in binary['cases']:
                if len(re.findall(r'(?m)^test '+re.escape(name)+r'(?: - should panic)? \.\.\. (?:ok|ignored(?:, [^\r\n]*)?)\r?$',log))!=1:
                    raise ValueError('Missing or duplicate Core assertion')
        receipts[group]=receipt
    core=validate_core_results(plan,bundle,receipts,snapshot_hash,bundle_hash,source)
    if report['ci']['core_evidence']!=core:raise ValueError('Aggregate Core ledger differs from original evidence')
    expected={key(r) for r in plan['rows'] if r['ci_required'] and (not before_package or r['name']!='release.package')}
    listed=[r['preset']+'::'+r['name'] for r in report['results']]
    selected=[r['preset']+'::'+r['name'] for r in report['selection']['selected']]
    if len(listed)!=len(expected) or set(listed)!=expected or len(selected)!=len(expected) or set(selected)!=expected or any(r['status']!='passed' for r in report['results']):
        raise ValueError('Aggregate must contain every current required test exactly once')
    groups_used=[entry for group in group_reports.values() for entry in group['ctest']]
    final_names={'full.core-suite'} if before_package else {'full.core-suite','release.package'}
    final_entries=[entry for entry in report['ctest'] if entry not in groups_used]
    if len(report['ctest'])!=len(groups_used)+len(final_names) or any(entry not in report['ctest'] for entry in groups_used):
        raise ValueError('Aggregate CTest reports do not match their original partitions')
    final_test_names=[]
    for entry in final_entries:
        if entry['preset']!='windows-x64' or entry['snapshot_sha256']!=snapshot_hash or entry['snapshot_id']!=plan['source']['snapshot_id']:
            raise ValueError('Final verification JUnit provenance differs')
        for case in ET.fromstring(read(evidence.report_key(entry['junit']))).iter('testcase'):
            if case.get('status','run') not in ('run','passed') or any(case.find(tag) is not None for tag in ('failure','error','skipped')):
                raise ValueError('Final verification JUnit did not pass')
            final_test_names.append(case.attrib['name'])
    if sorted(final_test_names)!=sorted(final_names):raise ValueError('Missing or duplicate Core/Package CTest result')
    must_be_mixed=any(case['evidence_mode']=='reused' for case in cases.values()) or bool(plan['reused_core'])
    if report['evidence_mode']!=('mixed' if must_be_mixed else 'executed'):
        raise ValueError('Aggregate execution mode differs from recorded evidence')
    modes={k:v['evidence_mode'] for k,v in cases.items()}
    modes['windows-x64::full.core-suite']='mixed' if plan['reused_core'] else 'executed'
    if not before_package:modes['windows-x64::release.package']='executed'
    if {key(r):r['evidence_mode'] for r in report['results']}!=modes:
        raise ValueError('Aggregate result modes contradict the evidence')
    return cases,core

def read_local(root, path):
    value=pathlib.Path(path)
    if not value.is_absolute():value=root/value
    if not value.resolve().is_relative_to((root/'build/reports').resolve()):raise ValueError('Test evidence must remain in this worktree reports')
    return value.read_bytes()

def check_core(snapshot_root, evidence_root, snapshot_hash):
    plan=json.loads((snapshot_root/'ci-plan.json').read_bytes())
    bundle_bytes=(snapshot_root/'core-tests/bundle.json').read_bytes()
    bundle=json.loads(bundle_bytes)
    if evidence.sha256((snapshot_root/'snapshot.json').read_bytes())!=snapshot_hash:raise ValueError('Core Snapshot changed')
    receipts={}
    for path in evidence_root.glob('*/receipt.json'):
        row=json.loads(path.read_bytes())
        if row['shard'] in receipts:raise ValueError('Duplicate Core receipt')
        for result in row['results']:
            binary=next(b for b in bundle['binaries'] if b['name']==result['name'])
            log=None
            for stream in ('stdout','stderr'):
                name=result[stream]
                if pathlib.PurePosixPath(name).name!=name:raise ValueError('Invalid Core log name')
                raw=(path.parent/name).read_bytes()
                if evidence.sha256(raw)!=result[stream+'_sha256']:raise ValueError('Core log hash differs')
                if stream=='stdout':log=raw.decode('utf-8-sig')
            summary=re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',log)
            if not summary or int(summary[1])+int(summary[2])!=len(binary['cases']):raise ValueError('Core assertions did not all pass')
            for case in binary['cases']:
                if len(re.findall(r'(?m)^test '+re.escape(case)+r'(?: - should panic)? \.\.\. (?:ok|ignored(?:, [^\r\n]*)?)\r?$',log))!=1:raise ValueError('Core assertion missing or duplicated')
        receipts[row['shard']]=row
    return validate_core_results(plan,bundle,receipts,snapshot_hash,evidence.sha256(bundle_bytes),load_source(snapshot_root))

def aggregate(root, snapshot_root, evidence_root, snapshot_hash):
    plan=json.loads((snapshot_root/'ci-plan.json').read_bytes());identity=plan['source']
    groups=[];reports=[]
    for part in plan['native']:
        folder=evidence_root/('verify-'+part['id'])
        receipt=json.loads((folder/'evidence.json').read_bytes());groups.append(receipt)
        reports.append(json.loads((folder/'report.json').read_bytes()))
    case_evidence={}
    ctest=[];subcases=[];results=[];selected=[]
    omitted=[dict(name=r['name'],preset=r['preset'],reason='Existing CI-safe interactive UI exclusion') for r in plan['rows'] if not r['ci_required']]
    for group,report in zip(groups,reports):
        part=partition(plan,group['partition']);preset='windows-'+part['architecture']
        ctest.extend(report['ctest'])
        for name in part['tests']:
            case=report['ci']['case_evidence'][name];case_evidence[preset+'::'+name]=case
            original=next(r for r in report['results'] if r['name']==name)
            results.append(dict(original,evidence_mode=case['evidence_mode']))
            selected.append(dict(name=name,preset=preset))
            subcases.extend(c for c in report['subcases'] if c.get('parent')==name and c.get('preset')==preset and c.get('selected'))
    core_junit=evidence_root/'junit-core-shards.xml';core_log=evidence_root/'ctest-core-shards.log'
    ctest.append(dict(preset='windows-x64',attempt=1,junit=str(core_junit),log=str(core_log),snapshot_id=identity['snapshot_id'],snapshot_sha256=snapshot_hash))
    cases=list(ET.fromstring(core_junit.read_bytes()).iter('testcase'))
    if len(cases)!=1 or cases[0].get('name')!='full.core-suite' or any(cases[0].find(t) is not None for t in ('failure','error','skipped')):raise ValueError('Core aggregate JUnit did not pass')
    selected.append(dict(name='full.core-suite',preset='windows-x64'))
    results.append(dict(name='full.core-suite',preset='windows-x64',status='passed',evidence_mode='mixed' if plan['reused_core'] else 'executed'))
    core_receipts=[str(p.relative_to(root).as_posix()) for p in sorted((evidence_root/'core').glob('*/receipt.json'))]
    core_evidence=dict(plan['reused_core'])
    for path in core_receipts:
        for result in json.loads((root/path).read_bytes())['results']:
            core_evidence[result['name']]=result
    report=dict(schema=SCHEMA,level='RELEASE',ci_safe=True,status='tests-passed',commit=identity['commit'],version=identity['version'],worktree_dirty=False,
                ctest=ctest,results=results,selection=dict(selected=selected,not_selected=omitted),subcases=subcases,
                evidence_mode='mixed' if plan['reused_core'] or any(c['evidence_mode']=='reused' for c in case_evidence.values()) else 'executed',
                ci=dict(groups=groups,core_receipts=core_receipts,case_evidence=case_evidence,core_evidence=core_evidence),
                release=dict(source_start=dict(commit=identity['commit'],version=identity['version'],clean=True),
                             source_end=dict(commit=identity['commit'],version=identity['version'],clean=True),architectures=['x64','x86'],
                             candidate_id=identity['snapshot_id'],snapshot_sha256=snapshot_hash))
    return report

def required_jobs(plan):
    return ['Plan','Windows Release / x64','Windows Release / x86','Windows Release / Core','Windows Release / CTest']+['Windows Release / '+p['name'] for p in plan['matrix']]+['Windows Release / CTest Merge','Windows Release / Package']

def validate_attested_full(att, files, bundle):
    plan=json.loads(files['source-plan.json']);snapshot=json.loads(files['source-snapshot.json'])
    if snapshot['files']['ci-plan.json']!=evidence.sha256(files['source-plan.json']):raise ValueError('Attested partition plan differs from its Snapshot')
    report=json.loads(files[att['report']])
    if att['required_jobs']!=required_jobs(plan) or plan['source']['commit']!=att['commit'] or plan['source']['run_id']!=str(att['run_id']) or plan['source']['run_attempt']!=str(att['run_attempt']):
        raise ValueError('Attested source plan or required jobs differ')
    if report.get('evidence_mode')!='executed' or plan['reused_core']:raise ValueError('Only a full executed run can create a new source baseline')
    original_permit=json.loads(files[evidence.report_key(report['release']['packaging_permit'])])
    if original_permit['ci']!=report['ci'] or original_permit['status']!='tests-passed' or original_permit['schema']!=SCHEMA:
        raise ValueError('Attested Package did not use the original accepted CI permit')
    cases,core=validate_coverage(plan,report,lambda p:files[evidence.report_key(p)],bundle,
                                 att['snapshot_sha256'],evidence.sha256(files['source-bundle.json']))
    if any(c['evidence_mode']!='executed' for c in cases.values()) or any(not c['executed'] or c['reused'] for c in core.values()):raise ValueError('A reused run cannot establish a full baseline')
    results={}
    for path in report['ci']['core_receipts']:
        path=evidence.report_key(path);receipt=json.loads(files[path])
        for row in receipt['results']:
            logs={s:files[path.rsplit('/',1)[0]+'/'+row[s]] for s in ('stdout','stderr')}
            results[row['name']]=(row,receipt,path,logs)
    return results

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=['plan','selection','collect','core-check','coverage','aggregate'])
    parser.add_argument('--root',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument('--input',type=pathlib.Path);parser.add_argument('--output',type=pathlib.Path)
    parser.add_argument('--snapshot-root',type=pathlib.Path);parser.add_argument('--partition')
    parser.add_argument('--evidence-root',type=pathlib.Path);parser.add_argument('--snapshot-hash')
    parser.add_argument('--env-file',type=pathlib.Path)
    parser.add_argument('--before-package',action='store_true')
    a=parser.parse_args()
    if a.mode=='plan':
        payload=json.loads(a.input.read_bytes())
        plan=make_plan(payload['inventories'],payload['source'],payload['bundle'],payload.get('reused_core'))
        plan['validation_environment']=payload['validation_environment']
        a.output.write_text(json.dumps(plan,indent=2,ensure_ascii=False),encoding='utf-8')
    elif a.mode=='selection':
        plan=json.loads((a.snapshot_root/'ci-plan.json').read_bytes());part=partition(plan,a.partition)
        if a.input:
            payload=json.loads(a.input.read_bytes());actual_inventory_matches(plan,payload['preset'],payload['inventory'])
        selection=fixture_closure(plan,part)
        rows=[r for r in plan['rows'] if r['preset']=='windows-'+part['architecture'] and r['name'] in selection]
        reused={}
        for row in rows:
            if row['classification']['policy']!='equivalence':continue
            env=json.loads(a.env_file.read_bytes())
            try:
                bindings=make_bindings(a.root,a.snapshot_root,plan,row,env,declared_invocation(row))
                proof=reuse_case(row,bindings,env,load_source(a.snapshot_root,indexed=True))
            except (ValueError,OSError,KeyError,TypeError) as error:
                print(f"{row['name']} will execute: {error}")
                proof=None
            if proof:reused[row['name']]=proof
        fixtures=sorted({f for row in rows for f in row['requires']+row['setup']})
        a.output.write_text(json.dumps(dict(canonical=part['tests'],selected=selection,fixtures=fixtures,reused=reused)),encoding='utf-8')
    elif a.mode=='collect':
        collect_group(a.root,a.snapshot_root,a.evidence_root,a.partition,a.snapshot_hash)
    elif a.mode=='core-check':
        core=check_core(a.snapshot_root,a.evidence_root,a.snapshot_hash)
        print(f"Verified all 37 Core binaries: {sum(r['executed'] for r in core.values())} executed, {sum(r['reused'] for r in core.values())} reused")
    elif a.mode=='aggregate':
        a.output.write_text(json.dumps(aggregate(a.root,a.snapshot_root,a.evidence_root,a.snapshot_hash),indent=2,ensure_ascii=False),encoding='utf-8')
    else:
        plan=json.loads((a.snapshot_root/'ci-plan.json').read_bytes());report=json.loads(a.input.read_bytes())
        bundle=(a.snapshot_root/'core-tests/bundle.json').read_bytes()
        cases,core=validate_coverage(plan,report,lambda p:read_local(a.root,p),json.loads(bundle),
                                     a.snapshot_hash,evidence.sha256(bundle),load_source(a.snapshot_root),a.before_package,
                                     bind_current=lambda row,env:make_bindings(a.root,a.snapshot_root,plan,row,env,declared_invocation(row)))
        print(f'Complete CI RELEASE coverage: {len(cases)} native tests and 37 Core binaries')

if __name__=='__main__':main()
