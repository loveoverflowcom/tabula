from build_designs import Scene,P
from pathlib import Path
import math,json
OUT=Path(__file__).resolve().parent
# Preview only: no full-pack regeneration until parent design review.
C={'bg':'#FFFBFF','rail':'#F4EDF7','low':'#F4EDF7','high':'#EAE3ED','primary':'#5634BE','on':'#FFFFFF','pc':'#E9E0FA','onpc':'#32007E','ink':'#1E1C21','muted':'#4B4650','sage':'#DEE8DB','onsage':'#244830','warm':'#F3E6D2','onwarm':'#62472E','rose':'#F3DFE2','onrose':'#683544','night':'#302F44'}

def rr(s,x,y,w,h,radii,fill):
 tl,tr,br,bl=radii
 s.a.append(f'<path d="M{x+tl} {y}H{x+w-tr}Q{x+w} {y} {x+w} {y+tr}V{y+h-br}Q{x+w} {y+h} {x+w-br} {y+h}H{x+bl}Q{x} {y+h} {x} {y+h-bl}V{y+tl}Q{x} {y} {x+tl} {y}Z" fill="{fill}"/>')
def button(s,x,y,w,t,kind='filled',h=52,shape='round',focus=False,pressed=False):
 colors={'filled':(C['primary'],'white'),'tonal':('#CFBCFF',C['onpc']),'surface':(C['high'],C['muted']),'white':('white',C['primary']),'dark':(C['onpc'],'white'),'disabled':('#E1DDE4','#696370')};bg,fg=colors[kind];rad=12 if pressed else h/2 if shape=='round' else 16
 if focus:s.rect(x-5,y-5,w+10,h+10,'none',C['primary'],r=rad+5,sw=3)
 s.rect(x,y,w,h,bg,r=rad);s.text(x+w/2,y+h/2+6,t,16 if h>=52 else 14,fg,600,'middle')
def icon(s,x,y,t,bg=C['pc'],fg=C['onpc'],size=48,square=False):s.rect(x,y,size,size,bg,r=16 if square else size/2);s.text(x+size/2,y+size*.68,t,24,fg,600,'middle')
def badge(s,x,y,t,bg,fg,w):s.rect(x,y,w,28,bg,r=14);s.text(x+w/2,y+19,t,11,fg,600,'middle')
def group(s,x,y,labels,widths,selected=0,h=52):
 dx=0
 for i,(t,w) in enumerate(zip(labels,widths)):
  # Official connected-group anatomy: 2dp gaps,8dp inner corners,round outer corners.
  rad=(h/2,8,8,h/2) if i==0 else (8,h/2,h/2,8) if i==len(labels)-1 else (8,8,8,8)
  rr(s,x+dx,y,w,h,rad,C['primary'] if i==selected else C['high']);s.text(x+dx+w/2,y+h/2+6,t,14,'white' if i==selected else C['onpc'],600,'middle');dx+=w+2

def shell(s,title,section='Thư viện'):
 s.rect(0,0,1440,1000,C['bg'],r=0);s.rect(0,0,176,1000,C['rail'],r=0);s.rect(20,19,36,36,C['primary'],r=13);s.text(38,45,'T.',24,'white',700,'middle');s.text(67,45,'tabula',22,weight=700)
 for i,(symbol,t) in enumerate([('▦','Thư viện'),('↺','Ván của tôi'),('♙','Phòng chơi'),('♡','Bạn bè')]):
  y=91+i*62;active=t==section
  if active:s.rect(12,y,152,52,C['pc'],r=26)
  s.text(30,y+35,symbol,23,C['onpc'] if active else C['muted']);s.text(64,y+34,t,13,C['onpc'] if active else C['muted'],600 if active else 400)
 s.label(24,381,'TRUY CẬP NHANH');icon(s,20,402,'♞',C['sage'],C['onsage'],40);s.text(70,428,'Cờ vua',13,weight=600);icon(s,20,457,'帥',C['pc'],C['onpc'],40);s.text(70,482,'Xiangqi',13,weight=600);s.text(70,504,'Định hướng #48',10,C['muted'])
 if section=='Cài đặt':s.rect(12,884,152,52,C['pc'],r=26)
 s.text(30,920,'⚙',23,C['onpc']);s.text(64,920,'Cài đặt',13,C['onpc'],600);s.circle(39,966,18,C['pc']);s.text(39,971,'B',13,C['onpc'],700,'middle');s.text(70,962,'Bạn',13,weight=600);s.text(70,981,'Hồ sơ mẫu',10,C['muted'])
 s.text(208,39,'Không gian chơi / '+title,13,C['muted']);badge(s,1120,18,'M3 EXPRESSIVE',C['pc'],C['onpc'],160);icon(s,1336,8,'B',C['pc'],C['onpc'],48);s.text(208,108,title,32,weight=700)
 s.text(208,976,'Tabula · Design 02 / M3 Expressive revision · Bản thiết kế tĩnh, dữ liệu mẫu',11,C['muted']);s.text(1408,976,'01.10.2026',11,C['muted'],anchor='end',mono=True)

