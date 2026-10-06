#!/usr/bin/env python3
"""Standalone design references, not browser/runtime screenshots.
Public account-avatar fixtures mirror dashboard/seat identity. Role art is private.
Render with Inkscape; no CSS or JS screenshot claim is made.
"""
from pathlib import Path
from io import BytesIO
import base64, json, math, html, subprocess, urllib.parse, re
from PIL import Image, ImageFont

HERE=Path(__file__).resolve().parent
FIXTURES=json.loads((HERE/'avatar-fixtures.json').read_text())
ACCOUNTS=FIXTURES.get('profiles', FIXTURES.get('accounts', FIXTURES.get('players', FIXTURES.get('avatars', FIXTURES))))
if not isinstance(ACCOUNTS,list):raise ValueError('avatar-fixtures.json needs an accounts/players/avatars list')
def field(a,*keys):
 for k in keys:
  if k in a:return a[k]
 raise KeyError(keys)
NAMES=[field(a,'displayName','name','display_name') for a in ACCOUNTS]
OWN_INDEX=next(i for i,a in enumerate(ACCOUNTS) if a['subjectId']==FIXTURES['ownSubjectId'])
OWN_SEAT_LABEL=f'Ghế {OWN_INDEX+1:02d}'
def avatar_uri(a):
 value=field(a,'avatar','avatarPath','path','src','file')
 if isinstance(value,dict):value=field(value,'assetRef','url','src','path','file')
 if value.startswith('data:'):return value
 return 'data:image/svg+xml;base64,'+base64.b64encode((HERE/value).read_bytes()).decode()
PORTRAITS=[avatar_uri(a) for a in ACCOUNTS]
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
 public_avatar(s,OWN_INDEX,s.w-(195.5 if mobile else 390),31 if mobile else 38,14.5 if mobile else 16,prefix='headerAccount')
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
 r=28.5 if mobile else 36;stroke='#dfcfff' if i==selected else '#dbc38b' if i==OWN_INDEX else '#b6a28a';pr=f'portrait{i}'
 if i==selected:s.circle(cx,cy,r+5,'#ba94ff28',PURPLE,1.2)
 s.circle(cx,cy+3,r+2,'#00000055');s.circle(cx,cy,r,'#2a2840',stroke,2 if not mobile else 1.5);s.add(f'<defs><clipPath id="{pr}"><circle cx="{cx}" cy="{cy}" r="{r-4}"/></clipPath></defs>');s.img(PORTRAITS[i],cx-r+4,cy-r+4,(r-4)*2,(r-4)*2,pr)
 if i==OWN_INDEX:s.rect(cx-17,cy+r-9,34,13 if mobile else 15,'#c6b085',5,'#24192e',1);s.text(cx,cy+r+(.5 if mobile else 2),'BẠN',6 if mobile else 8,'#24192e',anchor='middle',spacing=.8)
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
  x,y,w,h=12,730,366,178;s.rect(x,y,w,h,'#241e31',17,'#d1bafb20');own_card(s,27,753,89,132,revealed);s.icon('lock',132,751,13,'#bcadc8');s.text(151,760,'LÁ BÀI CỦA BẠN',10,'#bcadc8',spacing=.8);s.rect(327,747,35,16,'#c9b1ff12',4);s.text(344,758,OWN_SEAT_LABEL,9,'#bba7cc','middle');s.icon('eye',132,788,15);s.text(155,800,'Che lại lá bài của tôi' if revealed else 'Lật xem lá bài của tôi',12,PURPLE);s.text(132,830,'Chỉ lá bài của bạn được lật.',11,'#ad9cc0');s.text(132,848,'Vai trò các ghế khác vẫn được che.',11,'#ad9cc0');s.text(132,884,'Esc · che ngay khi rời màn hình',7,'#8e7ca2')
 else:
  x,y,w,h=1142,184,270,407;s.rect(x,y,w,h,'#241e31',22,'#d1bafb20');s.icon('lock',1160,202,13,'#bcadc8');s.text(1180,213,'LÁ BÀI CỦA BẠN',10,'#bcadc8',spacing=1.2);s.rect(1355,200,40,19,'#c9b1ff12',4);s.text(1375,213,OWN_SEAT_LABEL,8,'#bba7cc','middle');own_card(s,1187,234,180,266,revealed);s.icon('eye',1175,519,15);s.text(1198,532,'Che lại lá bài của tôi' if revealed else 'Lật xem lá bài của tôi',12,PURPLE);s.text(1277,556,'Chỉ lá bài của bạn được lật.',11,'#9486a4','middle');s.text(1277,572,'Vai trò các ghế khác vẫn được che.',11,'#9486a4','middle')
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
  target_profile=ACCOUNTS[selected]
  s.add(f'<g data-subject-id="{esc(target_profile["subjectId"])}" data-avatar-ref="{esc(target_profile["avatar"]["assetRef"])}"><title>{esc(NAMES[selected])} · avatar mục tiêu</title>')
  cp='targetPreview';s.add(f'<defs><clipPath id="{cp}"><circle cx="{tx}" cy="{ty}" r="{r-1}"/></clipPath></defs>');s.img(avatar_uri(target_profile),tx-r+1,ty-r+1,(r-1)*2,(r-1)*2,cp);s.add('</g>')
 s.text(tx+22 if mobile else tx+30,ty-4,'MỤC TIÊU',8 if mobile else 10,'#9f8bab',spacing=.7);s.text(tx+22 if mobile else tx+30,ty+11,NAMES[selected] if selected is not None else 'Chưa chọn',12,'#dfd0ee')

