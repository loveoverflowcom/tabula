#!/usr/bin/env python3
"""Standalone design references, not browser/runtime screenshots.
Layout, identity portraits and assets mirror index.html; render with Inkscape.
"""
from pathlib import Path
from io import BytesIO
import base64, json, math, html, subprocess, urllib.parse, re
from PIL import Image, ImageFont

HERE=Path(__file__).resolve().parent
DATA=json.loads((HERE/'portrait-data.json').read_text())
NAMES=DATA['names']
PORTRAITS=['data:image/svg+xml;base64,'+base64.b64encode(urllib.parse.unquote(p.split(',',1)[1]).encode()).decode() for p in DATA['portraits']]
scene=Image.open(HERE/'assets/village.webp').convert('RGB');buf=BytesIO();scene.save(buf,format='JPEG',quality=82,optimize=True)
SCENE='data:image/jpeg;base64,'+base64.b64encode(buf.getvalue()).decode()
dawn=Image.open(HERE/'assets/village-dawn.webp').convert('RGB');buf=BytesIO();dawn.save(buf,format='JPEG',quality=78,optimize=True)
DAWN='data:image/jpeg;base64,'+base64.b64encode(buf.getvalue()).decode()
WOLF='data:image/png;base64,'+base64.b64encode((HERE/'assets/werewolf.png').read_bytes()).decode()
PURPLE='#cbb5ff';GOLD='#d6b780';MUTED='#a99bb9'

