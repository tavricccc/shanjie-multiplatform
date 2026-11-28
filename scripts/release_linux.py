"""Bind Linux basic acceptance to the package tree transferred between jobs."""
import argparse
import json
import os
import pathlib
import platform
import re
import subprocess
import tarfile
import tomllib
import xml.etree.ElementTree as ET

from release import filehash

ROOT = pathlib.Path(__file__).resolve().parents[1]


def identity():
    commit = os.environ.get('GITHUB_SHA') or subprocess.check_output(
        ['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    if not re.fullmatch('[0-9a-f]{40}', commit):
        raise ValueError('A full Linux source commit is required')
    with (ROOT / 'linux/ibus/Cargo.toml').open('rb') as stream:
        version = tomllib.load(stream)['package']['version']
    return {'commit': commit, 'version': version, 'run_id': os.environ.get('GITHUB_RUN_ID')}


def payload_files(payload):
    return {path.relative_to(payload).as_posix(): filehash(path)
            for path in sorted(payload.rglob('*')) if path.is_file()}


def check_report(report):
    inventory = json.loads((report / 'inventory.json').read_text(encoding='utf-8-sig'))
    expected = [test['name'] for test in inventory['tests']]
    cases = ET.parse(report / 'junit.xml').findall('.//testcase')
    names = [case.attrib['name'] for case in cases]
    if not expected or len(set(expected)) != len(expected) or len(names) != len(expected) or set(names) != set(expected):
        raise ValueError('Linux CTest evidence is incomplete or duplicated')
    if any(case.find(tag) is not None for case in cases for tag in ['failure', 'error', 'skipped']):
        raise ValueError('Every required Linux CTest case must execute and pass')
    return {name: filehash(report / name) for name in ['inventory.json', 'junit.xml', 'ctest.log']}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['attest', 'package'])
    parser.add_argument('payload', type=pathlib.Path)
    parser.add_argument('receipt', type=pathlib.Path)
    parser.add_argument('--report', type=pathlib.Path)
    parser.add_argument('--output', type=pathlib.Path)
    args = parser.parse_args(argv)
    source = identity()
    if (args.payload / 'VERSION').read_text(encoding='utf-8-sig').strip() != source['version']:
        raise ValueError('Linux payload version differs from the source')
    files = payload_files(args.payload)
    if args.operation == 'attest':
        if not args.report:
            parser.error('attest requires --report')
        reports = check_report(args.report)
        record = {'schema': 1, 'source': source, 'files': files,
                  'report': args.report.resolve().relative_to(ROOT.resolve()).as_posix(), 'reports': reports}
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
        print(f'Accepted Linux tests: {args.receipt}')
        return
    if not args.output:
        parser.error('package requires --output')
    record = json.loads(args.receipt.read_text(encoding='utf-8'))
    report = (ROOT / record['report']).resolve()
    if not report.is_relative_to((ROOT / 'build/reports').resolve()):
        raise ValueError('Linux test reports must belong to this checkout')
    if record['schema'] != 1 or record['source'] != source or record['files'] != files:
        raise ValueError('Linux test receipt belongs to another source, run or payload')
    if check_report(report) != record['reports']:
        raise ValueError('Linux test evidence changed after acceptance')
    args.output.mkdir(parents=True, exist_ok=True)
    name = f'shanjie-linux-{source["version"]}-{platform.machine()}'
    archive = args.output / (name + '.tar.gz')
    with tarfile.open(archive, 'w:gz') as bundle:
        bundle.add(args.payload, arcname=name)
    archive.with_name(archive.name + '.sha256').write_text(
        f'{filehash(archive)}  {archive.name}\n', encoding='utf-8')
    print(archive)


if __name__ == '__main__':
    main()
