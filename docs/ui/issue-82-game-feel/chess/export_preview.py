"""Deterministic vector references, not browser screenshots. Shares model-data.json."""
from pathlib import Path
from html import escape
import json, re, subprocess
from PIL import Image

ROOT=Path(__file__).resolve().parent
MODEL=json.loads((ROOT/'model-data.json').read_text())
SYMBOLS=re.sub(r'^<svg[^>]*>|</svg>\s*$', '',(ROOT/'pieces.svg').read_text().strip())
FILES='abcdefgh'

def tag_text(x,y,value,size=12,fill='#b8a4c8',weight='400',anchor='start',family='DejaVu Sans',spacing=None):
    sp=f' letter-spacing="{spacing}"' if spacing else ''
    return f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" fill="{fill}" font-weight="{weight}" text-anchor="{anchor}"{sp}>{escape(str(value))}</text>'
def rect(x,y,w,h,fill,rx=0,stroke=None,sw=1):
    s=f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ''
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}"{s}/>'
def piece(code,x,y,size):
    return f'<ellipse cx="{x+size*.5}" cy="{y+size*.88}" rx="{size*.3}" ry="{size*.058}" fill="#1a112a" opacity=".3"/><use xlink:href="#{code}" x="{x}" y="{y}" width="{size}" height="{size}"/>'
def position_at(step):
    p=dict(MODEL['initial']);captures=[]
    for origin,target in MODEL['sequence'][:step]:
        if target in p:captures.append(p[target])
        p[target]=p.pop(origin)
    return p,captures

def player(x,y,width,name,side,active,mobile=False):
    color='#d3c3b0' if side=='white' else '#28363b'; ink='#433743' if side=='white' else '#e3cca3'
    s=rect(x,y,40,40,color,13,'#a68b5b88')+tag_text(x+20,y+28,name[0],22,ink,anchor='middle',family='DejaVu Serif')
    s+=tag_text(x+52,y+15,name,14,'#eddfef')+tag_text(x+52,y+34,f'Quân {"trắng" if side=="white" else "đen"} · dữ liệu mẫu',12,'#a997b5')
    text=f'Lượt {"trắng" if side=="white" else "đen"} · mẫu' if active else 'Đang quan sát'
    if active:s+=rect(x+width-129,y+5,129,30,'#332541',9,'#70508055')
    s+=tag_text(x+width-10,y+25,text,12,'#ddc7ff' if active else '#8f7e9e',anchor='end')
    return s

def board(x,y,size,position,orientation='white',last=None,selected=None,mobile=False):
    pad=20 if mobile else 27;sq=(size-pad*2)/8; out=''
    out+=rect(x,y,size,size,'url(#wood)',11,'#a98a6977')
    out+=rect(x+4,y+4,size-8,size-8,'none',8,'#251f2f',4)
    out+=rect(x+10,y+10,size-20,size-20,'none',3,'#b08c6b44')
    out+=rect(x+pad-2,y+pad-2,size-pad*2+4,size-pad*2+4,'#211a28',0,'#c0a08044')
    ff=list(FILES) if orientation=='white' else list(reversed(FILES));rr=list(range(8,0,-1)) if orientation=='white' else list(range(1,9))
    for r,rank in enumerate(rr):
        out+=tag_text(x+pad*.48,y+pad+(r+.5)*sq+4,rank,12,'#d0bfae','600','middle')
        for c,file in enumerate(ff):
            square=f'{file}{rank}';sx=x+pad+c*sq;sy=y+pad+r*sq
            color='#d0bdad' if (FILES.index(file)+rank)%2==0 else '#665368'
            out+=rect(sx,sy,sq,sq,color)+rect(sx,sy,sq,sq,'url(#grain)')
            if last and square in last:out+=rect(sx+2,sy+2,sq-4,sq-4,'#c0a0f020',0,'#d1b5ff88',1.5)
            if square==selected:out+=rect(sx+2,sy+2,sq-4,sq-4,'#ae84dd22',0,'#c6a1f4',2.5)
            if selected=='e4' and square=='d5':out+=f'<circle cx="{sx+sq/2}" cy="{sy+sq/2}" r="{sq*.42}" fill="none" stroke="#d8b9ff" stroke-width="2.5"/>'
            if square in position:
                ps=sq*(.91 if mobile else .83);out+=piece(position[square],sx+(sq-ps)/2,sy+(sq-ps)/2,ps)
    for c,file in enumerate(ff):out+=tag_text(x+pad+(c+.5)*sq,y+size-pad*.28,file,12,'#d0bfae','600','middle')
    for cx,cy in [(x+9,y+9),(x+size-9,y+size-9)]:out+=f'<rect x="{cx-3}" y="{cy-3}" width="6" height="6" fill="#785a45" stroke="#bc9d68" transform="rotate(45 {cx} {cy})"/>'
    return out

