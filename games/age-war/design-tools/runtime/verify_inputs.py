"""Verify private D05 delivery inputs against the private delivery index.

The index and archives are owner-private and never committed. This tool reads
them from an explicit local directory, re-hashes every present archive, tests
ZIP CRCs and re-hashes each archive member that the archive's own member list
declares. It writes a private JSON report. A missing input is reported, never
treated as a pass, and nothing is downloaded or repaired here.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import zipfile

# Member lists observed in the reconstructed-v05 delivery. Any other JSON member
# whose top level has a list under "members" or "files" with path/bytes/sha256
# records is also accepted, so a renamed list is not silently skipped.
KNOWN_MEMBER_LISTS = (
    'integrity/reconstructed-v05-members.json',
    'integrity/preview-inputs-reconstructed-v05.json',
    'manifest/reconstructed-source-member-index-v05.json',
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def member_records(archive: zipfile.ZipFile) -> tuple[str | None, list[dict]]:
    """Returns the archive's self-declared member list, if it has one."""
    names = archive.namelist()
    candidates = [n for n in KNOWN_MEMBER_LISTS if n in names]
    candidates += [n for n in names if n.startswith('integrity/') and n.endswith('.json')
                   and n not in candidates]
    for name in candidates:
        data = json.loads(archive.read(name))
        records = data.get('members', data.get('files')) if isinstance(data, dict) else None
        if isinstance(records, list) and records and all(
                {'path', 'bytes', 'sha256'} <= set(r) for r in records):
            return name, records
    return None, []


def audio_records(archive: zipfile.ZipFile) -> list[dict]:
    """The audio index pins WAV/OGG bytes and hashes per asset.

    The same index is also copied into contract-only archives that carry no
    audio. It applies only to an archive containing at least one indexed file.
    """
    name = 'manifest/audio-index-v01.json'
    names = set(archive.namelist())
    if name not in names:
        return []
    out = []
    for asset in json.loads(archive.read(name)):
        for kind in ('wav', 'ogg'):
            out.append({'path': asset[kind], 'bytes': asset[f'{kind}_bytes'],
                        'sha256': asset[f'{kind}_sha256']})
    return out if any(record['path'] in names for record in out) else []


def verify_archive(path: Path) -> dict:
    with zipfile.ZipFile(path) as archive:
        bad_crc = archive.testzip()
        names = set(archive.namelist())
        list_name, records = member_records(archive)
        records = records + audio_records(archive)
        mismatches, missing = [], []
        for record in records:
            if record['path'] not in names:
                missing.append(record['path'])
                continue
            data = archive.read(record['path'])
            if len(data) != record['bytes'] or hashlib.sha256(data).hexdigest() != record['sha256']:
                mismatches.append(record['path'])
        listed = {r['path'] for r in records}
        unlisted = sorted(n for n in names - listed
                          if not n.endswith('/') and n != list_name)
        return {
            'members': len(names),
            'crc': 'PASS' if bad_crc is None else f'FAIL:{bad_crc}',
            'member_list': list_name,
            'declared_records_checked': len(records),
            'member_mismatches': mismatches,
            'member_missing': missing,
            'unlisted_members': unlisted,
            'status': 'PASS' if bad_crc is None and records and not mismatches and not missing
            else ('PARTIAL' if bad_crc is None and not mismatches and not missing else 'FAIL'),
        }


def verify(index_path: Path, inputs: Path) -> dict:
    index = json.loads(index_path.read_text())
    report = {'index_version': index.get('version'), 'index_sha256': sha256_file(index_path),
              'entries': [], 'sourceqa': None}
    groups = (('current', index.get('current_files', [])),
              ('original_dependency', index.get('original_S02_dependencies', [])))
    for group, entries in groups:
        for entry in entries:
            local = inputs / entry['file_name']
            row = {'group': group, 'file_name': entry['file_name'],
                   'category': entry.get('category'), 'expected_bytes': entry['bytes']}
            if not local.is_file():
                row['status'] = 'NOT_RUN'
                row['reason'] = 'not present locally; not needed for the selected runtime/preview work'
            else:
                size, digest = local.stat().st_size, sha256_file(local)
                ok = size == entry['bytes'] and digest == entry['sha256_local_payload']
                row.update(bytes=size, sha256=digest, status='PASS' if ok else 'FAIL')
                if ok and local.suffix == '.zip':
                    row['archive'] = verify_archive(local)
            report['entries'].append(row)
    sourceqa = index.get('sourceqa')
    if sourceqa:
        local = inputs / sourceqa['file_name']
        report['sourceqa'] = {
            'file_name': sourceqa['file_name'],
            'declared_status': sourceqa.get('status'),
            'declared_local_members': sourceqa.get('members_local'),
            'present_locally': local.is_file(),
            'status': 'MISSING' if not local.is_file() else 'PRESENT_UNVERIFIED',
        }
    statuses = [e['status'] for e in report['entries']]
    report['summary'] = {s: statuses.count(s) for s in sorted(set(statuses))}
    report['archive_failures'] = [e['file_name'] for e in report['entries']
                                  if e.get('archive', {}).get('status') == 'FAIL']
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--index', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    report = verify(args.index, args.inputs)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=1) + '\n')
    print(json.dumps({'summary': report['summary'], 'archive_failures': report['archive_failures'],
                      'sourceqa': report['sourceqa']['status'] if report['sourceqa'] else None}))
    return 1 if 'FAIL' in report['summary'] or report['archive_failures'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
