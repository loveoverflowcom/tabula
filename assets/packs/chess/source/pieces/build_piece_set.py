from pathlib import Path
import json, html

HERE = Path(__file__).resolve().parent
PALETTES = {
    'w': {'name':'Ivory', 'fill':'#F7F1E4', 'edge':'#273638', 'accent':'#B58D42', 'facet':'#FFFFFF', 'facet_opacity':'.66'},
    'b': {'name':'Ink', 'fill':'#243E42', 'edge':'#C0A671', 'accent':'#C0A671', 'facet':'#5D8A80', 'facet_opacity':'.48'},
}
BASE = 'M28 71 H68 Q68 75 71 77 Q76 81 77 86 H19 Q20 81 25 77 Q28 75 28 71 Z'
SHAPES = {
    'K': {
        'name':'King',
        'body':'M43 7 H53 V16 H62 V26 H53 V34 H61 Q63 37 60 42 L57 48 Q55 59 66 70 L65 73 H31 L30 70 Q41 59 39 48 L36 42 Q33 37 35 34 H43 V26 H34 V16 H43 Z',
        'facet':'M43 40 H48 V48 Q48 58 39 68 H34 Q44 58 43 48 Z',
        'detail':'<path d="M38 38 H58" stroke="{accent}" stroke-width="3"/><path d="M39 48 H57" stroke="{edge}" stroke-width="2.4"/>',
    },
    'Q': {
        'name':'Queen',
        'body':'M28 44 L19 20 L33 31 L35 17 L43 31 L48 12 L53 31 L61 17 L63 31 L77 20 L68 44 Q66 49 56 51 Q56 61 66 70 L65 73 H31 L30 70 Q40 61 40 51 Q30 49 28 44 Z',
        'facet':'M32 44 H42 L43 50 Q43 60 36 68 H33 Q42 59 40 50 Z',
        'detail':'<path d="M29 44 H67" stroke="{accent}" stroke-width="3"/><path d="M40 52 H56" stroke="{edge}" stroke-width="2.4"/>',
        'finials':[(19,20,3.4),(35,17,3.4),(48,12,3.4),(61,17,3.4),(77,20,3.4)],
    },
    'B': {
        'name':'Bishop',
        'body':'M48 11 C43 18 30 23 30 34 C30 43 36 48 42 51 Q42 60 32 70 L31 73 H65 L64 70 Q54 60 54 51 C60 48 66 43 66 34 C66 23 53 18 48 11 Z',
        'facet':'M44 22 Q34 28 35 35 Q36 43 44 47 L43 52 Q43 60 36 68 H33 Q43 59 42 51 Q32 44 32 34 Q33 27 44 22 Z',
        'detail':'<path d="M54 23 L44 36" stroke="{edge}" stroke-width="4.5"/><path d="M40 52 H56" stroke="{accent}" stroke-width="3"/>',
    },
    'N': {
        'name':'Knight',
        'body':'M30 71 Q32 62 40 55 L42 43 L29 47 L19 41 L23 31 L36 23 L39 11 L49 19 Q58 18 64 27 Q72 39 68 55 L64 71 L64 73 H31 Z',
        'facet':'M49 24 Q58 24 61 33 Q65 44 61 56 L57 68 H50 Q58 53 58 42 Q57 31 49 24 Z',
        'detail':'<circle cx="40" cy="30" r="2.7" fill="{edge}" stroke="none"/><path d="M24 38 L29 40" stroke="{edge}" stroke-width="2.8"/><path d="M42 43 L49 40" stroke="{accent}" stroke-width="2.8"/>',
    },
    'R': {
        'name':'Rook',
        'body':'M25 15 H36 V25 H43 V15 H53 V25 H60 V15 H71 V36 L64 42 H61 L64 70 L65 73 H31 L32 70 L35 42 H32 L25 36 Z',
        'facet':'M36 43 H43 L42 65 L38 69 H35 Z',
        'detail':'<path d="M29 35 H67" stroke="{accent}" stroke-width="3"/><path d="M35 43 H61" stroke="{edge}" stroke-width="2.4"/>',
    },
    'P': {
        'name':'Pawn',
        'body':'M42 39 H54 V47 Q54 60 65 70 L64 73 H32 L31 70 Q42 60 42 47 Z',
        'facet':'M42 48 H47 Q47 59 39 68 H34 Q42 59 42 48 Z',
        'detail':'<path d="M38 45 H58" stroke="{accent}" stroke-width="3"/>',
        'orb':(48,27,13),
    },
}

manifest = {'name':'Tabula Carved Staunton', 'version':'1.0', 'viewBox':'0 0 96 96', 'intendedSize':'36–64px', 'palette':PALETTES, 'pieces':[]}
symbols=[]
for color, p in PALETTES.items():
    for kind, s in SHAPES.items():
        key=color+kind
        title=f"{p['name']} {s['name'].lower()}"
        group=[f'<g fill="{p["fill"]}" stroke="{p["edge"]}" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round">',f'<path d="{s["body"]}"/>',f'<path d="{s["facet"]}" fill="{p["facet"]}" fill-opacity="{p["facet_opacity"]}" stroke="none"/>']
        if 'orb' in s:
            x,y,r=s['orb']
            group += [f'<circle cx="{x}" cy="{y}" r="{r}"/>', f'<path d="M45 18 Q39 20 38 26" fill="none" stroke="{p["facet"]}" stroke-opacity="{p["facet_opacity"]}" stroke-width="3.4"/>']
        if 'finials' in s:
            for x,y,r in s['finials']:
                group += [f'<circle cx="{x}" cy="{y}" r="{r}"/>']
        group += [s['detail'].format(**p),f'<path d="{BASE}"/>',f'<path d="M29 72 H67" fill="none" stroke="{p["accent"]}" stroke-width="3"/>',f'<path d="M25 82 H71" fill="none" stroke="{p["edge"]}" stroke-opacity=".46" stroke-width="2"/>','</g>']
        body='\n  '.join(group)
        svg=f'''<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96" role="img" aria-labelledby="title desc">
  <title id="title">{title}</title>
  <desc id="desc">Original carved Staunton-style {s['name'].lower()} for Tabula, with {p['name'].lower()} body and warm brass detailing.</desc>
  {body}
</svg>\n'''
        (HERE/f'{key}.svg').write_text(svg)
        symbols.append(f'<symbol id="{key}" viewBox="0 0 96 96">\n  {body}\n</symbol>')
        manifest['pieces'].append({'id':key,'color':color,'kind':kind,'name':title,'file':f'{key}.svg'})