def esc(s):return html.escape(str(s))
class SVG:
 def __init__(self,w,h):
  self.w,self.h=w,h
  self.s=[f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}">', '''<defs><linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#100d24" stop-opacity=".28"/><stop offset=".45" stop-color="#0c0b19" stop-opacity=".02"/><stop offset=".85" stop-color="#0b0a17" stop-opacity=".45"/><stop offset="1" stop-color="#0c0a19" stop-opacity=".92"/></linearGradient><linearGradient id="mobileShade" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#151126" stop-opacity=".06"/><stop offset=".28" stop-color="#171424" stop-opacity=".1"/><stop offset=".6" stop-color="#171424" stop-opacity=".95"/><stop offset="1" stop-color="#171424"/></linearGradient><radialGradient id="cardBack"><stop stop-color="#3a314f"/><stop offset="1" stop-color="#1d1829"/></radialGradient><linearGradient id="roleFade" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#191421" stop-opacity="0"/><stop offset=".4" stop-color="#191421" stop-opacity=".9"/><stop offset="1" stop-color="#191421"/></linearGradient></defs>''']
 def add(self,x):self.s.append(x)
 def rect(self,x,y,w,h,fill,rx=0,stroke=None,sw=1):self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+'/>')
 def text(self,x,y,text,size=12,color='#f1ebf9',anchor='start',family='DejaVu Sans',weight='normal',spacing=None):
  self.add(f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" fill="{color}" text-anchor="{anchor}"'+(f' letter-spacing="{spacing}"' if spacing is not None else '')+f'>{esc(text)}</text>')
 def circle(self,x,y,r,fill,stroke=None,sw=1):self.add(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+'/>')
 def line(self,x,y,x2,y2,color,sw=1):self.add(f'<path d="M{x} {y}L{x2} {y2}" fill="none" stroke="{color}" stroke-width="{sw}"/>')
 def img(self,src,x,y,w,h,clip=None):self.add(f'<image x="{x}" y="{y}" width="{w}" height="{h}" preserveAspectRatio="xMidYMid slice" xlink:href="{src}"'+(f' clip-path="url(#{clip})"' if clip else '')+'/>')
 def icon(self,name,x,y,size=20,color=PURPLE):
  paths={'moon':'<path d="M20.4 14.8A9 9 0 0 1 9.2 3.6 9 9 0 1 0 20.4 14.8Z"/>','sun':'<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>','leaf':'<path d="M20 3c1 12-4 19-11 17C0 17 4 5 20 3Z M6 21 17 7"/>','eye':'<path d="M2 12s3.8-7 10-7 10 7 10 7-3.8 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>','lock':'<rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V6a4 4 0 0 1 8 0v4m4 4v3"/>','chevron':'<path d="m9 5 7 7-7 7"/>'}
  self.add(f'<g transform="translate({x} {y}) scale({size/24})" fill="none" stroke="{color}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{paths.get(name,paths["moon"])}</g>')
 def finish(self):
  value=''.join(self.s)+'</svg>'
  return re.sub(r'(fill|stroke)="(#[0-9a-fA-F]{6})([0-9a-fA-F]{2})"',lambda m:f'{m[1]}="{m[2]}" {m[1]}-opacity="{int(m[3],16)/255:.4f}"',value)

def header(s,mobile):
 s.rect(0,0,s.w,63 if mobile else 76,'#13101d');s.line(0,62 if mobile else 75,s.w,62 if mobile else 75,'#2b2536')
 x=18 if mobile else 36;y=22 if mobile else 26
 for i in range(4):s.rect(x+(i%2)*13,y+(i//2)*13,10,10,'#c6acff' if i in [0,3] else '#9678cf',3)
 s.text(x+35,y+21,'tabula',22 if mobile else 24,'#e1d5ff',weight='bold',spacing=-1.3)
 if not mobile:s.text(190,45,'Bàn chơi   /   Ma Sói',12,'#ad9ebc')
 s.circle(s.w-(152 if mobile else 307),31 if mobile else 38,3,'#b89be8');s.text(s.w-(142 if mobile else 296),34 if mobile else 42,'Mô phỏng cục bộ',8 if mobile else 11,'#ac9fc1')
 if not mobile:s.icon('moon',s.w-166,30,17);s.text(s.w-140,42,'Chuyển động',11,'#bcb0cd')
 else:s.icon('moon',s.w-34,23,18)

def phase_label(phase):return {'night':('ĐÊM THỨ 02','01:24','Làng đang ngủ','Chọn mục tiêu trong vùng ánh trăng.','moon'),'day':('NGÀY THỨ 02','02:48','Bình minh gọi làng thức','Lắng nghe. Quan sát. Cùng tìm manh mối.','sun'),'vote':('BIỂU QUYẾT 02','00:42','Ai đang giữ bí mật?','Lá phiếu công khai. Vai trò vẫn được che.','leaf')}[phase]

def title(s,mobile,phase):
 label,timer,*_=phase_label(phase)
 heading={'night':'Đêm ở làng Sương','day':'Làng Sương thức giấc','vote':'Lá phiếu dưới ánh trăng'}[phase]
 intro={'night':'Mọi người nhắm mắt. Những bí mật bắt đầu thức giấc.','day':'Làng thức giấc. Mỗi ánh mắt đều giấu một câu chuyện.','vote':'Hãy chọn người bạn nghi ngờ. Minh họa lá phiếu công khai.'}[phase]
 if mobile:
  s.text(17,93,'CLASSIC · 12 NGƯỜI',8,GOLD,spacing=1.4);s.text(17,125,heading,24,family='DejaVu Serif');s.text(17,149,'Mọi người nhắm mắt. Những bí mật',9,MUTED);s.text(17,166,'bắt đầu thức giấc.',9,MUTED)
  s.rect(287,86,86,84,'#211b2c',15,stroke='#cfb5ff20');s.text(298,105,label,6,'#cdbde1',spacing=.8);s.text(298,134,timer,19);s.text(298,151,'còn lại · mẫu',6,'#9587a4');s.rect(298,160,64,2,'#483150',1);s.rect(298,160,38,2,PURPLE,1)
 else:
  s.text(36,105,'CLASSIC · 12 NGƯỜI',10,GOLD,spacing=2);s.text(36,141,heading,31,family='DejaVu Serif');s.text(36,164,intro,11,MUTED)
  s.rect(s.w-286,100,250,72,'#252032',20,stroke='#cfb5ff18');s.icon(phase_label(phase)[4],s.w-264,121,29,'#d8c3ff');s.text(s.w-219,120,label,10,'#cdbde1',spacing=1.5);s.text(s.w-219,148,timer,25);s.text(s.w-117,148,'còn lại · mẫu',9,'#9587a4');s.rect(s.w-219,157,169,2,'#483150',1);s.rect(s.w-219,157,101,2,PURPLE,1)

def portrait(s,i,cx,cy,mobile,selected=None,ballot=False):
 r=28.5 if mobile else 36;stroke='#dfcfff' if i==selected else '#dbc38b' if i==3 else '#b6a28a';pr=f'portrait{i}'
 if i==selected:s.circle(cx,cy,r+5,'#ba94ff28',PURPLE,1.2)
 s.circle(cx,cy+3,r+2,'#00000055');s.circle(cx,cy,r,'#2a2840',stroke,2 if not mobile else 1.5);s.add(f'<defs><clipPath id="{pr}"><circle cx="{cx}" cy="{cy}" r="{r-4}"/></clipPath></defs>');s.img(PORTRAITS[i],cx-r+4,cy-r+4,(r-4)*2,(r-4)*2,pr)
 bw,bh=(17,22) if mobile else (21,26);s.rect(cx+r-bw/2,cy-r-2,bw,bh,'#29243c',4,'#c7ad72',.8);s.icon('moon',cx+r-bw/2+3,cy-r+4,bw-6,'#c1a675')
 if i==3:s.rect(cx-17,cy+r-9,34,13 if mobile else 15,'#c6b085',5,'#24192e',1);s.text(cx,cy+r+(.5 if mobile else 2),'BẠN',6 if mobile else 8,'#24192e',anchor='middle',spacing=.8)
 if ballot:s.circle(cx-r,cy+r-5,11,'#dfc18f','#332943',2);s.text(cx-r,cy+r-2,'1',9,'#32222e','middle')
 y=cy+r+17 if not mobile else cy+r+14;s.rect(cx-(22 if mobile else 29),y-(10 if mobile else 12),44 if mobile else 58,17 if mobile else 21,'#141023d9',7)
 s.text(cx,y,NAMES[i],12,'#f2eafa','middle');s.text(cx,y+13 if mobile else y+16,f'GHẾ {i+1:02d}',9 if mobile else 10,'#cbbbac','middle',spacing=.7)

def arena(s,mobile,phase,selected):
 x,y,w,h=(12,186,366,531) if mobile else (28,184,1096,610);clip='arena';s.add(f'<defs><clipPath id="{clip}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{20 if mobile else 24}"/></clipPath></defs>');s.rect(x,y,w,h,'#171424',20 if mobile else 24,'#d0bdf330')
 if mobile:s.img(DAWN if phase=='day' else SCENE,x-75,y,516,290,clip);s.rect(x,y,w,h,'url(#mobileShade)',20)
 else:s.img(DAWN if phase=='day' else SCENE,x,y,w,h,clip);s.rect(x,y,w,h,'url(#shade)',24)
 s.icon('leaf',x+16,y+17,14,'#ead5b5');s.text(x+37,y+28,'LÀNG SƯƠNG',7 if mobile else 9,'#ead5b5',spacing=1.5);s.rect(x+w-(115 if mobile else 137),y+14,99 if mobile else 112,22 if mobile else 27,'#100e26bb',15,'#dfcdf026');s.text(x+w-25,y+(28 if mobile else 32),'12 người còn sống',10 if mobile else 11,'#d0c8d8','end')
 label,timer,center,copy,icon=phase_label(phase)
 if mobile:
  cx=x+w/2;cy=y+85;s.circle(cx,cy,17.5,'#292037aa','#d4bd8740');s.icon(icon,cx-10,cy-10,20,'#ddc390');s.text(cx,y+121,center,17,'#fff0d6','middle','DejaVu Serif');s.text(cx,y+140,copy,8,'#d7c7bd','middle')
  for i in range(12):portrait(s,i,x+45.75+(i%4)*91.5,y+204+(i//4)*103,mobile,selected,phase=='vote' and i==selected)
 else:
  s.add(f'<ellipse cx="{x+w*.5}" cy="{y+h*.52}" rx="{w*.35}" ry="{h*.35}" fill="#c8923910" stroke="#dbc39340" stroke-width="1"/><ellipse cx="{x+w*.5}" cy="{y+h*.52}" rx="{w*.35-12}" ry="{h*.35-12}" fill="none" stroke="#dbc39328" stroke-dasharray="3 5"/>')
  cx=x+w*.5;cy=y+h*.40;s.circle(cx,cy-29,22,'#292037aa','#d4bd8740');s.icon(icon,cx-12.5,cy-41.5,25,'#ddc390');s.text(cx,cy+17,center,21,'#fff0d6','middle','DejaVu Serif');s.text(cx,cy+39,copy,10,'#d7c7bd','middle')
  for i in range(12):
   theta=(-90+i*30)*math.pi/180;px=x+w*(.5+.36*math.cos(theta));py=y+h*(.52+.36*math.sin(theta))-21;portrait(s,i,px,py,False,selected,phase=='vote' and i==selected)
  for i in range(8):s.circle(cx-23+i*6,y+h*.57-2*(i%3),1.3,'#fbc17b')
 hint=f'Đã chọn {NAMES[selected]}' if selected is not None else 'Chạm vào chân dung để chọn mục tiêu';hint=(f'Đã chọn {NAMES[selected]} · ghế {selected+1:02d}' if selected is not None else hint)
 if phase=='day':hint='Thảo luận mẫu · vai trò vẫn được che'
 s.circle(x+20,y+h-24,2.5,'#b99bde');s.text(x+31,y+h-21,hint,10 if mobile else 12,'#bdb1cd')
 if not mobile:s.text(x+w-24,y+h-21,'Tab · Enter',9,'#887895','end')

def own_card(s,x,y,w,h,revealed):
 s.rect(x,y,w,h,'url(#cardBack)',7,'#af8d56',1)
 if revealed:
  cp='ownrole';s.add(f'<defs><clipPath id="{cp}"><rect x="{x+1}" y="{y+1}" width="{w-2}" height="{h-2}" rx="6"/></clipPath></defs>');s.img(WOLF,x+1,y+1,w-2,h*.79,cp);s.rect(x+1,y+h*.64,w-2,h*.36-1,'url(#roleFade)',6);s.text(x+10,y+h*.80,'PHE MA SÓI · DỮ LIỆU MẪU',5 if w<100 else 7,'#ceb280',spacing=.4);s.text(x+10,y+h*.90,'Ma Sói',13 if w<100 else 25,'#f2ddb4',family='DejaVu Serif');
  if w>100:s.text(x+10,y+h*.955,'Chọn một người khác.',11,'#c0b3c9')
 else:
  s.rect(x+6,y+6,w-12,h-12,'none',4,'#89714977',.7);s.text(x+w/2,y+h*.10,'T A B U L A',4 if w<100 else 8,'#ceb387','middle',spacing=1)
  scale=w/180;ox=x+w/2-45*scale;oy=y+h*.24;s.add(f'<g transform="translate({ox} {oy}) scale({scale})" stroke="#d0b17b" stroke-width="1.2" fill="#d0b37914"><path d="m20 13 12 12 18-8 18 8 12-12 4 29-10 37-24 17-24-17-10-37Z"/><path d="m26 49 14 6-6 9-12-9m52-6-14 6 6 9 12-9M37 81l13 10 13-10-13-8Z"/><path d="m32 25-8 17m44-17 8 17M50 20v19"/></g>')
  s.text(x+w/2,y+h*.73,'MA SÓI',9 if w<100 else 17,'#ceb387','middle','DejaVu Serif',spacing=2);s.line(x+w*.42,y+h*.78,x+w*.58,y+h*.78,'#b19767');s.text(x+w/2,y+h*.86,'Một lá bài.',5 if w<100 else 9,'#a99783','middle');s.text(x+w/2,y+h*.92,'Một bí mật.',5 if w<100 else 9,'#a99783','middle')

def role_panel(s,mobile,revealed,phase):
 if mobile:
  x,y,w,h=12,730,366,178;s.rect(x,y,w,h,'#241e31',17,'#d1bafb20');own_card(s,27,753,89,132,revealed);s.icon('lock',132,751,13,'#bcadc8');s.text(151,760,'LÁ BÀI CỦA BẠN',10,'#bcadc8',spacing=.8);s.rect(327,747,35,16,'#c9b1ff12',4);s.text(344,758,'Ghế 04',9,'#bba7cc','middle');s.icon('eye',132,788,15);s.text(155,800,'Che lại lá bài của tôi' if revealed else 'Lật xem lá bài của tôi',12,PURPLE);s.text(132,830,'Chỉ lá bài của bạn được lật.',11,'#ad9cc0');s.text(132,848,'Vai trò các ghế khác vẫn được che.',11,'#ad9cc0');s.text(132,884,'Esc · che ngay khi rời màn hình',7,'#8e7ca2')
 else:
  x,y,w,h=1142,184,270,407;s.rect(x,y,w,h,'#241e31',22,'#d1bafb20');s.icon('lock',1160,202,13,'#bcadc8');s.text(1180,213,'LÁ BÀI CỦA BẠN',10,'#bcadc8',spacing=1.2);s.rect(1355,200,40,19,'#c9b1ff12',4);s.text(1375,213,'Ghế 04',8,'#bba7cc','middle');own_card(s,1187,234,180,266,revealed);s.icon('eye',1175,519,15);s.text(1198,532,'Che lại lá bài của tôi' if revealed else 'Lật xem lá bài của tôi',12,PURPLE);s.text(1277,556,'Chỉ lá bài của bạn được lật.',11,'#9486a4','middle');s.text(1277,572,'Vai trò các ghế khác vẫn được che.',11,'#9486a4','middle')
  s.rect(1142,607,270,187,'#1d192b',19,'#d1bafb16');s.text(1162,633,'LỜI DẪN CỦA QUẢN TRÒ',8,GOLD,spacing=1.4)
  lines={'night':['“Ánh trăng rọi qua mái nhà.','Hãy chọn mục tiêu của bạn,','rồi để màn đêm tiếp tục.”'],'day':['“Trời sáng. Làng Sương','thức giấc. Hãy lắng nghe','nhau trước khi quyết định.”'],'vote':['“Đã đến lúc đưa ra lựa chọn.','Một lá phiếu sẽ bay đến','người mà bạn chọn.”']}[phase]
  for i,line in enumerate(lines):s.text(1162,662+i*25,line,14,'#d3c4dc',family='DejaVu Serif')
  s.line(1162,768,1178,768,'#8d7f9d');s.text(1187,772,'Quản trò máy · câu dẫn mẫu',8,'#8d7f9d')

def wrap_text(text,width,size):
 font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',size);lines=['']
 for word in text.split():
  candidate=(lines[-1]+' '+word).strip()
  if font.getlength(candidate)>width and lines[-1]:lines.append(word)
  else:lines[-1]=candidate
 return lines

def dock(s,mobile,phase,selected):
 ey,tt,ds,bt={'night':('HÀNH ĐỘNG TRONG ĐÊM','Bạn sẽ chọn ai?','Chọn một chân dung. Vai trò của họ vẫn là bí mật.','Xác nhận mục tiêu'),'day':('CẢNH THẢO LUẬN MẪU','Làng cùng thức giấc','Quan sát các ghế. Cảnh mẫu chưa có chat hay voice.','Chờ cảnh biểu quyết'),'vote':('BIỂU QUYẾT CÔNG KHAI','Lá phiếu của bạn','Chọn một người để xem animation lá phiếu mẫu.','Gửi lá phiếu mẫu')}[phase]
 enabled=selected is not None and phase!='day'
 if mobile:
  s.rect(12,921,366,170,'#292135',18,'#cbb5ff30');s.text(29,947,ey,9,'#baa0df',spacing=1);s.text(29,973,tt,17);desc_lines=wrap_text(ds,204,11);s.text(29,994,desc_lines[0],11,'#b3a5c0');s.text(29,1009,desc_lines[1] if len(desc_lines)>1 else '',11,'#b3a5c0');tx=289;ty=963
  s.rect(29,1029,332,45,PURPLE if enabled else '#433550',13);s.text(47,1057,bt,12,'#352048' if enabled else '#9581a4');s.icon('chevron',335,1043,17,'#352048' if enabled else '#9581a4')
  s.text(16,1118,'Bản thiết kế • dữ liệu mẫu • mô phỏng cục bộ',10,'#766888');s.text(374,1138,'Thử chuyển cảnh ›',10,'#9d8faf','end')
 else:
  s.rect(28,810,1384,104,'#292135',22,'#cbb5ff30');s.rect(53,837,49,49,'none',15,'#bb9fce40');s.text(77.5,870,'02',26,'#a992bf','middle','DejaVu Serif');s.text(120,837,ey,8,'#baa0df',spacing=1.6);s.text(120,863,tt,18);s.text(120,886,ds,11,'#b3a5c0');tx=1042;ty=862
  s.rect(1177,839,210,46,PURPLE if enabled else '#433550',16);s.text(1198,868,bt,12,'#352048' if enabled else '#9581a4');s.icon('chevron',1361,853,17,'#352048' if enabled else '#9581a4');s.text(36,943,'Bản thiết kế • dữ liệu mẫu • mô phỏng cục bộ',10,'#766888');s.text(1404,943,'Thử chuyển cảnh ›',10,'#9d8faf','end')
 r=14.5 if mobile else 19;s.circle(tx,ty,r,'#1f1829','#9d83b94d')
 if selected is not None:
  cp='targetPreview';s.add(f'<defs><clipPath id="{cp}"><circle cx="{tx}" cy="{ty}" r="{r-1}"/></clipPath></defs>');s.img(PORTRAITS[selected],tx-r+1,ty-r+1,(r-1)*2,(r-1)*2,cp)
 s.text(tx+22 if mobile else tx+30,ty-4,'MỤC TIÊU',8 if mobile else 10,'#9f8bab',spacing=.7);s.text(tx+22 if mobile else tx+30,ty+11,NAMES[selected] if selected is not None else 'Chưa chọn',12,'#dfd0ee')

def export(name,mobile=False,phase='night',selected=4,revealed=True):
 w,h=(390,1160) if mobile else (1440,960);s=SVG(w,h);s.rect(0,0,w,h,'#100e19');header(s,mobile);title(s,mobile,phase);arena(s,mobile,phase,selected);role_panel(s,mobile,revealed,phase);dock(s,mobile,phase,selected)
 path=HERE/f'{name}.svg';path.write_text(s.finish());png=HERE/f'{name}.png';subprocess.run(['inkscape',str(path),'--export-type=png',f'--export-filename={png}'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
 im=Image.open(png).convert('RGB');im.save(HERE/f'{name}.jpg',quality=79,optimize=True)
 print(name,im.size,(HERE/f'{name}.jpg').stat().st_size)

if __name__=='__main__':
 export('desktop-night',selected=4,revealed=True)
 export('mobile-night',mobile=True,selected=4,revealed=False)
 export('desktop-day',phase='day',selected=None,revealed=False)
 export('desktop-vote',phase='vote',selected=10,revealed=False)
