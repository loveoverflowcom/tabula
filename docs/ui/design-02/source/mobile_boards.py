# Reusable design-only mobile composition. Not production UI code.
from build_designs import Scene,P,chess,xq,tiles
from pathlib import Path
import json
ROOT=Path(__file__).resolve().parents[1]/'dist'

def phone(s,x,title,future=False,tab='Thư viện'):
 y=150;w=390;s.rect(x-8,y-8,w+16,770,'#1E1C21',r=22);s.rect(x,y,w,754,P['bg'],r=16);s.rect(x,y,w,58,P['paper'],r=16);s.rect(x,y+42,w,16,P['paper'],r=0);s.text(x+16,y+36,'T.  tabula',21,P['primary'],700);s.text(x+w-16,y+36,'≡',25,P['muted'],anchor='end');s.text(x+16,y+94,title,21,weight=700);s.pill(x+16,y+112,'Dự kiến / có gate' if future else 'Local / dữ liệu mẫu','planned' if future else 'current',w=196)
 s.rect(x,y+698,w,56,P['paper'],r=0);s.line(x,y+698,x+w,y+698)
 for i,t in enumerate(['Thư viện','Ván của tôi','Phòng','Cài đặt']):s.text(x+49+i*97,y+732,t,10,P['primary'] if t==tab else P['muted'],600,'middle')
 return x+16,y+162,w-32