(HERE/'pieces.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg">\n'+'\n'.join(symbols)+'\n</svg>\n')
(HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')

# A self-contained review page: inline assets survive export and work offline.
cards=[]
for color,p in PALETTES.items():
    for kind,s in SHAPES.items():
        key=color+kind
        raw=(HERE/f'{key}.svg').read_text()
        small=raw.replace('width="96" height="96"','width="36" height="36"')
        cards.append(f'<article><div class="large">{raw}</div><div class="sizes"><span class="bone">{small}</span><span class="teal">{small}</span><span class="selected">{small}</span></div><div class="label">{html.escape(p["name"])} {s["name"].lower()}<small>{key}.svg</small></div></article>')
page='''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Tabula · Carved Staunton pieces</title><style>
*{box-sizing:border-box}body{margin:0;background:#112a2e;color:#f7f1e4;font-family:Arial,sans-serif;padding:48px}header{display:flex;align-items:end;justify-content:space-between;margin:0 auto 32px;max-width:1104px}h1{font:500 32px Georgia,serif;margin:0 0 8px}header p{color:#c5cbbb;margin:0;font-size:13px}header .tag{font-size:11px;letter-spacing:.18em;text-transform:uppercase;color:#c0a671}main{display:grid;grid-template-columns:repeat(6,1fr);gap:16px;max-width:1104px;margin:auto}article{border:1px solid #3a5353;background:#193439;border-radius:10px;overflow:hidden}.large{display:flex;align-items:center;justify-content:center;height:146px;background:#ece4d3}.large svg{width:110px;height:110px}.sizes{display:flex;justify-content:center;gap:8px;padding:17px 10px}.sizes span{display:flex;width:42px;height:42px;align-items:center;justify-content:center;border-radius:3px}.bone{background:#ece4d3}.teal{background:#5d8a80}.selected{background:#d5ab54}.label{padding:0 16px 17px;text-align:center;font-size:13px}.label small{display:block;margin-top:6px;font-size:10px;color:#aabdb5;letter-spacing:.08em}footer{max-width:1104px;margin:26px auto 0;font-size:12px;line-height:1.7;color:#aabdb5} @media(max-width:900px){body{padding:24px}main{grid-template-columns:repeat(3,1fr)}header{display:block}.tag{display:block;margin-top:16px}}
</style></head><body><header><div><h1>Carved Staunton</h1><p>Ivory and midnight ink · Original vector chessmen for Tabula</p></div><span class="tag">96 × 96 / checked at 36 px</span></header><main>'''+''.join(cards)+'''</main><footer>Each piece shown at display scale, then 36 px on bone, teal and selected-square brass. Solid carved contours, minimal warm-metal accents, transparent backgrounds. Original artwork authored as SVG paths.</footer></body></html>'''
(HERE/'piece-sheet.html').write_text(page)

(HERE/'PROVENANCE.md').write_text('''# Tabula Carved Staunton · Piece provenance

This is an original, code-native SVG set designed specifically for the Tabula chess design study on 2026-10-04. All silhouettes and detailing were hand-authored as SVG path and circle geometry for this task. There are no stock icons, Unicode chess symbols, emoji, font glyphs, copied SVGs, external imagery or external dependencies.

## Design

- Six conventional Staunton subjects: king, queen, bishop, knight, rook and pawn
- Two matching palettes: ivory #F7F1E4 with dark #273638 contour; midnight ink #243E42 with warm #C0A671 rim
- A shared carved foot, consistent 2.8-unit outline and restrained brass collar treatment
- Transparent 96 × 96 viewBox; intended board rendering at 36–64 CSS pixels
- All pieces face front except the left-facing horse-profile knight
- White and black variants share precisely the same geometry

## Files

- wK.svg, wQ.svg, wB.svg, wN.svg, wR.svg, wP.svg and corresponding b-prefixed black variants
- pieces.svg: optional reusable SVG symbols, referenced by #wK, #bK, etc.
- manifest.json: asset names, palettes and metadata
- piece-sheet.html: self-contained visual review sheet at large and 36 px scale on all three board states
- piece-sheet.png: static review sheet rendered from the actual SVG files
- build_piece_set.py: editable geometry and deterministic generation source
- render_sheet.py: reproducible static review sheet using Inkscape and Pillow

## Use

These assets are supplied for the requested design handoff. No production repository or prototype was changed by this asset task. All SVGs use standard paths and circles and are safe to embed as image sources. The SVG sprite is optional; the standalone files are the canonical assets.
''')
print('Created 12 standalone SVG pieces, sprite, manifest, review page and provenance')
