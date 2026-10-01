from pathlib import Path
import json, html, re, shutil, zipfile, hashlib
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'dist'
P={'bg':'#F9F7F4','paper':'#FFFFFF','ink':'#1E1C21','muted':'#4B4650','line':'#E7E2E9','primary':'#5634BE','soft':'#EBE4FA','green':'#106F3D','sage':'#DEE8DB','warm':'#F4E9D6','danger':'#BA1A1A','amber':'#B07000','night':'#302F44'}
class Scene:
 def __init__(self,w=1440,h=1000):self.w=w;self.h=h;self.a=[]
 def rect(self,x,y,w,h,fill=None,stroke=None,r=12,sw=1):self.a.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill or P["paper"]}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+'/>')
 def text(self,x,y,t,size=16,fill=None,weight=400,anchor='start',mono=False):self.a.append(f'<text x="{x}" y="{y}" font-family="{("DejaVu Sans Mono" if mono else "DejaVu Sans")},Noto Sans CJK SC,sans-serif" font-size="{size}" font-weight="{weight}" fill="{fill or P["ink"]}" text-anchor="{anchor}">{html.escape(str(t))}</text>')
 def lines(self,x,y,lines,size=15,fill=None,step=24,weight=400):
  for i,t in enumerate(lines):self.text(x,y+i*step,t,size,fill,weight)
 def line(self,x1,y1,x2,y2,c=None,sw=1,dash=''):self.a.append(f'<path d="M{x1} {y1}L{x2} {y2}" fill="none" stroke="{c or P["line"]}" stroke-width="{sw}"'+(f' stroke-dasharray="{dash}"' if dash else '')+'/>')
 def circle(self,x,y,r,fill,stroke=None,sw=1):self.a.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+'/>')
 def pill(self,x,y,t,kind='neutral',w=None):
  colors={'neutral':('#F4EDF7',P['muted']),'current':('#E4F0E5',P['green']),'planned':('#F7ECD9','#775A18'),'selected':(P['soft'],P['primary']),'danger':('#FBE9E7',P['danger'])};bg,fg=colors[kind];w=w or len(t)*7+24;self.rect(x,y,w,26,bg,r=8);self.text(x+12,y+18,t,11,fg,600);return w
 def button(self,x,y,w,t,kind='primary',focus=False):
  bg,fg,st=(P['primary'],'#FFF',None) if kind=='primary' else (P['soft'],P['primary'],None) if kind=='soft' else (P['paper'],P['muted'],P['line']) if kind=='secondary' else ('#F4EDF7','#79747E',None)
  if focus:self.rect(x-5,y-5,w+10,54,'none',P['primary'],r=12,sw=3)
  self.rect(x,y,w,44,bg,st,r=14);self.text(x+w/2,y+28,t,14,fg,600,'middle')
 def label(self,x,y,t):self.text(x,y,t,11,P['muted'],600)
 def field(self,x,y,w,label,value,helper=None,error=False):
  self.label(x,y,label);self.rect(x,y+10,w,44,P['paper'],P['danger'] if error else '#C9C1CD',r=8);self.text(x+14,y+38,value,14,P['muted']);
  if helper:self.text(x,y+72,helper,12,P['danger'] if error else P['muted'])
 def card(self,x,y,w,h,title=None,kicker=None):
  self.rect(x,y,w,h,P['paper'],P['line'],r=16)
  if kicker:self.label(x+20,y+28,kicker)
  if title:self.text(x+20,y+52 if kicker else y+34,title,18,weight=600)
 def row(self,x,y,w,title,sub=None,right=None,status=None):
  self.line(x,y+64,x+w,y+64);self.text(x,y+25,title,14,weight=600)
  if sub:self.text(x,y+47,sub,12,P['muted'])
  if right:self.text(x+w,y+32,right,14,P['muted'],anchor='end',mono=True)
  if status:self.pill(x+w-130,y+17,status,'planned',w=130)
 def check(self,x,y,t,on=True):self.rect(x,y,20,20,P['primary'] if on else P['paper'],P['primary'] if on else '#79747E',r=4);self.text(x+10,y+15,'✓' if on else '',14,'white',600,'middle');self.text(x+32,y+16,t,14)
 def toggle(self,x,y,on=True):self.rect(x,y,44,26,P['primary'] if on else '#DDD7E1',r=13);self.circle(x+31 if on else x+13,y+13,9,'white')
 def svg(self,title):return f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" viewBox="0 0 {self.w} {self.h}" role="img" aria-labelledby="title desc"><title id="title">{html.escape(title)}</title><desc id="desc">Tabula Design 02. Static design reference; sample data. This is not a runtime screenshot.</desc>'+''.join(self.a)+'</svg>'

def shell(s,title,section='Thư viện',future=False):
 s.rect(0,0,s.w,s.h,P['bg'],r=0);s.rect(0,0,176,s.h,P['paper'],r=0);s.line(176,0,176,s.h);s.rect(20,18,34,34,P['primary'],r=10);s.text(37,43,'T.',23,'white',700,'middle');s.text(64,43,'tabula',23,weight=700)
 nav=[('▦','Thư viện'),('↺','Ván của tôi'),('♙','Phòng chơi'),('♡','Bạn bè')]
 for i,(ico,t) in enumerate(nav):
  y=88+i*52
  if t==section:s.rect(12,y,152,44,P['soft'],r=8)
  s.text(27,y+29,ico,22,P['primary'] if t==section else P['muted']);s.text(58,y+28,t,13,P['primary'] if t==section else P['muted'],600 if t==section else 400)
 s.label(24,344,'TRUY CẬP NHANH');s.text(27,380,'♞',22,P['green']);s.text(58,377,'Cờ vua',13);s.text(27,424,'帥',22,P['danger']);s.text(58,421,'Xiangqi',13);s.pill(58,435,'Định hướng','planned',w=96)
 s.line(16,s.h-126,160,s.h-126);s.rect(12,s.h-112,152,44,P['soft'] if section=='Cài đặt' else P['paper'],r=8);s.text(28,s.h-84,'⚙',20,P['muted']);s.text(58,s.h-84,'Cài đặt',13);s.circle(36,s.h-32,16,P['soft']);s.text(36,s.h-27,'B',10,P['primary'],600,'middle');s.text(61,s.h-34,'Bạn',12,weight=600);s.text(61,s.h-16,'Hồ sơ minh họa',10,P['muted'])
 s.rect(177,0,s.w-177,64,P['paper'],r=0);s.line(176,64,s.w,64);s.text(204,39,'Không gian chơi  /  '+title,13,P['muted']);s.pill(1080,19,'DESIGN 02','neutral',w=116);s.text(1210,38,'Dữ liệu mẫu',12,P['muted']);s.circle(1387,32,18,P['soft']);s.text(1387,37,'B',11,P['primary'],600,'middle')
 s.text(208,108,title,26,weight=700);s.pill(1204,85,'Dự kiến · có gate' if future else 'Local · có trong code','planned' if future else 'current',w=204);s.text(208,s.h-20,'Tabula / Design 02 · Bản thiết kế tĩnh, không phải ảnh runtime',11,P['muted']);s.text(1408,s.h-20,'01.10.2026',11,P['muted'],anchor='end',mono=True)

