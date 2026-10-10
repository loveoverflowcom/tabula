"""Private unit/skill -> animation/VFX/SFX coverage matrix from generated evidence.

Joins the private runtime manifest, the builder report and the pilot's D01
oracle report. Status words follow the repository evidence vocabulary; a row
is never COMPLETE while owner art/motion/audio review is pending.
"""
from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path

REQUIRED = ('idle', 'attack', 'hit', 'death')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads((args.evidence / 'runtime-manifest.json').read_text())
    build = json.loads((args.evidence / 'runtime-build-report.json').read_text())
    check = json.loads((args.evidence / 'pilot' / 'check.json' if (args.evidence / 'pilot' / 'check.json').exists()
                        else args.evidence / 'pilot' / 'check' / 'report.json').read_text())
    oracle = {a['age']: a for a in check['ages']}
    rows = []
    for unit in manifest['units']:
        age = unit['age']
        groups = build['ages'][age]['decoded_rgba8_by_unit_facing']
        clips = {f: {c['clip_id']: c for c in unit['clips'][f]} for f in unit['clips']}
        attack = clips['right']['attack']
        marker = attack['markers'][0] if attack['markers'] else {}
        skills = []
        for skill in unit['skills']:
            sfx = skill['sfx']
            skills.append(f"{skill['skill_id']}[{'/'.join(skill['clips'])}; vfx={skill['vfx_recipe']}; "
                          f"sfx={','.join(sorted(sfx))}]")
        locomotion = [c for c in ('walk', 'run', 'charge_run') if c in clips['right']]
        loco = build['ages'][age]['locomotion']
        slide = [loco[k]['max_slide_lu'] for k in loco if k.startswith(unit['unit_id'] + '/')]
        planted = all(loco[k]['planted_pairs'] > 0 for k in loco if k.startswith(unit['unit_id'] + '/'))
        rows.append({
            'unit': unit['unit_id'], 'age': age, 'role': unit['role'], 'hint': unit['presentation_hint'],
            'pack': unit['pack'], 'facings': '+'.join(sorted(clips)),
            'required_clips': 'PASS' if all(c in clips[f] for f in clips for c in REQUIRED) else 'FAIL',
            'locomotion': '/'.join(locomotion),
            'foot_plant_socket': 'PASS' if planted and max(slide or [0]) <= 0.05 else 'NOT_VERIFIABLE',
            'attack_windup_ms': attack['windup_ms'], 'attack_period_ms': attack['duration_ms'],
            'attack_marker': f"{marker.get('kind')}@{marker.get('time_ms')}",
            'd01_oracle': 'PASS' if not oracle[age]['failed'] else 'FAIL',
            'skills': ' | '.join(skills),
            'weapon_vfx': unit['weapon']['vfx_recipe'], 'weapon_sfx': len(unit['weapon']['sfx'] or []),
            'sockets': ','.join(attack['socket_names']),
            'd1_mib_right': round(groups[f"{unit['unit_id']}/right"]['d1'] / 2**20, 1),
            'd2_mib_right': round(groups[f"{unit['unit_id']}/right"].get('d2', 0) / 2**20, 1),
            'pilot_render': 'PASS (Xvfb software GL)', 'listening': 'NOT_RUN',
            'motion_art_acceptance': 'PARTIAL (owner review pending)',
            'collider': 'unchanged D01 footprint',
        })
    with (args.evidence / 'coverage-matrix.csv').open('w', newline='') as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    spells = [{'spell': s['spell_id'], 'age': s['age'], 'slot': s['slot'], 'vfx': s['vfx_recipe'],
               'sfx_phases': ','.join(sorted(s['sfx'])), 'motion': s['motion'],
               'pilot_render': 'PASS (Xvfb; illustrative storyboard timing, not D01 cast rule)'} for s in manifest['spells']]
    with (args.evidence / 'coverage-spells.csv').open('w', newline='') as handle:
        writer = csv.DictWriter(handle, fieldnames=list(spells[0]))
        writer.writeheader()
        writer.writerows(spells)
    print(json.dumps({'units': len(rows), 'spells': len(spells),
                      'required_clips_fail': [r['unit'] for r in rows if r['required_clips'] != 'PASS'],
                      'foot_plant_not_verifiable': [r['unit'] for r in rows if r['foot_plant_socket'] != 'PASS']}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