def art(s,x,y,w,h,kind,bg,fg):
 # Artwork is contained by tonal surface, not boxed in a second bordered card.
 if kind=='chess':
  s.circle(x+w*.6,y+h*.56,h*.51,'#C2D4BB');s.text(x+w*.39,y+h*.83,'♞',h*.97,fg,600,'middle');s.text(x+w*.72,y+h*.83,'♔',h*.85,'#607B58',600,'middle')
 elif kind=='tiles':
  for i in range(3):
   xx=x+w*.27+i*45;yy=y+16+i*9;s.rect(xx,yy,64,64,'#BCCFAF',r=12);s.line(xx+32,yy,xx+32,yy+64,'#F6EBCB',10)
   if i==1:s.line(xx+32,yy+32,xx+64,yy+32,'#F6EBCB',10)
  s.text(x+w*.73,y+91,'♟',42,fg,600,'middle')
 elif kind=='xq':
  # Varied circle/rounded-square forms deliberately match game-family identity.
  s.rect(x+w*.27-34,y+18,78,78,'#FFF2DB',r=23);s.text(x+w*.27+5,y+78,'帥',51,'#9F3C39',600,'middle');s.circle(x+w*.68,y+66,39,'#FFF2DB');s.text(x+w*.68,y+85,'馬',48,C['onpc'],600,'middle')
 elif kind=='caro':s.rect(x+w*.26,y+20,76,76,'#E7C7AC',r=24);s.text(x+w*.26+38,y+82,'×',67,'#A96C49',600,'middle');s.circle(x+w*.68,y+62,35,'none','#B78965',9)
 else:
  s.circle(x+w*.64,y+53,38,'#EEE0A5');s.a.append(f'<path d="M{x+52} {y+110}l44-79 44 79Z M{x+119} {y+110}l42-62 42 62Z M{x+203} {y+110}l40-79 40 79Z" fill="#55556E"/>');s.circle(x+w*.52,y+77,3,'#EEE0A5');s.circle(x+w*.6,y+77,3,'#EEE0A5')

def game(s,x,y,w,name,kind,bg,fg,current=False):
 s.rect(x,y,w,232,bg,r=28);art(s,x+10,y+10,w-20,110,kind,bg,fg);icon(s,x+w-62,y+14,'♡','white' if kind!='wolf' else '#49485F',fg if kind!='wolf' else '#E0D8F0',44)
 s.text(x+22,y+158,name,22,fg,700);s.text(x+22,y+184,'2 người · cùng máy' if kind=='chess' else '2–5 người · đặt mảnh' if kind=='tiles' else 'Chơi / Phân tích / Học · dự kiến' if kind=='xq' else 'Adapter chưa đầy đủ',12,fg)
 badge(s,x+22,y+196,'Chơi local' if current else 'Dự kiến #48' if kind=='xq' else 'Chưa mở','white' if kind!='wolf' else '#55556E',fg if kind!='wolf' else '#E7E0F3',110);s.text(x+w-22,y+219,'→' if current else '·',23,fg,600,'end')

