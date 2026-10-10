"""Export the private art the D06 HUD preview shows, from sealed runtime packs.

Output goes to a private directory (never Git). Every exported image records the
pack@version, logical resource and the BLAKE3 file hash from the pack manifest
it was cut from, so the preview's asset map is traceable. HUD icons that no
runtime pack contains are derived stand-ins and are labelled as such.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import tomllib
import zipfile

from PIL import Image

AGES = ('Primitive', 'Ancient', 'Feudal', 'Arcane', 'Industrial', 'Future')
VERSION = '0.5.0'


class Pack:
    def __init__(self, root: Path, name: str):
        self.name = name
        self.base = root
        self.manifest = tomllib.loads((root / name / VERSION / 'pack.toml').read_text())
        self.files = {f['name']: f for f in self.manifest['files']}
        self.regions = {}
        for resource in self.manifest['resources']:
            for variant in resource['variants']:
                density = self.files[variant['file']].get('density')
                self.regions.setdefault((resource['id'], density), (variant['file'], variant.get('region')))
        self.cache = {}

    def has(self, resource: str, density: int | None = 1) -> bool:
        return (resource, density) in self.regions

    def image(self, resource: str, density: int | None = 1) -> tuple[Image.Image, str]:
        name, region = self.regions[(resource, density)]
        if name not in self.cache:
            self.cache[name] = Image.open(self.base / self.files[name]['path']).convert('RGBA')
        image = self.cache[name]
        if region:
            image = image.crop((region['x'], region['y'], region['x'] + region['width'], region['y'] + region['height']))
        trace = f"{self.name}@{VERSION}:{resource} (blake3 {self.files[name]['hash'][:16]})"
        return image, trace

    def text(self, resource: str) -> str:
        name, _ = self.regions[(resource, None)]
        return (self.base / self.files[name]['path']).read_text()


def clips(text: str) -> tuple[list, dict]:
    units, frames, markers, definitions = [], {}, {}, {}
    for line in text.splitlines():
        f = line.split('\t')
        if f[0] == 'unit':
            units.append(f[1])
        elif f[0] == 'clip':
            definitions[(f[1], f[2], f[3])] = {'duration': float(f[5]), 'loop': f[6] == '1'}
        elif f[0] == 'marker':
            markers.setdefault((f[1], f[2], f[3]), []).append(float(f[4]))
        elif f[0] == 'frame':
            frames.setdefault((f[1], f[2], f[3]), []).append(
                {'t': float(f[5]), 'box': [float(v) for v in f[6:10]],
                 'mask': None if f[10] == '-' else [float(v) for v in f[10:14]]})
    return units, {'frames': frames, 'markers': markers, 'definitions': definitions}


def motion(pack: Pack, info: dict, out: Path, age: str) -> dict:
    """Copy only density-1 colour/VFX pages; keep exact regions and logical pivots.

    The staging command verifies all pack bytes with the Rust pilot before this
    function is called. This is a presentation export, never a combat sampler.
    """
    copied, result = {}, {'clips': {}, 'vfx': {}, 'files': {}}

    def sprite(resource: str, density: int | None = 1) -> dict:
        name, region = pack.regions[(resource, density)]
        file = pack.files[name]
        if name not in copied:
            relative = f'{age}/motion/{Path(file["path"]).name}'
            target = out / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(pack.base / file['path'], target)
            copied[name] = relative
            result['files'][relative] = {'pack': f'{pack.name}@{VERSION}', 'blake3': file['hash'], 'bytes': file['bytes']}
        return {'file': copied[name], 'rect': region, 'resource': resource}

    for key, definition in info['definitions'].items():
        unit, facing, clip = key
        frames = []
        for i, frame in enumerate(info['frames'][key]):
            frames.append({**frame, 'sprite': sprite(f'unit/{unit}/{facing}/{clip}/{i:03d}')})
        result['clips']['/'.join(key)] = {**definition, 'frames': frames}
    for line in pack.text('meta/vfx').splitlines():
        f = line.split('\t')
        if f[0] == 'vfx':
            resource = f'vfx/{f[1]}/{f[2]}/{f[3]}/{f[4]}'
            result['vfx'].setdefault(f'{f[1]}/{f[2]}/{f[3]}', []).append({
                't': float(f[5]), 'pivot': [float(f[6]), float(f[7])],
                'size': [float(f[8]), float(f[9])], 'mode': f[10], 'sprite': sprite(resource, None)})
    return result


def export(pack_root: Path, vfx_zip: Path, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    data = {'versions': {'packs': VERSION}, 'ages': {}}
    aux = zipfile.ZipFile(vfx_zip)
    aux_index = {a['entity_id']: a for a in json.loads(aux.read('manifest/auxiliary-sprite-index-v01.json'))}
    for age in AGES:
        pack = Pack(pack_root, f'age-war-{age.lower()}')
        units, info = clips(pack.text('meta/clips'))
        folder = out / age
        (folder / 'units').mkdir(parents=True, exist_ok=True)
        backdrop, trace = pack.image('scene/backdrop', None)
        backdrop.save(folder / 'backdrop.png')
        entry = {'backdrop': {'file': f'{age}/backdrop.png', 'asset': trace}, 'units': {}, 'spells': {}, 'turrets': {}}
        entry['motion'] = motion(pack, info, out, age)
        for unit in units:
            record = {}
            for facing in ('right', 'left'):
                poses = {}
                walk = 'walk' if (unit, facing, 'walk') in info['frames'] else 'run'
                attack_marker = (info['markers'].get((unit, facing, 'attack')) or [0])[0]
                attack_frames = info['frames'][(unit, facing, 'attack')]
                attack_index = max(i for i, f in enumerate(attack_frames) if f['t'] <= attack_marker + 1e-3)
                for pose, clip, index in (('idle', 'idle', 0), ('walk', walk, len(info['frames'][(unit, facing, walk)]) // 2),
                                          ('attack', 'attack', attack_index)):
                    frame = info['frames'][(unit, facing, clip)][index]
                    resource = f'unit/{unit}/{facing}/{clip}/{index:03d}'
                    image, trace = pack.image(resource)
                    name = f'{age}/units/{unit}-{facing}-{pose}.png'
                    image.save(out / name)
                    pose_entry = {'file': name, 'box': frame['box'], 'asset': trace}
                    if frame['mask'] and pack.has(f'mask/{unit}/{facing}/{clip}/{index:03d}'):
                        mask, mask_trace = pack.image(f'mask/{unit}/{facing}/{clip}/{index:03d}')
                        mask_name = f'{age}/units/{unit}-{facing}-{pose}-mask.png'
                        mask.save(out / mask_name)
                        pose_entry['mask'] = {'file': mask_name, 'box': frame['mask'], 'asset': mask_trace}
                    poses[pose] = pose_entry
                record[facing] = poses
            density = 2 if pack.has(f'unit/{unit}/right/idle/000', 2) else 1
            source, trace = pack.image(f'unit/{unit}/right/idle/000', density)
            bust = source.crop((0, 0, source.width, max(1, round(source.height * 0.62))))
            bust.thumbnail((96, 96), Image.Resampling.LANCZOS)
            icon = Image.new('RGBA', (96, 96), (0, 0, 0, 0))
            icon.alpha_composite(bust, ((96 - bust.width) // 2, 96 - bust.height))
            icon.save(folder / 'units' / f'{unit}-icon.png')
            record['icon'] = {'file': f'{age}/units/{unit}-icon.png',
                              'asset': f'DERIVED stand-in from {trace}; production needs hud/unit-icon/{unit}'}
            entry['units'][unit] = record
        recipes = {key.split('/')[1] for key, _ in pack.regions if key.startswith('vfx/')}
        for recipe in sorted(recipes):
            resource = f'vfx/{recipe}/A/contact/0'
            if pack.has(resource, None):
                image, trace = pack.image(resource, None)
                name = f'{age}/vfx-{recipe}.png'
                image.save(out / name)
                entry['spells'][recipe] = {'file': name, 'asset': f'DERIVED stand-in from {trace}'}
        data['ages'][age] = entry
    for entity, record in aux_index.items():
        frame = next((f for f in record['frames'] if f['state'] == 'idle' and f['face'] == 'right'), record['frames'][0])
        sheet = Image.open(__import__('io').BytesIO(aux.read(record['path']))).convert('RGBA')
        x, y, w, h = frame['rect_xywh']
        name = f'aux-{entity}.png'
        sheet.crop((x, y, x + w, y + h)).save(out / name)
        data.setdefault('auxiliary', {})[entity] = {
            'file': name, 'asset': f"D05 auxiliary study {record['path'].split('/')[-1]} ({record['coverage']})"}
    (out / 'assets.json').write_text(json.dumps(data, indent=1) + '\n')
    return data


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packs', type=Path, required=True)
    parser.add_argument('--vfx-zip', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    data = export(args.packs, args.vfx_zip, args.out)
    print(json.dumps({age: len(entry['units']) for age, entry in data['ages'].items()}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