def public_avatar(s,i,cx,cy,r,selected=False,own=False,prefix='account'):
 """Render the actual account fixture; frame expresses selection/identity only."""
 if selected:s.circle(cx,cy,r+5,'#ba94ff28',PURPLE,1.2)
 s.circle(cx,cy+2,r+1,'#00000055')
 s.circle(cx,cy,r,'#2a2840',PURPLE if selected else GOLD if own else '#a99bb9',1.5)
 cp=f'{prefix}{i}'
 s.add(f'<defs><clipPath id="{cp}"><circle cx="{cx}" cy="{cy}" r="{r-2}"/></clipPath></defs>')
 s.img(PORTRAITS[i],cx-r+2,cy-r+2,(r-2)*2,(r-2)*2,cp)

def slim_header(s,landscape=False,compact=False):
 h=42 if landscape else 56
 s.rect(0,0,s.w,h,'#13101d');s.line(0,h-1,s.w,h-1,'#2b2536')
 x=12 if landscape or compact else 16;y=(h-22)/2
 for i in range(4):s.rect(x+(i%2)*11,y+(i//2)*11,8,8,'#c6acff' if i in [0,3] else '#9678cf',2)
 s.text(x+28,h/2+6,'tabula',19,'#e1d5ff',weight='bold',spacing=-1)
 public_avatar(s,OWN_INDEX,s.w-45 if compact else s.w-189 if landscape else 194.5,h/2,14.5,prefix='headerAccount')
 if not compact:s.circle(s.w-123,h/2,2.5,'#b89be8');s.text(s.w-114,h/2+4,'Mô phỏng cục bộ',12,'#ac9fc1')

def responsive_title(s,kind):
 if kind=='landscape':
  s.text(12,70,'Đêm ở làng Sương',22,family='DejaVu Serif');s.text(263,69,'CLASSIC · 12 NGƯỜI · MẪU',9,GOLD,spacing=.8)
  s.rect(645,49,187,28,'#252032',10);s.icon('moon',655,55,16);s.text(679,67,'ĐÊM 02',9,'#cdbde1');s.text(752,68,'01:24',15)
 elif kind=='compact':
  s.text(12,78,'CLASSIC · 12 NGƯỜI · MẪU',8,GOLD,spacing=.6);s.text(12,108,'Đêm ở làng Sương',21,family='DejaVu Serif')
  s.rect(240,65,68,55,'#252032',11);s.text(251,83,'ĐÊM 02',8,'#cdbde1');s.text(251,108,'01:24',15)
 else:
  s.text(16,78,'CLASSIC · 12 NGƯỜI · MẪU',8,GOLD,spacing=.8);s.text(16,105,'Đêm ở làng Sương',24,family='DejaVu Serif');s.text(16,124,'Nhắm mắt. Những bí mật bắt đầu thức giấc.',9,MUTED)
  s.rect(300,71,74,55,'#252032',12);s.text(311,89,'ĐÊM 02',8,'#cdbde1');s.text(311,112,'01:24',18)

def responsive_arena(s,kind,selected=4):
 if kind=='mobile':x,y,w,h,r,cols=12,142,366,430,28,4;centers=[(x+w/8+(i%4)*w/4,y+112+(i//4)*102) for i in range(12)];name_size,seat_size=12,12
 elif kind=='compact':x,y,w,h,r,cols=8,131,304,318,21,4;centers=[(x+w/8+(i%4)*w/4,y+94+(i//4)*74) for i in range(12)];name_size,seat_size=12,12
 else:x,y,w,h,r,cols=12,88,568,256,22,6;centers=[(x+w/12+(i%6)*w/6,y+76+(i//6)*108) for i in range(12)];name_size,seat_size=12,12
 cp='responsiveArena';s.add(f'<defs><clipPath id="{cp}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"/></clipPath></defs>');s.rect(x,y,w,h,'#171424',18,'#d0bdf330')
 s.img(SCENE,x,y,w,255 if kind!='landscape' else h,cp);s.rect(x,y,w,h,'url(#mobileShade)',18)
 s.icon('leaf',x+12,y+12,13,'#ead5b5');s.text(x+32,y+23,'LÀNG SƯƠNG',8,'#ead5b5',spacing=.8);s.text(x+w-12,y+23,'12 còn sống',9,'#d0c8d8','end')
 if kind=='mobile':s.icon('moon',x+w/2-8,y+42,16,'#ddc390');s.text(x+w/2,y+78,'Làng đang ngủ',15,'#fff0d6','middle','DejaVu Serif')
 elif kind=='compact':s.text(x+w/2,y+48,'Làng đang ngủ',13,'#fff0d6','middle','DejaVu Serif')
 else:s.text(x+w/2,y+43,'Chọn mục tiêu trong vùng ánh trăng',10,'#e8d4c0','middle')
 for i,(cx,cy) in enumerate(centers):
  public_avatar(s,i,cx,cy,r,i==selected,i==OWN_INDEX,prefix='responsiveSeat')
  name_offset,seat_offset=(16,29) if kind=='compact' else (20,35)
  s.rect(cx-(30 if kind=='mobile' else 27),cy+r+(3 if kind=='compact' else 6),60 if kind=='mobile' else 54,16 if kind=='compact' else 18,'#141023d9',6)
  s.text(cx,cy+r+name_offset,NAMES[i],name_size,'#f2eafa','middle')
  s.text(cx,cy+r+seat_offset,f'GHẾ {i+1:02d}',seat_size,'#cbbbac','middle')
 if kind!='landscape':s.circle(x+15,y+h-18,2.5,'#b99bde');s.text(x+25,y+h-15,f'Đã chọn {NAMES[selected]} · avatar tài khoản công khai',8 if kind=='compact' else 9,'#bdb1cd')

def collapsed_role(s,x,y,w,h,small=False):
 s.rect(x,y,w,h,'#241e31',12,'#d1bafb20');s.icon('lock',x+12,y+(h-16)/2,16,'#bcadc8')
 s.text(x+35,y+h/2+4,f'Lá bài của bạn · {OWN_SEAT_LABEL}',10 if small else 11,'#cabbd6')
 if w>330:s.icon('eye',x+w-87,y+(h-16)/2,16);s.text(x+w-65,y+h/2+4,'Lật xem',10,PURPLE)
 else:s.icon('eye',x+w-29,y+(h-17)/2,17)

def responsive_dock(s,kind,selected=4):
 if kind=='mobile':
  collapsed_role(s,12,584,366,46)
  x,y,w,h=12,642,366,158;s.rect(x,y,w,h,'#292135',18,'#cbb5ff30');s.text(x+16,y+23,'HÀNH ĐỘNG TRONG ĐÊM',8,'#baa0df',spacing=.7);s.text(x+16,y+49,'Bạn sẽ chọn ai?',18)
  public_avatar(s,selected,x+w-70,y+42,17,prefix='mobileTarget');s.text(x+w-44,y+46,NAMES[selected],11,'#dfd0ee')
  s.text(x+16,y+70,'Avatar là danh tính công khai. Vai trò vẫn được che.',9,'#b3a5c0')
  s.rect(x+16,y+97,w-32,44,PURPLE,13);s.text(x+32,y+125,'Xác nhận mục tiêu',12,'#352048');s.icon('chevron',x+w-42,y+110,17,'#352048')
  s.text(16,819,'Bản thiết kế · dữ liệu mẫu · mô phỏng cục bộ',9,'#8e7ca2')
 elif kind=='compact':
  collapsed_role(s,8,458,304,44,True)
  s.rect(8,511,304,122,'#292135',14,'#cbb5ff30');s.text(22,531,'HÀNH ĐỘNG TRONG ĐÊM · MẪU',8,'#baa0df',spacing=.4);s.text(22,554,f'Mục tiêu: {NAMES[selected]} · Ghế {selected+1:02d}',12,'#dfd0ee')
  public_avatar(s,selected,282,549,13.5,prefix='compactTarget')
  s.rect(20,576,280,44,PURPLE,12);s.text(35,604,'Xác nhận mục tiêu',12,'#352048');s.icon('chevron',274,589,17,'#352048')
 else:
  collapsed_role(s,592,88,240,42,True)
  s.rect(592,140,240,68,'#1d192b',12,'#d1bafb16');s.text(606,159,'QUẢN TRÒ MÁY · MẪU',8,GOLD,spacing=.5);s.text(606,181,'“Hãy chọn mục tiêu của bạn,',11,'#d3c4dc','start','DejaVu Serif');s.text(606,197,'rồi để màn đêm tiếp tục.”',11,'#d3c4dc','start','DejaVu Serif')
  s.rect(592,220,240,124,'#292135',14,'#cbb5ff30');s.text(606,241,'HÀNH ĐỘNG TRONG ĐÊM',8,'#baa0df',spacing=.6);s.text(606,266,f'Mục tiêu: {NAMES[selected]} · Ghế {selected+1:02d}',11,'#dfd0ee');s.rect(605,284,214,44,PURPLE,12);s.text(619,312,'Xác nhận mục tiêu',12,'#352048');s.icon('chevron',792,297,17,'#352048');s.text(14,377,'Bản thiết kế · dữ liệu mẫu · mô phỏng cục bộ',9,'#8e7ca2');s.text(832,377,'Avatar công khai · vai trò riêng tư',9,'#8e7ca2','end')

def responsive(kind):
 w,h={'mobile':(390,844),'compact':(320,640),'landscape':(844,390)}[kind];s=SVG(w,h);s.rect(0,0,w,h,'#100e19');slim_header(s,kind=='landscape',kind=='compact');responsive_title(s,kind);responsive_arena(s,kind);responsive_dock(s,kind);return s

def avatar_sync_reference():
 s=SVG(1440,960);s.rect(0,0,1440,960,'#100e19');header(s,False)
 s.text(36,121,'DANH TÍNH CÔNG KHAI · DỮ LIỆU MẪU',10,GOLD,spacing=1.4);s.text(36,164,'Cùng một avatar tài khoản, từ dashboard đến bàn chơi',29,family='DejaVu Serif')
 s.text(36,195,'Mẫu đối chiếu danh tính; phần bên trái chỉ là thẻ hồ sơ, không phải thiết kế dashboard đầy đủ.',12,MUTED)
 s.rect(28,221,448,636,'#241e31',22,'#d1bafb20');s.text(52,260,'Thẻ hồ sơ trên dashboard',19);s.text(52,285,'Tên và file avatar gắn với subject_id',11,MUTED)
 s.rect(524,221,888,636,'#171424',22,'#d1bafb20');cp='syncScene';s.add('<defs><clipPath id="syncScene"><rect x="524" y="221" width="888" height="636" rx="22"/></clipPath></defs>');s.img(SCENE,524,221,888,636,cp);s.rect(524,221,888,636,'url(#shade)',22);s.text(552,260,'Ghế công khai trên bàn Ma Sói',19);s.text(552,285,'Cùng asset avatar; số ghế và trạng thái nằm ngoài chân dung',11,'#d4c5db')
 for i in range(6):
  y=310+i*87;s.rect(48,y,408,74,'#1b1628',13,'#d1bafb12');public_avatar(s,i,89,y+37,25,prefix='dashboard')
  s.text(128,y+30,NAMES[i],14);account=str(ACCOUNTS[i].get('subjectId',ACCOUNTS[i].get('accountId',ACCOUNTS[i].get('id',f'fixture-{i+1:02d}'))));s.text(128,y+50,account,9,MUTED);s.text(435,y+43,'Công khai',9,'#c9b0ef','end')
  cx=744+(i%2)*444;cy=390+(i//2)*176;public_avatar(s,i,cx,cy,42,i==4,i==OWN_INDEX,prefix='syncSeat');s.text(cx,cy+66,NAMES[i],16,'#f2eafa','middle');s.text(cx,cy+86,f'GHẾ {i+1:02d}'+(' · BẠN' if i==OWN_INDEX else ''),11,'#cbbbac','middle')
 s.rect(28,878,1384,49,'#292135',15,'#cbb5ff30');s.icon('lock',47,894,16);s.text(74,908,'Avatar không chứa mặt nạ, huy hiệu phe hoặc biểu tượng vai trò. Lá bài riêng tư dùng asset khác.',12,'#d3c4dc');s.text(36,947,'Bản tham chiếu tĩnh · asset fixture dùng chung với prototype',10,'#8e7ca2');return s

def save_svg(name,s):
 """Publish only complete renders so packaging cannot observe an in-flight image."""
 path=HERE/f'{name}.svg';svg_tmp=HERE/f'.{name}.render.svg';svg_tmp.write_text(s.finish());svg_tmp.replace(path)
 png=HERE/f'{name}.png';png_tmp=HERE/f'.{name}.render.png'
 jpg=HERE/f'{name}.jpg';jpg_tmp=HERE/f'.{name}.render.jpg'
 subprocess.run(['inkscape',str(path),'--export-type=png',f'--export-filename={png_tmp}'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
 with Image.open(png_tmp) as im:
  im.load();rgb=im.convert('RGB');dimensions=im.size
 rgb.save(jpg_tmp,format='JPEG',quality=79,optimize=True)
 with Image.open(jpg_tmp) as check:check.load()
 png_tmp.replace(png);jpg_tmp.replace(jpg)
 print(name,dimensions,jpg.stat().st_size)

def export(name,mobile=False,phase='night',selected=4,revealed=True):
 if mobile:s=responsive('mobile')
 else:
  s=SVG(1440,960);s.rect(0,0,1440,960,'#100e19');header(s,False);title(s,False,phase);arena(s,False,phase,selected);role_panel(s,False,revealed,phase);dock(s,False,phase,selected)
 save_svg(name,s)

if __name__=='__main__':
 export('desktop-night',selected=4,revealed=True)
 export('mobile-night',mobile=True,selected=4,revealed=False)
 export('desktop-day',phase='day',selected=None,revealed=False)
 export('desktop-vote',phase='vote',selected=10,revealed=False)
 save_svg('compact-night',responsive('compact'))
 save_svg('landscape-night',responsive('landscape'))
 save_svg('dashboard-avatar-sync',avatar_sync_reference())