def chess(s,x,y,size=560):
 s.rect(x-8,y-8,size+16,size+16,'#5C6B59',r=12);a=size/8
 pos=['r.bq.rk.','ppp..ppp','..np.n..','..b.p...','..B.P...','..NP.N..','PPP..PPP','R.BQ.RK.']
 glyph={'p':'♟','r':'♜','n':'♞','b':'♝','q':'♛','k':'♚','P':'♟','R':'♜','N':'♞','B':'♝','Q':'♛','K':'♚'}
 for r in range(8):
  for c in range(8):
   color='#D9D38D' if (r,c) in [(0,4),(0,6)] else '#EEE7DA' if (r+c)%2==0 else '#9AA996';s.rect(x+c*a,y+r*a,a,a,color,r=0)
   if pos[r][c]!='.':s.text(x+(c+.5)*a,y+(r+.78)*a,glyph[pos[r][c]],a*.79,'#FFFCF3' if pos[r][c].isupper() else '#343440',500,'middle');s.a[-1]=s.a[-1].replace('<text ', '<text paint-order="stroke fill" stroke="#606C58" stroke-width="1.6" ')
 for c in range(8):s.text(x+(c+.5)*a,y+size+22,'abcdefgh'[c],11,P['muted'],anchor='middle',mono=True)
 for r in range(8):s.text(x-18,y+(r+.55)*a,8-r,11,P['muted'],anchor='middle',mono=True)
 s.rect(x+5*a+2,y+5*a+2,a-4,a-4,'none',P['primary'],r=0,sw=3)
 for c,r in [(3,4),(6,3),(7,4),(4,7)]:s.circle(x+(c+.5)*a,y+(r+.5)*a,8,P['green'])
 s.circle(x+4.5*a,y+3.5*a,a*.43,'none',P['green'],3)

def xq(s,x,y,w=484,h=570,arrow=False):
 s.rect(x,y,w,h,P['warm'],'#CEBA97',r=12,sw=6);sx=w/10;sy=h/12;xx=x+sx;yy=y+sy
 for r in range(10):s.line(xx,yy+r*sy,xx+8*sx,yy+r*sy,'#89775E')
 for c in range(9):
  if c in [0,8]:s.line(xx+c*sx,yy,xx+c*sx,yy+9*sy,'#89775E')
  else:s.line(xx+c*sx,yy,xx+c*sx,yy+4*sy,'#89775E');s.line(xx+c*sx,yy+5*sy,xx+c*sx,yy+9*sy,'#89775E')
 for r in [0,7]:s.line(xx+3*sx,yy+r*sy,xx+5*sx,yy+(r+2)*sy,'#89775E');s.line(xx+5*sx,yy+r*sy,xx+3*sx,yy+(r+2)*sy,'#89775E')
 s.text(xx+2*sx,yy+4.65*sy,'楚 河',24,'#89775E',anchor='middle');s.text(xx+6*sx,yy+4.65*sy,'漢 界',24,'#89775E',anchor='middle')
 pieces=[]
 for r,team,chars in [(0,'black','車馬象士將士象馬車'),(9,'red','俥傌相仕帥仕相傌俥')]:
  for c,ch in enumerate(chars):pieces.append((c,r,ch,team))
 for r,team in [(2,'black'),(7,'red')]:
  for c in [1,7]:pieces.append((c,r,'砲' if team=='black' else '炮',team))
 for r,team in [(3,'black'),(6,'red')]:
  for c in [0,2,4,6,8]:pieces.append((c,r,'卒' if team=='black' else '兵',team))
 for c,r,ch,team in pieces:
  c=4 if arrow and team=='red' and r==7 and c==7 else c
  px=xx+c*sx;py=yy+r*sy;s.circle(px,py+2,sx*.41,'#CEBA97');s.circle(px,py,sx*.41,'#FFF6E5','#B79461');s.circle(px,py,sx*.34,'none','#C4A57C',.7);s.text(px,py+sx*.2,ch,sx*.58,P['danger'] if team=='red' else '#343332',600,'middle')
 if arrow:s.line(xx+7*sx,yy+7*sy,xx+4*sx,yy+7*sy,P['primary'],5);s.circle(xx+4*sx,yy+7*sy,7,P['primary'])
 for c in range(9):s.text(xx+c*sx,y+h-14,9-c,10,P['muted'],anchor='middle',mono=True)

def tiles(s,x,y,w=680,h=574):
 s.rect(x,y,w,h,'#E8EEE5',P['line'],r=12)
 for ix in range(0,int(w),70):s.line(x+ix,y,x+ix,y+h,'#D7E2D3')
 for iy in range(0,int(h),70):s.line(x,y+iy,x+w,y+iy,'#D7E2D3')
 cells=[(1,0),(2,0),(3,0),(0,1),(1,1),(2,1),(3,1),(4,1),(1,2),(2,2),(3,2),(2,3)]
 for i,(c,r) in enumerate(cells):
  xx=x+150+c*74;yy=y+86+r*74;s.rect(xx,yy,72,72,'#C9D5B8' if i%2 else '#BBCDAF','#A5B69C',r=4)
  if i%3==0:s.rect(xx,yy,72,20,'#C79872',r=4)
  s.line(xx+36,yy,xx+36,yy+72,'#F8ECD1',11)
  if i%2:s.line(xx+36,yy+36,xx+72,yy+36,'#F8ECD1',11)
  s.circle(xx+16,yy+52,6,'#76916A')
  if i in [3,6,9]:s.text(xx+36,yy+48,'♟',32,P['primary'] if i==3 else '#AC3D37' if i==6 else P['green'],600,'middle')
 xx=x+150+4*74;yy=y+86+2*74;s.rect(xx,yy,72,72,'#F2F5EF',P['primary'],r=4,sw=2);s.text(xx+36,yy+45,'+',30,P['primary'],600,'middle');s.pill(x+18,y+18,'Vị trí hợp lệ: 3','current',w=148);s.text(x+w-24,y+32,'N ↑',14,P['muted'],anchor='end');s.text(x+20,y+h-20,'Kéo để di chuyển · +/- để thu phóng · R xoay mảnh',12,P['muted'])

def moves(s,x,y,w=330,title='Nước đi',xqmode=False):
 s.card(x,y,w,348,title);s.pill(x+w-98,y+16,'Ván mẫu',w=80);s.label(x+20,y+68,'LƯỢT');s.label(x+75,y+68,'ĐỎ' if xqmode else 'TRẮNG');s.label(x+206,y+68,'ĐEN')
 rows=[('P2–5','M8.7'),('M2.3','X9–8'),('X1–2','B7.1'),('B7.1','P8.4'),('M8.7','M2.3'),('X9–8','X1–2')] if xqmode else [('e4','e5'),('Nf3','Nc6'),('Bc4','Nf6'),('d3','Bc5'),('O–O','d6'),('Nc3','O–O')]
 if xqmode:rows=[];s.text(x+20,y+113,'Chưa có nước đi',14,P['muted']);s.text(x+20,y+143,'Thế ban đầu · Đỏ đi trước',13,P['muted'])
 for i,(a,b) in enumerate(rows):
  yy=y+91+i*33
  if i==5:s.rect(x+12,yy-20,w-24,32,P['soft'],r=6)
  s.text(x+24,yy,str(i+1)+'.',12,P['muted'],mono=True);s.text(x+75,yy,a,13,P['primary'] if i==5 else None,mono=True);s.text(x+206,yy,b,13,P['primary'] if i==5 else None,mono=True)
 s.line(x+20,y+284,x+w-20,y+284)
 for i,t in enumerate(['|‹','‹','›','›|']):s.button(x+20+i*73,y+296,62,t,'secondary')

def gamecard(s,x,y,w,title,kind,status,sub):
 s.card(x,y,w,222);s.rect(x+14,y+14,w-28,100,P['soft'] if kind=='xq' else P['sage'] if kind in ['chess','tiles'] else '#F5E1D4' if kind=='caro' else P['night'],r=8)
 if kind=='chess':s.text(x+w/2,y+91,'♞  ♔',68,'#4D6249',600,'middle')
 elif kind=='xq':
  for dx,ch,col in [(-34,'帥',P['danger']),(34,'馬','#343332')]:s.circle(x+w/2+dx,y+65,33,'#FFF6E5','#BCA37A');s.text(x+w/2+dx,y+80,ch,38,col,600,'middle')
 elif kind=='tiles':
  for i in range(3):s.rect(x+w/2-63+i*39,y+42+i*8,45,45,'#C9D5B8','#7E956F',r=4);s.line(x+w/2-42+i*39,y+43+i*8,x+w/2-42+i*39,y+87+i*8,'#F8ECD1',8)
 elif kind=='caro':s.text(x+w/2,y+91,'×  ○',66,'#A66747',600,'middle')
 else:s.circle(x+w/2,y+57,27,'#EEE1B2');s.text(x+w/2,y+101,'▲ ▲ ▲',34,'#232331',600,'middle')
 s.text(x+20,y+142,title,17,weight=600);s.pill(x+20,y+154,status,'current' if status=='Chơi local' else 'planned',w=128);s.text(x+20,y+201,sub,12,P['muted']);s.text(x+w-20,y+145,'♡',20,P['primary'],anchor='end')