def library():
 s=Scene();shell(s,'Thư viện game');s.text(208,142,'Chọn một ván local. Các tính năng tương lai được ghi rõ trạng thái.',14,C['muted']);s.rect(974,86,434,58,C['high'],r=29);s.text(1000,123,'⌕',24,C['muted']);s.text(1036,122,'Tìm trò chơi',15,C['muted']);badge(s,1341,100,'/',C['bg'],C['muted'],42)
 # Strongest salience belongs to continuation, not an editorial hero.
 s.rect(208,170,1200,108,C['primary'],r=28);icon(s,228,194,'♞','#CFBCFF','#32007E',60,square=True);s.text(312,211,'Tiếp tục cờ vua',24,'white',700);s.text(312,245,'Local · đến lượt Trắng · dữ liệu mẫu',13,'#EAE1FF');s.text(1134,238,'09:42',39,'white',700,'end',True);button(s,1160,196,224,'Tiếp tục →','white',56)
 group(s,208,302,['Tất cả','Chơi local','Chiến thuật','Hội nhóm','Yêu thích'],[126,148,160,144,148],0,52);s.text(1408,335,'5 game · 2 game chơi local',12,C['muted'],anchor='end')
 s.text(208,393,'Chọn trò chơi',19,weight=700);s.text(1408,393,'Local / Dự kiến',12,C['muted'],anchor='end')
 games=[('Cờ vua','chess',C['sage'],C['onsage'],True),('Miền đất nhỏ','tiles','#E4EBDB',C['onsage'],True),('Cờ tướng','xq',C['pc'],C['onpc'],False),('Caro','caro',C['warm'],C['onwarm'],False),('Ma sói','wolf',C['night'],'#F0EAF7',False)]
 for i,g in enumerate(games):game(s,208+i%3*408,414+i//3*254,384,*g)
 # Scientific, short architecture/capability information stays low-salience.
 s.rect(1024,668,384,232,C['low'],r=28);s.text(1048,710,'Chơi local, không cần',18,weight=600);s.text(1048,738,'tài khoản',18,weight=600);s.lines(1048,776,['Chess và Tiles có presenter thật.','Online, bot, Xiangqi cần gate riêng.','Không dùng mẫu làm trạng thái server.'],12,C['muted'],step=24);button(s,1048,843,336,'Khả năng từng game →','tonal',44)
 return s

def foundation():
 s=Scene();shell(s,'Material 3 Expressive · Foundation','Cài đặt');s.text(208,142,'Component reference cho Leptos / Macroquad. Chưa phải bộ component native đã có trong Tabula.',13,C['muted'])
 # Action family: isolated filled/tonal buttons with contrasted shape and size.
 s.rect(208,174,584,294,C['pc'],r=28);s.label(232,207,'01 / HÀNH ĐỘNG');s.text(232,241,'Đưa việc chính lên trước',24,C['onpc'],700);button(s,232,265,324,'Bắt đầu ván →','filled',64);button(s,572,273,196,'Thiết lập','tonal',48);s.text(232,365,'Filled: việc chính · Tonal: việc liên quan',13,C['onpc']);button(s,232,391,230,'Thử lại','tonal',52,shape='square');button(s,476,391,292,'Chưa mở / có gate','disabled',52)
 # Selection family: connected groups have exact anatomy from official component docs.
 s.rect(816,174,592,294,C['low'],r=28);s.label(840,207,'02 / CONNECTED BUTTON GROUP');s.text(840,241,'Một lựa chọn, nhìn rõ trạng thái',22,weight=700);group(s,840,270,['Chơi','Phân tích','Gia sư'],[180,182,182],0,56);s.text(840,365,'Khe 2dp · góc trong 8dp · góc ngoài tròn',13,C['muted']);s.text(840,393,'Single selection có nhãn; không dùng icon mơ hồ.',13,C['muted']);s.text(840,429,'Pressed / selected đổi shape; reduced motion giữ feedback.',12,C['muted'])
 s.rect(208,490,584,190,C['sage'],r=28);s.label(232,523,'03 / FLOATING TOOLBAR');s.text(232,554,'Thao tác bàn chơi, cùng một cụm',20,C['onsage'],700);s.rect(232,578,536,76,'#C8D9C0',r=38);icon(s,244,592,'↶','white',C['onsage'],48);icon(s,298,592,'↷','white',C['onsage'],48);button(s,354,590,236,'Đặt mảnh →','filled',52);icon(s,602,592,'×','white',C['onsage'],48);icon(s,658,592,'…','white',C['onsage'],48);s.text(232,672,'↶ Xoay trái · ↷ Xoay phải · × Bỏ chọn · … Thêm',10,C['onsage'])
 s.rect(816,490,592,190,C['low'],r=28);s.label(840,523,'04 / CONTAINED LIST');
 for i,(t,sub,on) in enumerate([('Giảm chuyển động','Theo hệ thống / preference',True),('Âm thanh','Preference dùng chung',False)]):
  yy=544+i*60;rr(s,840,yy,544,58,(18,18,4,4) if i==0 else (4,4,18,18),C['high']);s.text(856,yy+24,t,14,weight=600);s.text(856,yy+45,sub,11,C['muted']);s.toggle(1320,yy+16,on)
 s.rect(208,702,584,228,C['low'],r=28);s.label(232,735,'05 / FILLED FIELD + FEEDBACK');s.rect(232,758,536,62,C['high'],r=16);s.text(250,780,'MÃ PHÒNG',10,C['muted'],600);s.text(250,806,'TB-XX',16);s.line(246,819,754,819,'#BA1A1A',2);s.text(232,850,'Mã không hợp lệ. Kiểm tra rồi thử lại.',12,'#BA1A1A');s.text(232,899,'Không có border trang trí. Chỉ focus / lỗi / cấu trúc cần thiết.',11,C['muted'])
 s.rect(816,702,592,228,C['primary'],r=28);s.label(840,735,'06 / THỨ BẬC RÕ RÀNG');s.a[-1]=s.a[-1].replace(C['muted'],'#EAE1FF');s.text(840,791,'48–64dp',38,'white',700);s.text(840,824,'Action target mẫu / tối thiểu canonical 44dp',12,'#EAE1FF');s.text(840,862,'3dp focus · 4 theme · token source duy nhất',14,'white',600);s.text(840,900,'Shape / motion là proposal có consumer, không fork theme.',11,'#EAE1FF')
 return s
if __name__=='__main__':
 for name,fn in [('01-library-m3-expressive',library),('02-foundation-m3-expressive',foundation)]:
  (OUT/(name+'.svg')).write_text(fn().svg(name))
 print('Two M3Expressive direction previews generated')