def turn_panel(x,y,w,step=2,mobile=False,promotion=False):
    h=258 if mobile else 285
    s=rect(x,y,w,h,'url(#panel)',20,'#ccb0f526')
    s+=f'<circle cx="{x+25}" cy="{y+27}" r="6" fill="#ead9bb" stroke="#aa916d"/>'
    s+=tag_text(x+39,y+31,'LƯỢT TRẮNG · MẪU',12,'#c7abed',spacing='.7')+tag_text(x+w-22,y+31,'RIÊNG' if promotion else '02 / 04',12,'#8e789f',anchor='end')
    s+=tag_text(x+22,y+72,'Một quân cờ lớn lên.' if promotion else 'Một tốt mở lối.',24,'#eadcf4',family='DejaVu Serif')
    lines=['Tốt trắng a7 → a8. Chọn một','trong bốn quân mẫu.'] if promotion else ['e4 → d5: quân tốt trắng tiến chéo,','quân bị bắt rời bàn.']
    for i,line in enumerate(lines):s+=tag_text(x+22,y+103+i*23,line,13,'#b9a6c9')
    for ix,label in enumerate(['NHẤC','LƯỚT','ĐẶT']):
        tx=x+22+ix*(w-44)/3;s+=tag_text(tx,y+161,label,12,'#c7ae8d',spacing='1.2')
        if ix<2:s+=f'<path d="M{tx+52} {y+157}h{(w-44)/3-64}" stroke="#b5945b55"/>'
    s+=rect(x+21,y+181,w-42,47,'#cbb5ff',13)+tag_text(x+35,y+210,'Mở lựa chọn phong cấp mẫu' if promotion else 'Xem nước mẫu e4 → d5',12,'#352146')+tag_text(x+w-35,y+212,'↗',22,'#352146',anchor='end')
    if not mobile:s+=tag_text(x+22,y+253,'Chuỗi 4 nước cố định',12,'#927da2')+tag_text(x+22,y+272,'Không kiểm luật đầy đủ',12,'#927da2')
    else:s+=tag_text(x+22,y+248,'Chuỗi 4 nước cố định · dữ liệu mẫu',12,'#927da2')
    return s

def history_panel(x,y,w,promotion=False):
    s=rect(x,y,w,207,'#20192a',20,'#ccb0f51b')+tag_text(x+22,y+30,'Dấu vết trên bàn',14,'#decee9')
    s+=tag_text(x+w-22,y+30,'TỌA ĐỘ MẪU',12,'#8a7799',anchor='end')
    for tx,label in [(x+22,'NƯỚC'),(x+70,'TRẮNG'),(x+183,'ĐEN')]:s+=tag_text(tx,y+71,label,12,'#897395')
    s+=tag_text(x+22,y+103,'—' if promotion else '1.',12,'#7f6b8d')
    s+=tag_text(x+70,y+103,'Tình huống riêng' if promotion else 'e2 → e4',12,'#c2afcf')
    if not promotion:s+=rect(x+174,y+84,96,29,'#c8a6ef14',7)+tag_text(x+183,y+103,'d7 → d5',12,'#e0caff')
    s+=f'<path d="M{x+22} {y+125}h{w-44}" stroke="#cfb7e91c"/>'
    s+=tag_text(x+22,y+148,'QUÂN BỊ BẮT · MẪU',12,'#9581a7',spacing='.6')+tag_text(x+22,y+183,'Chưa có',12,'#725f83')
    return s

