#!/usr/bin/env python3
"""Conservative font-advance layout examples, not native/browser pixel QA.

Pillow is used only for local validation. Text payloads are read from the actual
Rust presenter; checked sizes/line heights come from tokens.toml and the same
bounded bundled OpenSans fonts as the host. Advances are summed without kerning
and include semantic tracking, then scalar-wrapped as renderer-macroquad does.
"""
from pathlib import Path
import json
import re
import tomllib
from PIL import ImageFont

ROOT = Path(__file__).resolve().parents[2]
TOKENS = tomllib.loads((ROOT/'tokens.toml').read_text())['sys']['type']
MOD = (ROOT/'games/werewolf/src/presentation/mod.rs').read_text()
RENDER = (ROOT/'games/werewolf/src/presentation/render.rs').read_text()


def width(text, token):
    kind, size = token.split('.')
    style = TOKENS[kind][size]
    filename = 'OpenSans-Semibold.ttf' if style['weight'] >= 600 else 'OpenSans-Regular.ttf'
    font = ImageFont.truetype(str(ROOT/'assets/fonts'/filename), round(style['size']))
    return sum(font.getlength(char) for char in text) + max(0, len(text)-1)*style['letter-spacing']


def line_count(text, limit, token):
    lines = 0
    for paragraph in text.split('\n'):
        current = ''
        for char in paragraph:
            if current and width(current+char,token) > limit:
                lines += 1
                current = ''
            current += char
        lines += bool(current) or not paragraph
    return lines


def main():
    roles=['Villager','Werewolf','Seer','Doctor','Hunter','Witch']
    full = MOD[MOD.index('fn role_rules('):MOD.index('fn perspective_label(')]
    short = RENDER[RENDER.index('let short = match scope.role'):RENDER.index('let short = match scope.role')+800]
    payloads = lambda source: dict(re.findall(r'Role::(\w+)\s*=>\s*"([^"]+)"',source))
    full,short=payloads(full),payloads(short)
    checks=[]
    for card_width in [148.0,222.67,286.67,320.0]:
        for role in roles:
            value=short[role] if card_width<260 else full[role]
            count=line_count(value,card_width-28,'body.sm')
            body_top=card_width+2+(26 if card_width<220 else 46)
            end=body_top+count*TOKENS['body']['sm']['line-height']
            assert end<=card_width*1.5, (card_width,role,count,end)
            checks.append({'kind':'card_body','width':card_width,'role':role,'lines':count,'end':round(end,2),'card_end':round(card_width*1.5,2)})
    # Target controls use stacked numeral + one explicit status label. Both are
    # independently single-line at the narrowest 320px four-column grid.
    minimum_cell=(320-2*9.6-3*8)/4
    for status in ['Sống','Chết','Chọn']:
        assert width(status,'label.md')<=minimum_cell-16
        checks.append({'kind':'target_label','label':status,'width':round(width(status,'label.md'),2),'limit':round(minimum_cell-16,2)})
    navigation=(320-2*9.6-2*8)/3
    for label in ['Ghế trước','Ghế sau','Công khai']:
        assert width(label,'label.lg')<=navigation-16
        checks.append({'kind':'navigation_label','label':label,'width':round(width(label,'label.lg'),2),'limit':round(navigation-16,2)})
    print(json.dumps({'evidence':'headless conservative font-advance examples; not rendered pixels','checks':checks},ensure_ascii=False,indent=2))

if __name__=='__main__':
    main()