def mobile(pack):
 s=Scene();s.rect(0,0,1440,1000,P['bg'],r=0);s.text(56,53,'Tabula · '+pack+' · Mobile & trạng thái',27,weight=700);s.text(56,87,'Bố cục 390px được thiết kế riêng · 44dp controls · dữ liệu mẫu · không phải runtime screenshot',14,P['muted']);xs=[64,525,986]
 if pack=='01-foundation':
  x,y,w=phone(s,xs[0],'Cài đặt',tab='Cài đặt');s.card(x,y,w,404,'Giao diện');
  s.button(x+16,y+54,w-32,'Theme: Theo hệ thống ▾','secondary')
  for i,t in enumerate(['Tương phản cao','Giảm chuyển động','Âm thanh','Mật độ gọn']):s.text(x+16,y+146+i*61,t,13,weight=600);s.toggle(x+w-60,y+126+i*61,i in [1,3]);s.line(x+16,y+166+i*61,x+w-16,y+166+i*61)
  s.button(x,y+426,w,'Cài đặt trong ván','secondary')
  x,y,w=phone(s,xs[1],'Overlay trong ván',tab='Cài đặt');s.rect(x,y,w,190,P['sage'],r=8);s.text(x+20,y+48,'Bàn giữ nguyên snapshot',14,P['green'],600);s.card(x,y+210,w,288,'Bàn chơi');s.row(x+16,y+251,w-32,'Ô hợp lệ','Dấu hiệu + chữ');s.row(x+16,y+326,w-32,'Âm thanh','Preference chung');s.button(x+16,y+430,w-32,'Đóng & trả focus','secondary')
  x,y,w=phone(s,xs[2],'Component / focus');s.button(x+8,y+26,w-16,'Primary · target 44dp',focus=True);s.button(x,y+98,w,'Disabled · lý do có gate','disabled');s.field(x,y+185,w,'EMAIL','ban@example.com','Thông báo lỗi inline',True);s.card(x,y+299,w,193,'Thử lại');s.lines(x+16,y+355,['Mất kết nối. Giữ dữ liệu nhập.','Focus vào lỗi và hành động rõ.'],12,P['muted']);s.button(x+16,y+423,w-32,'Thử lại','secondary')
 elif pack=='02-discovery':
  x,y,w=phone(s,xs[0],'Thư viện game');s.field(x,y,w,'TÌM GAME','⌕  Tìm theo tên…');s.button(x,y+80,172,'Chơi local','soft');s.button(x+186,y+80,172,'Yêu thích','secondary');
  for i,(a,b,c) in enumerate([('♞  Cờ vua','2 người · cùng máy','Chơi local'),('▧  Miền đất nhỏ','2–5 người · đặt mảnh','Chơi local'),('帥  Cờ tướng','#48 · chưa có module','Định hướng')]):s.card(x,y+146+i*104,w,90,a);s.text(x+16,y+206+i*104,b,11,P['muted']);s.pill(x+w-142,y+162+i*104,c,'current' if i<2 else 'planned',w=126)
  x,y,w=phone(s,xs[1],'Cờ vua / Chi tiết');s.rect(x,y,w,118,P['sage'],r=8);s.text(x+w/2,y+89,'♞  ♔',64,'#4D6249',600,'middle');s.text(x,y+159,'Cùng một máy, không cần account',13,weight=600);s.lines(x,y+189,['Capability do module cung cấp.','Online và bot có gate riêng.'],12,P['muted']);s.button(x,y+251,w,'Thiết lập ván local →');s.button(x,y+313,w,'Online · chưa mở','disabled');s.card(x,y+383,w,117,'Luật & phím tắt');s.text(x+16,y+449,'Nguồn versioned theo game module',12,P['muted'])
  x,y,w=phone(s,xs[2],'Thiết lập local');s.field(x,y,w,'TRẮNG','Bạn');s.field(x,y+90,w,'ĐEN','Người chơi 2');s.label(x,y+190,'THỜI GIAN · MẪU');s.button(x,y+210,172,'10 + 5','soft');s.button(x+186,y+210,172,'15 + 10','secondary');s.check(x,y+289,'Ô hợp lệ và nước vừa đi');s.card(x,y+350,w,88,'Tóm tắt · local');s.text(x+16,y+409,'Timer chỉ bật nếu module hỗ trợ.',11,P['muted']);s.button(x,y+461,w,'Bắt đầu ván local →')
 elif pack=='03-gameplay':
  x,y,w=phone(s,xs[0],'Cờ vua');s.text(x,y+20,'Đen',13,weight=600);s.text(x+w,y+20,'08:16',21,anchor='end',mono=True,weight=600);chess(s,x+8,y+54,342);s.text(x,y+437,'Bạn · đến lượt',13,P['green'],600);s.text(x+w,y+437,'09:42',21,anchor='end',mono=True,weight=600);s.button(x,y+464,172,'Nước đi','soft');s.button(x+186,y+464,172,'Bàn đọc','secondary')
  x,y,w=phone(s,xs[1],'Miền đất nhỏ');s.rect(x,y,w,298,'#E8EEE5',r=8)
  for c,r in [(0,0),(1,0),(2,0),(0,1),(1,1),(2,1),(1,2)]:
   xx=x+60+c*72;yy=y+35+r*72;s.rect(xx,yy,70,70,'#C9D5B8','#A5B69C',r=4);s.line(xx+35,yy,xx+35,yy+70,'#F8ECD1',9)
  s.text(x+16,y+279,'Pan / zoom không che action',11,P['muted']);s.pill(x,y+315,'Bạn đến lượt · điểm 18 mẫu','current',w=296);s.button(x,y+367,172,'↶  Xoay','secondary');s.button(x+186,y+367,172,'Đặt mảnh');s.button(x,y+429,w,'Người chơi / điểm','soft');s.text(x,y+495,'Target 44dp, không thu nhỏ thành34dp',11,P['muted'])
  x,y,w=phone(s,xs[2],'Mất kết nối',True);s.card(x,y,w,302,'Ván đang tạm khóa');s.lines(x+16,y+76,['Giữ snapshot cuối.','Không chuyển online thành local.','Khóa input đến khi server resync.','Lỗi rõ + retry / rời an toàn.'],13,P['muted'],step=30);s.button(x+16,y+231,w-32,'Thử lại','secondary');s.card(x,y+324,w,174,'Adapter chưa đầy đủ');s.lines(x+16,y+383,['Caro: cần rules / presenter.','Ma sói: role view riêng tư.','Không tiết lộ vai trò người khác.'],12,P['muted'])
 elif pack=='04-replay':
  x,y,w=phone(s,xs[0],'Ván của tôi',True,'Ván của tôi');s.button(x,y,172,'Tất cả game','soft');s.button(x+186,y,172,'Nhập replay','secondary')
  for i,(a,b) in enumerate([('Cờ vua · Trắng thắng','42 lượt · Local · mẫu'),('Miền đất nhỏ · 28 điểm','36 lượt · Local · mẫu'),('Cờ vua · Hòa','58 lượt · Local · mẫu')]):s.card(x,y+70+i*126,w,112,a);s.text(x+16,y+133+i*126,b,11,P['muted']);s.text(x+w-16,y+163+i*126,'Xem lại →',12,P['primary'],600,'end')
  x,y,w=phone(s,xs[1],'Kết quả ván',True,'Ván của tôi');s.card(x,y,w,228);s.pill(x+16,y+20,'Cờ vua · Local · mẫu',w=216);s.text(x+16,y+94,'Trắng thắng',27,weight=700);s.text(x+16,y+125,'Chiếu hết · 42 lượt · dữ liệu mẫu',12,P['muted']);s.button(x+16,y+164,w-32,'Xem replay →');s.card(x,y+252,w,162,'Nguồn kết quả');s.lines(x+16,y+310,['GameResult từ module.','Không có accuracy/Elo giả.','Lỗi lưu vẫn giữ kết quả trên màn.'],12,P['muted']);s.button(x,y+446,w,'Chơi ván mới','secondary')
  x,y,w=phone(s,xs[2],'Replay · ply12',True,'Ván của tôi');chess(s,x+10,y+13,338);s.pill(x,y+387,'Bản gốc chỉ đọc','selected',w=184);s.button(x,y+430,78,'|‹','secondary');s.button(x+92,y+430,78,'‹','secondary');s.button(x+184,y+430,78,'›','secondary');s.button(x+276,y+430,82,'›|','secondary');s.text(x,y+502,'Nhánh thử riêng; ←/→ bước lượt',11,P['muted'])
 elif pack=='05-xiangqi':
  x,y,w=phone(s,xs[0],'Xiangqi / Chơi',True);xq(s,x+18,y+2,322,380);s.button(x,y+403,172,'Chơi','soft');s.button(x+186,y+403,172,'Phân tích','secondary');s.button(x,y+465,w,'Bot · chưa có provider','disabled')
  x,y,w=phone(s,xs[1],'Gia sư / Evidence',True);s.card(x,y,w,162,'Ply 1 · lời giải mẫu');s.lines(x+16,y+63,['Không có AI/engine đang chạy.','Lời giải thật phải gắn vị trí + PV.','Thiếu dữ kiện → nói chưa đủ cơ sở.'],12,P['muted']);s.card(x,y+186,w,202,'Bằng chứng');s.lines(x+16,y+248,['Position hash / ply1 / branch','PV / depth / nodes / elapsed','Provider + version + nguồn','Position đổi → đánh dấu stale.'],12,P['muted'],step=27);s.field(x,y+425,w,'HỎI VỀ VỊ TRÍ NÀY','Một câu hỏi…');s.button(x,y+485,w,'Gửi · chưa có LLM','disabled')
  x,y,w=phone(s,xs[2],'Tài nguyên',True);s.card(x,y,w,196,'Chưa cài engine');s.lines(x+16,y+62,['Provider chưa được chọn.','License / checksum / compatibility.','Không tải binary/model trong mock.'],12,P['muted']);s.button(x+16,y+133,w-32,'Cài · có gate','disabled');s.card(x,y+220,w,276,'State tài nguyên')
  for i,t in enumerate(['Chưa cài','Đang tải / có hủy','Xác minh checksum','Sẵn sàng sau probe','Lỗi / thử lại']):s.pill(x+16,y+277+i*40,t,'current' if i==3 else 'neutral',w=w-32)
 elif pack=='06-accounts':
  x,y,w=phone(s,xs[0],'Đăng nhập',True);s.text(x,y+20,'Chơi local không cần account',13,weight=600);s.field(x,y+74,w,'EMAIL','ban@example.com');s.field(x,y+165,w,'MẬT KHẨU','••••••••••');s.check(x,y+255,'Nhớ thiết bị này',False);s.button(x,y+311,w,'Đăng nhập · có gate','disabled');s.button(x,y+373,w,'Tiếp tục chơi local','secondary');s.text(x,y+464,'Lỗi generic, không dò tồn tại email.',11,P['muted'])
  x,y,w=phone(s,xs[1],'Tạo tài khoản',True);s.field(x,y,w,'TÊN HIỂN THỊ','Bạn');s.field(x,y+92,w,'EMAIL','ban@example.com');s.field(x,y+184,w,'MẬT KHẨU','••••••••••');s.check(x,y+286,'Điều khoản khi có URL thật',False);s.button(x,y+341,w,'Tạo account · có gate','disabled');s.button(x,y+403,w,'Tiếp tục chơi local','secondary');s.text(x,y+483,'Không tạo session giả từ form mock.',11,P['muted'])
  x,y,w=phone(s,xs[2],'Bạn bè',True,'Thư viện');s.field(x,y,w,'TÌM THEO ID','⌕  Tên hoặc ID…');s.card(x,y+96,w,192,'Lời mời · dữ liệu mẫu');s.text(x+16,y+154,'Người chơi 3 · @player3',13);s.button(x+16,y+216,154,'Chấp nhận','disabled');s.button(x+184,y+216,158,'Từ chối','disabled');s.card(x,y+316,w,184,'Presence chưa nối');s.lines(x+16,y+376,['Không dùng mẫu làm online thật.','Mời chơi chỉ sau adapter.','Quyền riêng tư và block có scope.'],12,P['muted'])
 else:
  x,y,w=phone(s,xs[0],'Phòng chơi',True,'Phòng');s.field(x,y,w,'TÌM PHÒNG','⌕  Tên hoặc mã…');s.button(x,y+81,172,'Nhập mã','secondary');s.button(x+186,y+81,172,'Tạo phòng','disabled')
  for i,t in enumerate(['Một ván cùng bạn','Đường đi cuối tuần','Cờ vua thư thả']):s.card(x,y+151+i*117,w,103,t);s.text(x+16,y+209+i*117,'Game / capacity từ server · mẫu',11,P['muted']);s.pill(x+16,y+224+i*117,'Chưa mở / có gate','planned',w=216)
  x,y,w=phone(s,xs[1],'Phòng TB-2086',True,'Phòng');s.card(x,y,w,182,'Người chơi');s.row(x+16,y+43,w-32,'Bạn / host','Chưa ready · mẫu');s.row(x+16,y+112,w-32,'Chỗ trống','Slot Đen · chờ player');s.card(x,y+206,w,127,'Cấu hình phòng');s.text(x+16,y+266,'Chess · 10 + 5 · revision mẫu',12,P['muted']);s.text(x+16,y+298,'Config đổi → ready invalidation',11,P['muted']);s.button(x,y+369,w,'Sẵn sàng · có gate','disabled');s.button(x,y+430,w,'Chờ đủ người','disabled');s.text(x,y+506,'Start sau server ack + session thật.',11,P['muted'])
  x,y,w=phone(s,xs[2],'Ghép trận',True,'Phòng');s.circle(x+w/2,y+85,45,P['soft']);s.text(x+w/2,y+108,'↻',58,P['primary'],600,'middle');s.text(x+w/2,y+194,'00:42',33,weight=600,anchor='middle',mono=True);s.text(x+w/2,y+229,'Thời gian mẫu · không có ETA giả',11,P['muted'],anchor='middle');s.card(x,y+267,w,145,'Queue state');s.lines(x+16,y+324,['Ticket thật trước khi báo đang chờ.','Reconcile khi mất kết nối.','Hủy xử lý race với match.'],12,P['muted']);s.button(x,y+452,w,'Hủy · có gate','disabled')
 s.text(64,965,'Responsive contract: drawer + 56px nav · board trước detail sheet · zoom200% không bị khóa · target 44dp',12,P['muted']);return s
if __name__=='__main__':
 for d in sorted(ROOT.glob('[0-9][0-9]-*')):
  p=d.name;s=mobile(p);(d/'screens/mobile-states.svg').write_text(s.svg(p+' mobile states'));idx=json.loads((d/'screen-index.json').read_text());idx.append({'id':'mobile-states','title':'Mobile & trạng thái','file':'screens/mobile-states.svg'});(d/'screen-index.json').write_text(json.dumps(idx,ensure_ascii=False,indent=2));html=(d/'index.html').read_text();a=html.index('<script id="screen-data"');b=html.index('</script>',a);old=html[a:b];start=old.index('>')+1;html=html[:a]+old[:start]+json.dumps(idx,ensure_ascii=False)+html[b:];(d/'index.html').write_text(html)
 print('Generated 7mobile boards')