def export(name,width=1440,height=960,orientation='white',promotion=False):
    mobile=width<761
    defs=f'<defs>{SYMBOLS}<linearGradient id="wood" x2="1" y2="1"><stop stop-color="#604543"/><stop offset=".45" stop-color="#302a33"/><stop offset="1" stop-color="#634845"/></linearGradient><linearGradient id="panel" x2="1" y2="1"><stop stop-color="#302338"/><stop offset="1" stop-color="#241c2e"/></linearGradient><radialGradient id="glow"><stop stop-color="#624361" stop-opacity=".27"/><stop offset="1" stop-color="#15111e" stop-opacity="0"/></radialGradient><pattern id="grain" patternUnits="userSpaceOnUse" width="26" height="26"><path d="M-5 26L5 0M10 26L20 0M25 26L35 0" stroke="#32243b" stroke-opacity=".035" stroke-width="1"/></pattern><filter id="pieceShadow" x="-.2" y="-.2" width="1.4" height="1.5"><feDropShadow dx="0" dy="3" stdDeviation="1.4" flood-color="#211429" flood-opacity=".6"/></filter></defs>'
    s=rect(0,0,width,height,'#15111e')+rect(0,0,width,62 if mobile else 68,'#181320')
    s+=f'<path d="M0 {62 if mobile else 68}H{width}" stroke="#d3b8fb14"/>'
    bx=18 if mobile else 36;by=22 if mobile else 25
    for dx,dy,color in [(0,0,'#cbb5ff'),(14,0,'#9478c2'),(0,14,'#9478c2'),(14,14,'#cbb5ff')]:s+=rect(bx+dx,by+dy,11,11,color,3)
    s+=tag_text(bx+40,by+22,'tabula',25,'#ded0fb','600',spacing='-1.2')
    if not mobile:s+=tag_text(226,41,'Bàn chơi    /    Cờ vua',12,'#b39ec0')+tag_text(width-36,41,'●  Bản thiết kế · dữ liệu mẫu',12,'#b7a4ca',anchor='end')
    else:s+=tag_text(width-18,29,'Bản thiết kế',12,'#b7a4ca',anchor='end')+tag_text(width-18,46,'dữ liệu mẫu',12,'#b7a4ca',anchor='end')
    hx=18 if mobile else 90;hy=89 if mobile else 96
    s+=tag_text(hx,hy,'CỜ VUA · BÀN GỖ TRẦM',12,'#cfad77','600',spacing='1.1')
    s+=tag_text(hx,hy+37,'Một nước. Một câu chuyện.',25 if mobile else 31,'#f1eaf7',family='DejaVu Serif')
    s+=tag_text(hx,hy+62,'Bàn cờ yên tĩnh. Quân cờ có trọng lượng.',12,'#ac9ab8')
    if mobile:s+=rect(18,hy+76,177,25,'#261e32',8)+tag_text(27,hy+93,'Không đồng hồ · mẫu',12,'#bdacd0')
    else:s+=rect(width-270,105,186,36,'#261e32',12)+tag_text(width-177,128,'Không đồng hồ · mẫu',12,'#bdacd0',anchor='middle')
    if mobile:x,y,size=15,266,360;player_y=210;side_x,side_y,side_w=12,746,366
    else:x,y,size=265,241,548;player_y=176;side_x,side_y,side_w=1030,176,330
    # Quiet concentric rings form a deterministic atmospheric frame.
    s+=f'<ellipse cx="{x+size/2}" cy="{y+size/2}" rx="{size*.8}" ry="{size*.68}" fill="url(#glow)"/>'
    for radius in range(5):s+=f'<ellipse cx="{x+size/2}" cy="{y+size/2}" rx="{size*.5+radius*42}" ry="{size*.46+radius*32}" fill="none" stroke="#b098b8" stroke-opacity=".035"/>'
    s+=rect(x+4,y+11,size-8,size,'#0c08129c',10)+rect(x,y+5,size,size,'#221c25',11)
    pos=dict(MODEL['promotion']['position']) if promotion else position_at(2)[0]
    top='black' if orientation=='white' else 'white';bottom=orientation
    s+=player(x,player_y,size,MODEL['players'][top],top,top=='white',mobile)
    s+=board(x,y,size,pos,orientation,None if promotion else MODEL['sequence'][1],MODEL['promotion']['from'] if promotion else 'e4',mobile)
    s+=player(x,y+size+19,size,MODEL['players'][bottom],bottom,bottom=='white',mobile)
    capy=y+size+86;s+=f'<circle cx="{x+3}" cy="{capy-4}" r="2.5" fill="#ad8bde"/>'
    s+=tag_text(x+13,capy,'Chạm vào quân để xem gợi ý mẫu',12,'#a595b2')+tag_text(x+size,capy,f'Hướng {"trắng" if orientation=="white" else "đen"}',12,'#816f91',anchor='end')
    s+=turn_panel(side_x,side_y,side_w,mobile=mobile,promotion=promotion)
    if not mobile:
        s+=history_panel(side_x,side_y+302,side_w,promotion)
        s+=rect(side_x,side_y+526,side_w,184,'#21192a',20,'#ccb0f51b')+tag_text(side_x+22,side_y+556,'CHUYỂN ĐỘNG CÓ CHỦ ĐÍCH',12,'#b39bc7',spacing='.5')
        for i,line in enumerate(['Quân cờ nhấc nhẹ, lướt một','vòng cung rồi đặt xuống. Ô','vừa đi giữ lại dấu sáng dịu.']):s+=tag_text(side_x+22,side_y+584+i*22,line,13,'#b49fc4')
        s+=tag_text(side_x+22,side_y+678,'Gợi ý chỉ cho nước kế tiếp mẫu.',12,'#8f7a9e')
        s+=tag_text(90,height-29,'Bản thiết kế • dữ liệu mẫu • mô phỏng cục bộ',12,'#84718f')+tag_text(width-83,height-29,'Công cụ xem mẫu  ⌄',12,'#b19bc5',anchor='end')
    else:
        s+=tag_text(16,height-27,'Bản thiết kế • dữ liệu mẫu',12,'#84718f')+tag_text(width-16,height-27,'Công cụ mẫu  ⌄',12,'#b19bc5',anchor='end')
    if promotion:
        s+=rect(0,68,width,height-68,'#0d071dc9')
        mw=470;mx=(width-mw)/2;my=300
        s+=rect(mx,my,mw,306,'#2b2037',24,'#b592e966')
        s+=tag_text(mx+27,my+32,'TÌNH HUỐNG PHONG CẤP · MẪU',12,'#cfad77',spacing='1')+tag_text(mx+27,my+74,'Chọn quân tiếp theo',27,'#eadff3',family='DejaVu Serif')+tag_text(mx+27,my+103,'Tốt trắng a7 → a8. Đây là tình huống riêng.',12,'#bca6cc')
        for i,kind in enumerate(MODEL['promotion']['choices']):
            px=mx+27+i*107;s+=rect(px,my+127,95,104,'#3c2d48',13,'#8c70a36b')+piece('w'+kind,px+17,my+135,60)+tag_text(px+47.5,my+219,MODEL['pieceNames'][kind],12,'#dfcef0',anchor='middle')
        s+=rect(mx+27,my+248,mw-54,39,'#cbb5ff',12)+tag_text(mx+mw/2,my+273,'Đóng tình huống mẫu',12,'#342046',anchor='middle')
    output=ROOT/(name+'.svg');svg=f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="0 0 {width} {height}">{defs}{s}</svg>'
    svg=re.sub(r' (fill|stroke)="(#[0-9a-fA-F]{8})"',lambda m:f' {m[1]}="{m[2][:7]}" {m[1]}-opacity="{int(m[2][7:9],16)/255:.4f}"',svg)
    output.write_text(svg)
    subprocess.run(['inkscape',str(output),'--export-type=png',f'--export-filename={ROOT/(name+".png")}'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    with Image.open(ROOT/(name+'.png')) as image:image.convert('RGB').save(ROOT/(name+'.jpg'),quality=91,optimize=True)
    print(f'{name}: {width}×{height}')

if __name__=='__main__':
    export('desktop')
    export('mobile',390,1050)
    export('black-orientation',orientation='black')
    export('promotion',promotion=True)