def library(s):
 shell(s,'Thư viện game');s.text(208,138,'Chọn trò chơi. Bắt đầu ngay trên máy này.',14,P['muted']);s.field(972,108,436,'TÌM TRÒ CHƠI','⌕  Tìm theo tên hoặc thể loại')
 s.card(208,174,1200,76);s.circle(246,211,21,P['sage']);s.text(246,218,'♞',28,P['green'],anchor='middle');s.text(282,206,'Tiếp tục cờ vua',16,weight=600);s.text(282,230,'Ván local · đến lượt Trắng · lưu trên thiết bị',12,P['muted']);s.text(1161,218,'09:42',21,weight=600,mono=True);s.button(1234,189,154,'Tiếp tục →')
 for i,t in enumerate(['Tất cả','Chơi local','Chiến thuật','Hội nhóm','Yêu thích']):s.button(208+i*148,272,136,t,'soft' if i==0 else 'secondary')
 games=[('Cờ vua','chess','Chơi local','2 người · Chess presenter có thật'),('Miền đất nhỏ','tiles','Chơi local','2–5 người · Tiles presenter có thật'),('Cờ tướng','xq','Định hướng #48','Chơi · Phân tích · Gia sư dự kiến'),('Caro','caro','Adapter chưa đủ','Rules/UI đang phát triển'),('Ma sói','wolf','Adapter chưa đủ','6–12 người · màn minh họa')]
 for i,g in enumerate(games):gamecard(s,208+(i%3)*410,338+(i//3)*242,380,*g)
 s.card(1028,580,380,222,'Chơi local không cần tài khoản');s.lines(1048,636,['Tài khoản, phòng và ghép trận chỉ','mở khi service tương ứng sẵn sàng.','Không dùng số liệu mẫu làm trạng thái thật.'],13,P['muted']);s.button(1048,736,340,'Xem trạng thái khả năng','secondary')
 s.text(208,857,'Mật độ gọn: 5 game và trạng thái khả năng trong một màn hình',12,P['muted']);s.text(208,887,'Tìm kiếm, filter và yêu thích là UI local; badge đến từ CapabilitySet.',12,P['muted'])

def detail(s):
 shell(s,'Cờ vua · Chi tiết game');s.text(208,141,'Thư viện / Cờ vua',13,P['muted']);s.card(208,170,1200,192);s.rect(226,188,220,156,P['sage'],r=8);s.text(336,292,'♞ ♔',70,'#4D6249',600,'middle');s.text(470,213,'Một bàn cờ, một ván tập trung',22,weight=700);s.lines(470,244,['Luật và bàn chơi từ module Chess. Chơi cùng người khác','trên một máy; không cần đăng nhập.'],14,P['muted']);s.pill(470,298,'Chơi local','current',w=116);s.pill(598,298,'2 người',w=94);s.pill(704,298,'Có replay API',w=144);s.button(1124,270,264,'Thiết lập ván local →');s.button(1124,206,264,'Online · chưa mở','disabled')
 s.card(208,388,788,394,'Chọn cách chơi');s.row(228,434,748,'Cùng một máy','Hai người luân phiên trên bàn local','Sẵn sàng');s.row(228,510,748,'Đấu với bot','Chỉ mở nếu module/provider công bố hỗ trợ',status='Có gate');s.row(228,586,748,'Online','Chỉ mở sau service/protocol và phase exit',status='Chưa mở');s.text(228,701,'Không suy hỗ trợ từ bản thiết kế.',14,weight=600);s.text(228,728,'Đọc metadata + CapabilitySet của module đang nạp.',13,P['muted'])
 s.card(1020,388,388,394,'Luật & tài nguyên');s.row(1040,434,348,'Hướng dẫn cơ bản','Nội dung versioned theo module');s.row(1040,510,348,'Phím tắt','Di chuyển focus / chọn / quay lại');s.row(1040,586,348,'Nguồn module','Chess · engine/version từ manifest');s.button(1040,714,348,'Xem hướng dẫn','secondary')
 s.card(208,810,1200,92);s.label(228,839,'TRẠNG THÁI THAY THẾ');s.text(228,869,'Không nạp được module → lỗi inline + Thử lại · capability thiếu → CTA có lý do',14,P['muted'])

def setup(s):
 shell(s,'Thiết lập ván local');s.text(208,141,'Cờ vua / Ván mới · cấu hình mẫu',13,P['muted']);s.card(208,174,788,652,'Một cấu hình rõ ràng trước khi chơi');s.label(232,244,'CHẾ ĐỘ');s.button(232,259,228,'Cùng một máy','soft');s.button(476,259,228,'Bot · có gate','disabled');s.button(720,259,252,'Online · chưa mở','disabled')
 s.field(232,341,740,'NGƯỜI CHƠI TRẮNG','Bạn / Trắng');s.field(232,439,740,'NGƯỜI CHƠI ĐEN','Người chơi 2 / Đen');s.label(232,549,'THỜI GIAN · GIÁ TRỊ MINH HỌA')
 for i,t in enumerate(['Không giới hạn','5 + 0','10 + 5','15 + 10']):s.button(232+i*188,564,176,t,'soft' if i==2 else 'secondary')
 s.check(232,650,'Hiển thị ô hợp lệ và nước vừa đi',True);s.check(232,694,'Âm thanh nước đi',True);s.text(232,766,'Clock/time control chỉ bật khi module cung cấp timer thật.',13,P['muted'])
 s.card(1020,174,388,392,'Tóm tắt ván');
 for i,(a,b) in enumerate([('Game','Cờ vua'),('Chế độ','Local / cùng máy'),('Bên đi trước','Trắng'),('Timer','10 + 5 · mẫu'),('Lưu replay','Theo adapter')]):s.text(1044,245+i*48,a,13,P['muted']);s.text(1384,245+i*48,b,14,anchor='end',weight=600)
 s.button(1040,497,348,'Bắt đầu ván local →');s.card(1020,588,388,238,'Lỗi cấu hình');s.lines(1040,641,['Không thể khởi tạo ván.','Giữ cấu hình vừa chọn để thử lại.','Không chuyển sang game khác tự động.'],14,P['muted']);s.button(1040,740,348,'Thử lại','secondary')
 s.text(208,875,'Mobile: form một cột, tóm tắt thu gọn; CTA 44dp luôn nằm sau validation',13,P['muted'])

def gameplay(s,kind='chess'):
 title='Cờ vua · Bàn chơi' if kind=='chess' else 'Miền đất nhỏ · Bàn chơi';shell(s,title);s.text(208,140,'Local / '+('2 người cùng máy' if kind=='chess' else 'Đặt mảnh · consumer HUD hiện có'),13,P['muted']);s.button(1136,122,130,'Hướng dẫn','secondary');s.button(1278,122,130,'Cài đặt','secondary')
 if kind=='chess':
  s.text(248,198,'Minh Anh · Đen',14,weight=600);s.rect(681,171,147,44,'#F4EDF7',r=8);s.text(754,202,'08:16',26,weight=600,anchor='middle',mono=True);chess(s,258,236,560);s.text(248,865,'Bạn · Trắng · đến lượt',14,P['green'],600);s.rect(681,836,147,50,P['ink'],r=8);s.text(754,870,'09:42',28,'white',600,'middle',mono=True);moves(s,862,174,546)
  s.card(862,542,546,86);s.circle(888,577,5,P['green']);s.text(906,581,'Đến lượt Trắng',16,P['green'],600);s.text(882,608,'Mã f3 được chọn · ô hợp lệ minh họa',13,P['muted']);s.card(862,648,546,156,'Trạng thái bàn đọc');s.lines(882,704,['Ô focus: f3 · Mã trắng','Thông báo lượt và nước đi dùng cả chữ + dấu hiệu.','BoardReader/keyboard parity là tiêu chí implementation.'],13,P['muted']);s.button(862,826,168,'Lật bàn','secondary');s.button(1042,826,168,'Toàn màn','secondary');s.button(1222,826,186,'Kết thúc ván','secondary')
 else:
  tiles(s,208,180,796,574);s.card(208,776,796,122,'Chọn mảnh → xoay → xác nhận');s.button(232,826,174,'↶  Xoay trái','secondary');s.button(418,826,174,'↷  Xoay phải','secondary');s.button(604,826,192,'Đặt mảnh');s.button(810,826,174,'Bỏ chọn','secondary');s.card(1028,180,380,208,'Mảnh hiện tại');s.rect(1048,238,110,110,'#C9D5B8',r=6);s.line(1103,238,1103,348,'#F8ECD1',13);s.text(1180,265,'Đường thẳng',16,weight=600);s.text(1180,294,'Góc: 0° · R xoay',13,P['muted']);s.text(1180,324,'Còn 42 mảnh · mẫu',13,P['muted']);s.card(1028,410,380,276,'Người chơi & điểm mẫu');
  for i,(a,b) in enumerate([('Bạn · đến lượt','18'),('Minh Anh','14'),('Hoàng','12')]):s.row(1048,454+i*66,340,a,'7 meeple · mẫu' if i==0 else 'Đang chờ',b)
  s.card(1028,710,380,188,'HUD dùng chung');s.lines(1048,764,['Target tăng 34 → 44dp.','Điểm, lượt và mảnh đọc được bằng chữ.','Hành động bị chặn giữ focus + lý do.'],13,P['muted']);s.button(1048,826,340,'Xem cách tính điểm','secondary')

def companion(s):
 shell(s,'Game adapter & trạng thái',future=True);s.text(208,139,'Caro / Ma sói chỉ là thiết kế adapter. Loading/recovery không giả lập server thành công.',13,P['muted'])
 s.card(208,174,576,414,'Caro · màn 05');s.pill(618,191,'Chưa playable','planned',w=146);s.rect(228,230,272,272,'#F5E1D4',r=8)
 for i in range(12):s.line(240+i*22,240,240+i*22,491,'#9F7254');s.line(240,240+i*22,491,240+i*22,'#9F7254')
 for i,(c,r,t) in enumerate([(5,5,'×'),(6,5,'○'),(5,6,'○'),(6,6,'×'),(7,6,'×')]):s.text(240+c*22,247+r*22,t,26,P['primary'] if t=='×' else '#A66747',600,'middle')
 s.lines(523,255,['Lượt X / Lượt O','Ô cuối + điểm focus','Undo theo capability','Không hiển thị bot thật'],13,P['muted'],step=30);s.button(228,526,536,'Luật + adapter phải qua gate trước','disabled')
 s.card(808,174,600,414,'Ma sói · màn 07');s.pill(1240,191,'Rules/UI stub','planned',w=146);s.rect(828,234,174,206,P['night'],r=12);s.circle(914,295,32,'#EEE1B2');s.text(914,375,'Vai trò riêng',15,'white',600,'middle');s.text(914,404,'Chỉ người chơi này thấy',10,'#E7E1E5',anchor='middle');s.text(1030,262,'Pha ngày · thảo luận',17,weight=600);s.lines(1030,295,['Danh sách sống / chết','Phiếu bầu theo quyền','Không tiết lộ vai trò người khác','Thông báo phase từ authority'],13,P['muted'],step=29);s.button(828,464,174,'Xem vai trò','secondary');s.button(1026,464,360,'Biểu quyết · có gate','disabled');s.text(828,555,'State UI riêng tư nằm ngoài canonical public view.',12,P['muted'])
 s.card(208,612,372,258,'Đang chuẩn bị bàn');s.text(228,668,'Nạp module đã xác minh',14,P['muted']);s.rect(228,693,332,8,'#F4EDF7',r=4);s.rect(228,693,172,8,P['primary'],r=4);s.text(228,730,'2 / 3 tài nguyên · ví dụ',13,P['muted']);s.button(228,793,332,'Hủy','secondary')
 s.card(604,612,384,258,'Kết nối gián đoạn');s.pill(624,662,'Đang nối lại','planned',w=146);s.lines(624,722,['Khóa input nước đi đến khi resync.','Giữ snapshot cuối, không chuyển local.'],13,P['muted']);s.button(624,793,170,'Thử lại','secondary');s.button(806,793,162,'Về phòng','secondary')
 s.card(1012,612,396,258,'Không thể khởi tạo');s.lines(1032,668,['Không có tài nguyên tương thích.','Hiển thị mã lỗi, nguyên nhân và retry.','Không chỉ ghi stderr.'],13,P['muted']);s.button(1032,793,356,'Thử lại','secondary')

def history(s):
 shell(s,'Ván của tôi','Ván của tôi',True);s.text(208,140,'Replay domain đã có; danh sách lưu/đồng bộ cần adapter thật.',13,P['muted']);s.button(1220,126,188,'Nhập replay','secondary');s.card(208,180,1200,694);s.field(232,218,640,'TÌM VÁN','⌕  Tìm theo game hoặc người chơi');s.button(896,228,232,'Tất cả game','secondary');s.button(1140,228,244,'Gần nhất','secondary');s.label(232,321,'GAME / NGƯỜI CHƠI');s.label(696,321,'CHẾ ĐỘ');s.label(886,321,'KẾT QUẢ');s.label(1050,321,'LƯỢT / NGÀY');s.line(232,338,1384,338)
 data=[('Cờ vua','Bạn · Minh Anh','Local','Trắng thắng','42','01.10 · 03:12'),('Miền đất nhỏ','Bạn · Hoàng · Minh Anh','Local','Bạn 28 điểm','36','30.09 · 19:20'),('Cờ vua','Bạn · Minh Anh','Local','Hòa','58','29.09 · 21:06')]
 for i,(g,n,m,r,t,d) in enumerate(data):
  y=371+i*100;s.circle(257,y+15,21,P['sage']);s.text(257,y+22,'♞' if g=='Cờ vua' else '▧',25,P['green'],anchor='middle');s.text(292,y+8,g,16,weight=600);s.text(292,y+34,n,12,P['muted']);s.text(696,y+15,m,14);s.pill(886,y-4,r,'current' if i==0 else 'neutral',w=144);s.text(1050,y+9,t+' lượt',13,mono=True);s.text(1050,y+33,d,12,P['muted'],mono=True);s.button(1252,y-8,132,'Xem lại →','secondary');s.line(232,y+65,1384,y+65)
 s.card(232,736,544,108,'Chưa có ván');s.text(252,790,'Giải thích nơi lưu, mở “Bắt đầu ván local”.',13,P['muted']);s.card(800,736,584,108,'Replay không tương thích');s.text(820,790,'Giữ metadata + báo game/version, không đoán parse.',13,P['muted'])

def result(s):
 shell(s,'Kết quả ván','Ván của tôi',True);s.card(208,180,1200,174);s.pill(232,205,'Cờ vua · Local',w=162);s.text(232,273,'Trắng thắng · chiếu hết',30,weight=700);s.text(232,312,'42 lượt · 18 phút 24 giây · kết quả và thời gian mẫu',14,P['muted']);s.button(1108,220,276,'Xem replay →');s.button(1108,276,276,'Chơi ván mới','secondary');s.card(208,382,576,390,'Hai bên');s.circle(260,468,26,P['soft']);s.text(260,476,'B',14,P['primary'],600,'middle');s.text(308,463,'Bạn · Trắng',18,weight=600);s.text(308,490,'Thắng',14,P['green']);s.row(232,527,528,'Minh Anh · Đen','Cùng một máy','Thua');s.row(232,603,528,'Lý do kết thúc','Nguồn result event của module','Chiếu hết');s.card(808,382,600,390,'Tóm tắt có thể tin được');s.lines(832,449,['Kết quả lấy từ GameResult / event của module.','Không tự tính accuracy/Elo nếu provider chưa có.','Không dùng bảng demo làm bằng chứng chất lượng AI.'],14,P['muted'],step=30);s.button(832,666,552,'Lưu / xuất replay · adapter có gate','disabled');s.card(208,804,1200,92);s.text(232,846,'Các trạng thái: hòa · đầu hàng · timeout · aborted · lỗi lưu',15,weight=600);s.text(232,873,'Nếu không lưu được: giữ result, cho retry; CTA xuất luôn nêu format + version.',13,P['muted'])

def replay(s):
 shell(s,'Replay · Nhánh phân tích','Ván của tôi',True);s.text(208,140,'Bản ghi gốc chỉ đọc. Nhánh thử riêng, không sửa ván đã kết thúc.',13,P['muted']);s.pill(208,171,'Cờ vua · mẫu',w=144);s.pill(368,171,'Ply 12 / 84','selected',w=130);chess(s,252,238,540);moves(s,848,180,560);s.card(848,548,560,306,'Timeline & nhánh');s.text(872,606,'Bản ghi gốc',15,weight=600);s.line(887,650,1367,650,'#D1C8DC',4)
 for i in range(7):s.circle(887+i*80,650,8,P['primary'] if i<=3 else '#D1C8DC');s.text(887+i*80,684,i*7,11,P['muted'],anchor='middle',mono=True)
 s.text(872,726,'Nhánh thử từ ply 12 · chưa lưu',13,P['primary']);s.button(872,780,248,'Tạo nhánh từ vị trí','secondary');s.button(1134,780,248,'Về bản ghi gốc','secondary');s.button(208,843,148,'|‹  Đầu','secondary');s.button(368,843,148,'‹  Trước','secondary');s.button(528,843,148,'Tiếp  ›','secondary');s.button(688,843,148,'Cuối  ›|','secondary');s.text(208,920,'← / →: bước lượt · Home / End: đầu/cuối · số ply được thông báo cho AT',12,P['muted'])

def xiangqi(s,mode='play'):
 shell(s,'Xiangqi · '+{'play':'Chơi','analysis':'Phân tích','learn':'Gia sư'}[mode],future=True);s.text(208,140,'Định hướng #48 · engine / network / AI chưa có trong code hiện tại',13,P['muted']);
 for i,t in enumerate(['Chơi','Phân tích','Gia sư']):s.button(208+i*164,164,152,t,'soft' if ['play','analysis','learn'][i]==mode else 'secondary')
 s.button(1086,164,158,'Tài nguyên','secondary');s.button(1256,164,152,'Nhập thế cờ','secondary');xq(s,254,234,510,590,mode!='play')
 if mode=='play':
  s.text(254,877,'Bạn · Đỏ · nước đi mẫu',14,P['danger'],600);s.rect(654,842,110,48,P['ink'],r=8);s.text(709,874,'09:42',23,'white',600,'middle',mono=True);moves(s,816,234,592,xqmode=True);s.card(816,602,592,138,'Đối thủ & luật');s.lines(836,656,['Local sau khi có game module Xiangqi.','Bot chỉ mở khi EngineProvider cài/kiểm định thành công.','Không gọi mock thành engine thật.'],13,P['muted']);s.button(816,760,284,'Đấu bot · chưa có','disabled');s.button(1112,760,296,'Luật & notation','secondary');s.card(816,826,592,76);s.text(836,856,'Khi provider lỗi: giữ bàn + engine offline badge',13,weight=600);s.text(836,881,'Không tự tạo nước đi hoặc thành công giả.',12,P['muted'])
 elif mode=='analysis':
  s.card(816,234,592,164,'Engine · chưa kết nối');s.pill(1196,253,'Kết quả mẫu','planned',w=192);s.text(836,296,'Không có evaluation thật',20,weight=600);s.text(836,326,'PV, depth, nodes, elapsed và provider/version là bắt buộc.',12,P['muted']);s.button(836,346,552,'Chọn EngineProvider · có gate','disabled');s.card(816,420,592,240,'Biến thể & bằng chứng');s.label(836,479,'PV MẪU');s.text(836,516,'M8.7  M2.3  X9–8 · sau P2–5',18,P['primary'],mono=True);s.lines(836,556,['Mỗi nhận định liên kết ply + PV + vị trí trên bàn.','Replay core và nhánh thử thuộc pack 04.','Không kết luận “tốt nhất” từ dữ liệu mock.'],13,P['muted']);s.card(816,682,592,220,'Ngân sách tính toán');s.field(836,737,552,'CPU / THREAD / TIME','25% CPU · 1 thread · tối đa 5s · ví dụ');s.button(836,834,264,'Bắt đầu · có gate','disabled');s.button(1112,834,276,'Hủy tác vụ','secondary')
 else:
  s.card(816,234,592,516,'Gia sư · hội thoại có căn cứ');s.pill(1190,253,'Dữ liệu mẫu','planned',w=198);s.rect(836,294,552,68,P['soft'],r=8);s.lines(852,322,['Vì sao nước P2–5 được cân nhắc?','Câu hỏi minh họa gắn với ply 1.'],13,P['primary']);s.lines(836,405,['Lời giải thật phải dựa trên luật + vị trí + output engine.','Trong bản mẫu, không có model hay engine chạy.','Nếu thiếu dữ kiện, gia sư phải nói chưa đủ cơ sở.'],14,P['muted'],step=28);s.button(836,505,552,'Xem PV minh họa trên bàn','secondary');s.card(836,570,552,150,'Bằng chứng đi kèm');s.text(856,620,'Ply 1 · Position hash · Provider/version',13,P['muted']);s.text(856,649,'PV / depth / elapsed · nguồn bài học',13,P['muted']);s.text(856,678,'Không có bằng chứng → không đưa khẳng định mạnh',12,P['danger']);s.field(816,790,592,'CÂU HỎI CHO VỊ TRÍ NÀY','Hỏi về một nước đi…');s.button(816,862,592,'Gửi · chưa tích hợp AI','disabled')

def resources(s):
 shell(s,'Xiangqi · Tài nguyên',future=True);s.text(208,140,'Engine/model/bài học là proposal #48; danh mục sau đây là metadata minh họa.',13,P['muted']);s.card(208,180,788,544,'EngineProvider & tài nguyên');s.label(232,249,'TÊN / LOẠI');s.label(663,249,'PHIÊN BẢN / DUNG LƯỢNG');s.line(232,270,972,270);s.row(232,284,740,'Xiangqi engine','Provider chưa được lựa chọn','Chưa cài');s.row(232,366,740,'Mạng đánh giá','License / checksum / compatibility phải kiểm tra','Có gate');s.row(232,448,740,'Bài học cơ bản','Nguồn + version + quyền phân phối','Có gate');s.lines(232,570,['Trước download: nguồn chính thức, license, dung lượng, checksum.','Không cài binary/model thật từ prototype.','Hủy/retry giữ trạng thái và không báo thành công sớm.'],13,P['muted']);s.button(232,653,740,'Tải / cài · chưa chọn provider','disabled');s.card(1020,180,388,544,'Thiết bị & giới hạn');
 for i,(a,b) in enumerate([('Nền tảng','Theo adapter'),('Ngân sách CPU','25% · mẫu'),('RAM dự kiến','Chưa xác minh'),('Lưu offline','Chưa có'),('Phiên bản engine','Chưa cài')]):s.row(1040,232+i*74,348,a,None,b)
 s.card(208,748,1200,148,'Trạng thái tài nguyên');
 for i,t in enumerate(['Chưa cài','Đang tải · hủy','Xác minh checksum','Sẵn sàng','Lỗi · thử lại']):s.pill(232+i*224,807,t,'current' if i==3 else 'planned' if i==4 else 'neutral',w=206)
 s.text(232,871,'Chỉ hiện “Sẵn sàng” sau xác minh, tương thích và probe provider thành công.',13,P['muted'])

def settings(s):
 shell(s,'Cài đặt','Cài đặt',True);s.text(208,140,'Thiết kế preference; persistence/runtime parity cần adapter phù hợp.',13,P['muted']);s.card(208,180,788,668,'Giao diện & khả năng tiếp cận');s.label(232,246,'THEME · TÁI DÙNG TOKENS.TOML');
 for i,(t,bg,fg) in enumerate([('Sáng','#FFFBFF','#5634BE'),('Tối','#141217','#CFBCFF'),('HC sáng','#FFFFFF','#2C0078'),('HC tối','#000000','#E0D3FF')]):
  x=232+i*184;s.rect(x,263,168,126,bg,'#79747E',r=8);s.rect(x+14,279,140,34,fg,r=5);s.text(x+84,302,'Aa  09:42',14,bg,600,'middle',True);s.text(x+14,363,t,14,fg,600)
 for i,(a,b,on) in enumerate([('Theo theme hệ thống','Khi chưa override preference',True),('Tương phản cao','ThemeKind HC light / HC dark',False),('Giảm chuyển động','Tôn trọng prefers-reduced-motion',True),('Âm thanh','Không thay canonical game state',True),('Mật độ gọn','Giảm layout chrome, giữ target 44dp',True)]):
  y=419+i*74;s.row(232,y,740,a,b);s.toggle(928,y+20,on)
 s.card(1020,180,388,290,'Xem trước');s.rect(1040,238,348,74,P['soft'],r=8);s.text(1060,267,'Đến lượt bạn',16,P['primary'],600);s.text(1060,292,'Chữ + dấu hiệu, không chỉ màu',12,P['primary']);s.button(1048,358,332,'Bắt đầu ván',focus=True);s.text(1040,441,'Focus 3dp + offset 2dp / target 44dp',12,P['muted']);s.card(1020,492,388,356,'Thiết lập trong ván');s.lines(1040,552,['Overlay gọn dùng cùng preference.','Không nhân đôi màn settings đầy đủ.','Account/privacy chỉ hiện khi adapter có.'],13,P['muted'],step=28);s.row(1040,656,348,'Ô hợp lệ','Dùng marker ngoài màu');s.row(1040,732,348,'BoardReader','Theo hỗ trợ platform, không giả có')

def showcase(s):
 shell(s,'Bộ thành phần · Spec','Cài đặt',True);s.text(208,140,'Showcase/spec màn 22, tách khỏi trang settings màn 13. Không phải framework mới.',13,P['muted']);s.card(208,180,588,260,'Action & focus');s.button(228,240,168,'Primary');s.button(408,240,168,'Secondary','secondary');s.button(588,240,188,'Disabled','disabled');s.button(234,319,168,'Focused',focus=True);s.button(414,319,168,'Busy…','soft');s.button(594,319,44,'⚙','secondary');s.text(650,347,'Icon 44×44',12,P['muted']);s.text(228,409,'Default / hover / pressed / disabled / focus / busy',12,P['muted']);s.card(820,180,588,260,'Feedback & capability');s.pill(840,241,'Local sẵn sàng','current',w=166);s.pill(1020,241,'Có gate','planned',w=140);s.pill(1174,241,'Không tương thích','danger',w=214);s.rect(840,294,548,100,'#FBE9E7',r=8);s.text(860,325,'Không nạp được module',16,P['danger'],600);s.text(860,355,'Giữ cấu hình · mã lỗi · Thử lại rõ ràng',13,P['danger']);s.card(208,464,588,272,'Field & validation');s.field(228,527,548,'TÊN PHÒNG','Một ván cùng bạn','2–60 ký tự · validation inline');s.field(228,636,548,'MÃ PHÒNG','TB-XX','Mã không hợp lệ. Kiểm tra rồi thử lại.',True);s.card(820,464,588,272,'Dialog & list');s.rect(840,522,548,190,P['bg'],P['line'],r=8);s.text(860,557,'Rời ván đang chơi?',18,weight=600);s.text(860,589,'Hậu quả và lựa chọn đều rõ ràng.',13,P['muted']);s.button(860,643,245,'Ở lại','secondary');s.button(1119,643,249,'Rời ván');s.card(208,760,1200,142,'Shared contract');s.text(232,820,'16px body · 14px label · 12px meta · tabular mono cho clock / notation',14,weight=600);s.text(232,853,'Tokens: canonical scheme 4 theme · spacing 4/8/12/16/24 · shape 8/12 · no hardcoded Light consumer',13,P['muted']);s.text(232,880,'DOM: Leptos shell / canvas: Macroquad RenderList · không port SVG mock thành gameplay DOM',12,P['muted'])

def auth(s,register=False):
 shell(s,'Tạo tài khoản' if register else 'Đăng nhập',future=True);s.rect(208,180,444,704,P['soft'],r=12);s.text(236,247,'Một tài khoản,',27,P['primary'],700);s.text(236,287,'nhiều ván cùng nhau.',27,P['primary'],700);s.lines(236,350,['Chơi local không cần tài khoản.','Đăng nhập chỉ cần khi service online mở.','Không thu mật khẩu thật trong prototype.'],14,P['primary'],step=28);s.circle(430,549,88,'#FFF6E5','#C7AE83',2);s.text(430,591,'帥',106,P['danger'],600,'middle');s.text(236,806,'PHASE 4 / AUTH GATE',12,P['primary'],600);s.card(676,180,732,704,'Tạo tài khoản' if register else 'Chào mừng trở lại');s.text(700,248,'Trạng thái mẫu · backend/auth chưa sẵn sàng',13,P['muted']);yy=304
 if register:s.field(700,yy,684,'TÊN HIỂN THỊ','Bạn muốn được gọi là gì?');yy+=100
 s.field(700,yy,684,'EMAIL','ban@example.com');yy+=100;s.field(700,yy,684,'MẬT KHẨU','••••••••••••       Hiện');yy+=100
 if register:s.check(700,yy,'Điều khoản / quyền riêng tư chỉ khi có URL thật',False);yy+=60
 else:s.check(700,yy,'Nhớ thiết bị này',False);s.text(1384,yy+16,'Quên mật khẩu?',13,P['primary'],anchor='end');yy+=60
 s.button(700,yy,684,'Tạo tài khoản · có gate' if register else 'Đăng nhập · có gate','disabled');s.button(700,yy+60,684,'Tiếp tục chơi local','secondary');s.text(700,yy+126,'Đã có tài khoản? Đăng nhập' if register else 'Chưa có tài khoản? Tạo tài khoản',13,P['primary']);s.text(700,853,'Lỗi auth: generic + retry; không tiết lộ tồn tại email.',12,P['muted'])

def profile(s):
 shell(s,'Hồ sơ','Bạn bè',True);s.card(208,180,1200,174);s.circle(264,264,40,P['soft']);s.text(264,274,'B',25,P['primary'],600,'middle');s.text(328,250,'Bạn',26,weight=700);s.text(328,281,'Hồ sơ minh họa · @nguoi-choi',14,P['muted']);s.pill(328,304,'Local / chưa đăng nhập',w=224);s.button(1144,230,240,'Chỉnh sửa hồ sơ','secondary');s.card(208,380,788,454,'Thông tin & quyền hiển thị');s.field(232,449,740,'TÊN HIỂN THỊ','Bạn','Cập nhật local nếu chưa có account adapter');s.field(232,558,740,'GIỚI THIỆU','Thích những ván cờ cùng bạn bè');s.label(232,665,'TRẠNG THÁI ONLINE');s.button(232,682,228,'Chỉ bạn bè','soft');s.button(476,682,228,'Ẩn','secondary');s.button(720,682,252,'Công khai','secondary');s.button(232,770,740,'Lưu · account adapter có gate','disabled');s.card(1020,380,388,454,'Dữ liệu phải có nguồn');s.lines(1040,443,['Không bịa rating, streak hay tỷ lệ thắng.','Nếu adapter chưa có: hiện placeholder','có lý do thay vì số liệu demo thật.'],13,P['muted'],step=28);s.row(1040,549,348,'Ván đã chơi','Theo nguồn replay','—');s.row(1040,623,348,'Xếp hạng','Chỉ theo rating service','—');s.row(1040,697,348,'Bạn bè','Chỉ sau account/friends','—')

def friends(s):
 shell(s,'Bạn bè','Bạn bè',True);s.text(208,140,'Danh sách mẫu. Không gửi lời mời, không có presence realtime.',13,P['muted']);s.button(1220,126,188,'Thêm bạn','secondary');s.card(208,180,788,664,'Bạn bè');s.field(232,243,740,'TÌM NGƯỜI BẠN','⌕  Tìm theo tên hiển thị hoặc ID');
 for i,(name,meta,status) in enumerate([('Minh Anh','@minhanh · dữ liệu mẫu','Online mẫu'),('Ngọc Mai','@ngocmai · dữ liệu mẫu','Offline mẫu'),('Hoàng','@hoang · dữ liệu mẫu','Trong ván mẫu')]):
  y=353+i*132;s.circle(258,y+25,24,P['soft']);s.text(258,y+32,''.join(x[0] for x in name.split()),13,P['primary'],600,'middle');s.text(302,y+17,name,17,weight=600);s.text(302,y+44,meta,12,P['muted']);s.pill(302,y+59,status,'neutral',w=142);s.button(804,y+8,168,'Mời chơi','disabled');s.line(232,y+103,972,y+103)
 s.card(1020,180,388,324,'Lời mời · dữ liệu mẫu');s.text(1040,253,'Từ Lê An · @lean',16,weight=600);s.text(1040,285,'Chờ phản hồi · mẫu',13,P['muted']);s.button(1040,330,164,'Chấp nhận','disabled');s.button(1218,330,166,'Từ chối','disabled');s.lines(1040,419,['Pending / accepted / blocked phải','dựa trên adapter, không đổi giả.'],13,P['muted']);s.card(1020,528,388,316,'Trạng thái rỗng / quyền riêng tư');s.lines(1040,591,['Chưa có bạn bè: hướng dẫn rõ CTA.','Không tìm thấy: không dò email cá nhân.','Invite lỗi: giữ input và retry an toàn.','Presence cũ: timestamp hoặc offline.'],13,P['muted'],step=29);s.button(1040,772,348,'Tìm bạn · có gate','disabled')

def rooms(s):
 shell(s,'Phòng chơi','Phòng chơi',True);s.text(208,140,'Lobby service skeleton · phòng, presence, capacity minh họa',13,P['muted']);s.button(1068,126,164,'Nhập mã','secondary');s.button(1244,126,164,'Tạo phòng','disabled');s.card(208,180,1200,704);s.field(232,241,740,'TÌM PHÒNG','⌕  Tên phòng hoặc mã mời');s.button(992,251,184,'Cờ vua','soft');s.button(1190,251,194,'Mọi game','secondary');s.label(232,345,'PHÒNG / GAME');s.label(842,345,'THỜI GIAN');s.label(1036,345,'CHỖ TRỐNG');s.line(232,362,1384,362)
 for i,(t,c,g,cap) in enumerate([('Một ván cùng bạn','TB-2086','Cờ vua','1 / 2'),('Đường đi cuối tuần','TB-3188','Miền đất nhỏ','2 / 4'),('Cờ vua thư thả','TB-4421','Cờ vua','2 / 2')]):
  y=397+i*118;s.text(232,y,t,17,weight=600);s.text(232,y+29,c+'  ·  '+g+'  ·  dữ liệu mẫu',12,P['muted'],mono=True);s.text(842,y+15,'10 + 5 · mẫu',14,mono=True);s.pill(1036,y-3,cap,'neutral',w=108);s.button(1212,y-8,172,'Đã đầy' if i==2 else 'Vào · có gate','disabled');s.line(232,y+72,1384,y+72)
 s.card(232,782,1152,74);s.text(252,813,'Offline / service chưa mở → banner có lý do + chơi local',14,weight=600);s.text(252,838,'Không hiển thị danh sách mẫu như phòng đang tồn tại thật.',12,P['muted'])

def room(s):
 shell(s,'Phòng chờ · TB-2086','Phòng chơi',True);s.text(208,140,'Cờ vua / Một ván cùng bạn · capacity và quyền từ server, đây là dữ liệu mẫu',13,P['muted']);s.card(208,180,788,300,'Người chơi');s.pill(812,197,'1 / 2 · mẫu',w=162);s.circle(258,277,25,P['soft']);s.text(258,284,'B',14,P['primary'],600,'middle');s.text(302,271,'Bạn · host',18,weight=600);s.text(302,298,'Slot Trắng · trạng thái mẫu',13,P['muted']);s.pill(814,262,'Chưa ready',w=156);s.line(232,331,972,331);s.circle(258,392,25,'#F4EDF7');s.text(258,401,'+',24,P['muted'],anchor='middle');s.text(302,389,'Chỗ trống',18,weight=600);s.text(302,415,'Slot Đen · chờ người chơi thật',13,P['muted']);s.card(1020,180,388,300,'Cấu hình phòng');s.row(1040,237,348,'Game / version','Chess · lấy từ server');s.row(1040,311,348,'Thời gian','10 + 5 · mẫu');s.button(1040,416,348,'Đổi cấu hình · host gate','disabled');s.card(208,504,788,380,'Tin nhắn phòng · chưa nối service');s.rect(232,566,740,198,P['bg'],r=8);s.text(252,604,'Hệ thống · 03:12 mẫu',12,P['muted']);s.text(252,634,'Chỉ gửi chat sau khi auth, quyền và room session hợp lệ.',13,P['muted']);s.field(232,797,550,'TIN NHẮN','Nhập tin nhắn…');s.button(796,807,176,'Gửi · có gate','disabled');s.card(1020,504,388,380,'Sẵn sàng & bắt đầu');s.lines(1040,568,['Host không tự start khi thiếu người.','Ready state đồng bộ theo revision.','Config đổi → ready invalidation rõ.','Start chỉ sau server ack + session.'],13,P['muted'],step=29);s.button(1040,711,348,'Sẵn sàng · có gate','disabled');s.button(1040,767,348,'Bắt đầu · chờ đủ người','disabled');s.button(1040,823,348,'Rời phòng','secondary')

def queue(s):
 shell(s,'Ghép trận','Phòng chơi',True);s.card(208,180,788,704,'Đang tìm đối thủ · trạng thái minh họa');s.circle(602,352,62,P['soft']);s.text(602,373,'↻',68,P['primary'],600,'middle');s.text(602,483,'00:42',40,weight=600,anchor='middle',mono=True);s.text(602,524,'Thời gian đã chờ · mẫu',14,P['muted'],anchor='middle');s.pill(456,558,'Cờ vua · 10 + 5 · mẫu',w=294);s.lines(602,642,['Chỉ tạo queue ticket sau request thật.','Không dựng người chơi/rating hoặc ETA để trấn an.'],14,P['muted'],step=28)
 # center the two copy lines explicitly
 s.a[-2]=s.a[-2].replace('text-anchor="start"','text-anchor="middle"');s.a[-1]=s.a[-1].replace('text-anchor="start"','text-anchor="middle"')
 s.button(376,766,452,'Hủy tìm trận · có gate','disabled');s.text(602,856,'Hủy đợi server ack; xử lý race khi vừa match.',12,P['muted'],anchor='middle');s.card(1020,180,388,214,'Đã tìm thấy · mẫu');s.lines(1040,240,['Hiện opponent/session sau ack.','Cho accept nếu protocol yêu cầu.','Đếm ngược dựa trên deadline server.'],13,P['muted']);s.button(1040,326,348,'Vào phòng · có gate','disabled');s.card(1020,416,388,214,'Timeout / hết hiệu lực');s.lines(1040,476,['Nêu lý do ticket kết thúc.','Không tự đăng ký lại vô hạn.','Giữ mode/time để thử lại.'],13,P['muted']);s.button(1040,562,348,'Thử lại','secondary');s.card(1020,652,388,232,'Mất kết nối');s.lines(1040,712,['Reconcile ticket trước retry.','Không tạo hai ticket song song.','Trở về lobby giữ state cần thiết.'],13,P['muted']);s.button(1040,816,348,'Về phòng chơi','secondary')

SCREENS=[('01-library','02-discovery','Thư viện game',library),('02-game-detail','02-discovery','Chi tiết Cờ vua',detail),('03-new-match','02-discovery','Thiết lập local',setup),('04-chess','03-gameplay','Chess HUD',lambda s:gameplay(s,'chess')),('05-caro','03-gameplay','Caro / Werewolf / Recovery',companion),('06-tiles','03-gameplay','Tiles HUD',lambda s:gameplay(s,'tiles')),('07-werewolf','03-gameplay','Ma sói adapter',companion),('08-xiangqi','05-xiangqi','Xiangqi Chơi',lambda s:xiangqi(s,'play')),('09-result','04-replay','Kết quả ván',result),('10-history','04-replay','Ván của tôi',history),('11-analysis','04-replay','Replay core',replay),('12-learn','05-xiangqi','Xiangqi Gia sư',lambda s:xiangqi(s,'learn')),('13-settings','01-foundation','Cài đặt',settings),('14-resources','05-xiangqi','Tài nguyên',resources),('15-login','06-accounts','Đăng nhập',lambda s:auth(s,False)),('16-register','06-accounts','Tạo tài khoản',lambda s:auth(s,True)),('17-rooms','07-lobby','Phòng chơi',rooms),('18-room','07-lobby','Phòng chờ',room),('19-queue','07-lobby','Ghép trận',queue),('20-profile','06-accounts','Hồ sơ',profile),('21-friends','06-accounts','Bạn bè',friends),('22-design-system','01-foundation','Bộ thành phần',showcase),('11-xiangqi-analysis','05-xiangqi','Xiangqi Phân tích',lambda s:xiangqi(s,'analysis'))]
PACKS={
 '01-foundation':('Foundation / Theme / Settings',['13-settings','22-design-system']),
 '02-discovery':('Library / Game detail / Local setup',['01-library','02-game-detail','03-new-match']),
 '03-gameplay':('Board-first Chess / Tiles / Adapter states',['04-chess','06-tiles','05-caro']),
 '04-replay':('Result / History / Replay core',['09-result','10-history','11-analysis']),
 '05-xiangqi':('Xiangqi Play / Analysis / Learn / Resources',['08-xiangqi','11-xiangqi-analysis','12-learn']),
 '06-accounts':('Auth / Profile / Friends',['15-login','20-profile','21-friends']),
 '07-lobby':('Rooms / Ready room / Queue',['17-rooms','18-room','19-queue'])}
if __name__=='__main__':
 OUT.mkdir(parents=True,exist_ok=True)
 bypack={k:[] for k in PACKS}
 for key,pack,title,fn in SCREENS:
  d=OUT/pack/'screens';d.mkdir(parents=True,exist_ok=True);s=Scene();fn(s);(d/(key+'.svg')).write_text(s.svg(title));bypack[pack].append({'id':key,'title':title,'file':'screens/'+key+'.svg'})
 for pack,(title,previews) in PACKS.items():
  d=OUT/pack;(d/'screen-index.json').write_text(json.dumps(bypack[pack],ensure_ascii=False,indent=2));(d/'previews').mkdir(exist_ok=True)
  viewer='''const screens=JSON.parse(document.getElementById('screen-data').textContent);const select=document.getElementById('screens'),figure=document.getElementById('design');for(const s of screens){const option=document.createElement('option');option.value=s.file;option.textContent=s.id+' · '+s.title;select.append(option)}function show(){figure.src=select.value;document.querySelector('a#source').href=select.value;document.querySelector('#label').textContent=screens.find(s=>s.file===select.value).title}select.addEventListener('change',show);document.querySelector('#zoom').addEventListener('change',e=>{figure.style.width=e.target.value==='fit'?'100%':'1440px'});show();'''
  (d/'viewer.mjs').write_text(viewer)
  (d/'prototype.css').write_text('''*{box-sizing:border-box}body{margin:0;background:#F9F7F4;color:#1E1C21;font:16px system-ui,sans-serif}header{display:flex;gap:16px;align-items:center;flex-wrap:wrap;padding:12px 20px;background:#fff;border-bottom:1px solid #E7E2E9}h1{font-size:18px;margin:0}select,a{min-height:44px;padding:10px 12px;border:1px solid #79747E;border-radius:8px;color:#5634BE;background:white}a:focus-visible,select:focus-visible{outline:3px solid #5634BE;outline-offset:2px}main{overflow:auto;padding:16px}img{display:block;max-width:none}p{margin:0;font-size:13px;color:#4B4650}small{font-size:12px}@media(max-width:700px){header{align-items:stretch}select{max-width:100%}main{padding:0}}''')
  (d/'index.html').write_text('<!doctype html><html lang="vi"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Tabula '+title+'</title><link rel="stylesheet" href="prototype.css"><header><h1>Tabula · '+title+'</h1><label>Màn hình <select id="screens"></select></label><label>Xem <select id="zoom"><option value="fit">Vừa chiều rộng</option><option value="full">Kích thước 1440px</option></select></label><a id="source">SVG chỉnh sửa được</a><p>Bản thiết kế tĩnh · dữ liệu mẫu · không chạy luật, backend hay AI</p></header><main><p id="label"></p><img id="design" alt="Thiết kế Tabula; mô tả và state contract nằm trong implementation-notes.md"></main><script id="screen-data" type="application/json">'+json.dumps(bypack[pack],ensure_ascii=False).replace('</','<\\/')+'</script><script type="module" src="viewer.mjs"></script></html>')
  (d/'render.mjs').write_text('''// Design-only SVG rasterizer. Install sharp from the official npm registry, then run node render.mjs.\nimport fs from 'node:fs/promises';import path from 'node:path';import { createRequire } from 'node:module';const require=createRequire(import.meta.url);const sharp=require('sharp');const here=path.dirname(new URL(import.meta.url).pathname);const ids='''+json.dumps(previews[:2]+['mobile-states'])+''';await fs.mkdir(path.join(here,'previews'),{recursive:true});for(const id of ids){await sharp(path.join(here,'screens',id+'.svg')).png({compressionLevel:9}).toFile(path.join(here,'previews',id+'.png'));console.log(id)}''')
 (OUT/'traceability.json').write_text(json.dumps([{'input_screen':id,'pack':pack,'design_source':pack+'/screens/'+id+'.svg','title':title} for id,pack,title,fn in SCREENS],ensure_ascii=False,indent=2))
 print('Generated',len(SCREENS),'SVG screen designs in',len(PACKS),'packs')
