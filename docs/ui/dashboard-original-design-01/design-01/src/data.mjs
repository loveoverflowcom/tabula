/** Demo content only. No accounts, network services, or real AI are connected. */
export const GAMES = [
 {id:'xiangqi',name:'Cờ tướng',label:'Xiangqi Tutor',tag:'Có gia sư AI',category:'strategy',tone:'lavender',players:'1–2',time:'15–30 phút',level:'Chiến thuật',icon:'spark',desc:'Một bàn cờ. Một người thầy. Khám phá ý nghĩa phía sau mỗi nước đi cùng huấn luyện viên AI.',modes:['local','bot','online'],available:true},
 {id:'chess',name:'Cờ vua',label:'Chess',tag:'Kinh điển',category:'strategy',tone:'sage',players:'2',time:'5–30 phút',level:'Chiến thuật',icon:'trophy',desc:'64 ô cờ, vô vàn khả năng. Rủ một người bạn và dành một khoảng thời gian cho những nước đi hay.',modes:['local','online'],available:true},
 {id:'caro',name:'Caro',label:'Five in a row',tag:'Dễ bắt đầu',category:'quick',tone:'peach',players:'2',time:'5–10 phút',level:'Nhẹ nhàng',icon:'bolt',desc:'Năm quân liên tiếp, một chiến thắng ngọt ngào. Đơn giản để bắt đầu, thú vị để chơi thêm một ván.',modes:['local','online'],available:true},
 {id:'tiles',name:'Miền đất nhỏ',label:'Tiles',tag:'Chơi cùng bot',category:'strategy',tone:'mint',players:'2–5',time:'20–40 phút',level:'Khám phá',icon:'leaf',desc:'Từng mảnh ghép làm nên một thế giới. Kết nối những con đường, dựng thành phố và tìm cơ hội ghi điểm.',modes:['local','bot','online'],available:true},
 {id:'werewolf',name:'Ma sói',label:'Werewolf',tag:'Bản thiết kế',category:'party',tone:'night',players:'6–12',time:'20–45 phút',level:'Hội nhóm',icon:'moon',desc:'Khi ngôi làng chìm vào giấc ngủ, những bí mật thức giấc. Lắng nghe, suy luận và tìm ra ai đang nói dối.',modes:['online'],available:false}
];
export const GAME_BY_ID = Object.fromEntries(GAMES.map(g=>[g.id,g]));
export const SCREEN_INDEX = [
 ['01-library','library','Thư viện game','Khám phá · tìm kiếm · tiếp tục ván'],
 ['02-game-detail','game/xiangqi','Chi tiết game','Chế độ · luật chơi · tài nguyên'],
 ['03-new-match','setup/xiangqi','Thiết lập ván','Đối thủ · thời gian · bên chơi'],
 ['04-chess','play/chess','Bàn chơi · Cờ vua','Bàn cờ · đồng hồ · nước đi'],
 ['05-caro','play/caro','Bàn chơi · Caro','Bàn ô · lượt đánh · kết quả'],
 ['06-tiles','play/tiles','Bàn chơi · Tiles','Bản đồ · mảnh ghép · điểm'],
 ['07-werewolf','play/werewolf','Bàn chơi · Ma sói','Ngày/đêm · vai trò · biểu quyết'],
 ['08-xiangqi','play/xiangqi','Bàn chơi · Cờ tướng','Bàn cờ · đối thủ · điều khiển'],
 ['09-result','result/xiangqi','Kết quả ván','Tổng kết · chơi lại · xem lại'],
 ['10-history','history','Ván của tôi','Đang chơi · đã xong · đã lưu'],
 ['11-analysis','analysis/xiangqi','Phân tích','Replay · nhánh thử · phương án'],
 ['12-learn','learn/xiangqi','Gia sư cờ tướng','Hội thoại · gợi ý · bàn phân tích'],
 ['13-settings','settings','Cài đặt','Giao diện · âm thanh · dữ liệu'],
 ['14-resources','resources/xiangqi','Tài nguyên game','Engine · giải thích · offline'],
 ['15-login','login','Đăng nhập','Luồng tài khoản minh họa'],
 ['16-register','register','Tạo tài khoản','Đăng ký · tiếp tục chơi local'],
 ['17-rooms','rooms','Phòng chơi','Tìm kiếm · lọc · tạo phòng'],
 ['18-room','room','Phòng chờ','Chỗ ngồi · sẵn sàng · mời bạn'],
 ['19-queue','queue','Ghép trận','Đang tìm · hủy · kết quả mẫu'],
 ['20-profile','profile','Hồ sơ','Thông tin · thống kê · lịch sử'],
 ['21-friends','friends','Bạn bè','Trạng thái · lời mời · kết nối'],
 ['22-design-system','design-system','Bộ thành phần','Token · màu · kiểu chữ · trạng thái']
];
export const MATCHES = [
 {game:'xiangqi',title:'Một ván cùng huấn luyện viên',opponent:'Huấn luyện viên',date:'Hôm nay, 09:42',status:'playing',result:'Đến lượt bạn',moves:'12 nước',time:'08:24',id:'local-01'},
 {game:'chess',title:'Ván cờ sáng thứ Năm',opponent:'Minh Anh',date:'Hôm nay, 08:30',status:'done',result:'Bạn thắng',moves:'32 nước',time:'14:08',id:'local-02'},
 {game:'tiles',title:'Miền đất của chúng mình',opponent:'2 bot',date:'Hôm qua, 20:15',status:'done',result:'Hạng 1 · 68 điểm',moves:'24 lượt',time:'28:35',id:'local-03'},
 {game:'xiangqi',title:'Luyện tập khai cuộc',opponent:'Huấn luyện viên',date:'Hôm qua, 19:00',status:'saved',result:'Đã lưu để học',moves:'18 nước',time:'12:06',id:'local-04'},
 {game:'caro',title:'Thêm một ván nữa',opponent:'Hoàng Nam',date:'28/09, 21:06',status:'done',result:'Bạn thắng',moves:'27 lượt',time:'06:24',id:'local-05'}
];
export const FRIENDS = [
 {name:'Minh Anh',handle:'minhanh',initials:'MA',tone:'peach',status:'Đang ở phòng chờ',game:'Cờ vua',online:true},
 {name:'Hoàng Nam',handle:'hoangnam',initials:'HN',tone:'sage',status:'Đang chơi',game:'Caro',online:true},
 {name:'Linh Chi',handle:'linhchi',initials:'LC',tone:'lavender',status:'Sẵn sàng chơi',game:'',online:true},
 {name:'Tuấn Anh',handle:'tuananh',initials:'TA',tone:'mint',status:'Hoạt động 2 giờ trước',game:'',online:false},
 {name:'Phương Thảo',handle:'phuongthao',initials:'PT',tone:'sand',status:'Hoạt động hôm qua',game:'',online:false}
];
export const CHESS_INITIAL = [
 ['r','n','b','q','k','b','n','r'],['p','p','p','p','p','p','p','p'],['','','','','','','',''],['','','','','','','',''],['','','','','P','','',''],['','','','','','','',''],['P','P','P','P','','P','P','P'],['R','N','B','Q','K','B','N','R']
];
export const XIANGQI_INITIAL = [
 [0,0,'車','black'],[1,0,'馬','black'],[2,0,'象','black'],[3,0,'士','black'],[4,0,'將','black'],[5,0,'士','black'],[6,0,'象','black'],[7,0,'馬','black'],[8,0,'車','black'],
 [1,2,'砲','black'],[7,2,'砲','black'],[0,3,'卒','black'],[2,3,'卒','black'],[4,3,'卒','black'],[6,3,'卒','black'],[8,3,'卒','black'],
 [0,6,'兵','red'],[2,6,'兵','red'],[4,5,'兵','red'],[6,6,'兵','red'],[8,6,'兵','red'],[1,7,'炮','red'],[7,7,'炮','red'],
 [0,9,'俥','red'],[1,9,'傌','red'],[2,9,'相','red'],[3,9,'仕','red'],[4,9,'帥','red'],[5,9,'仕','red'],[6,9,'相','red'],[7,9,'傌','red'],[8,9,'俥','red']
];
export const TOKENS_META = {source:'tokens.toml',ref:'44f6b74e07648abc7191363d7582efc1fceab262',identity:'M3 Expressive-inspired',date:'2026-10-01',note:'Các semantic token gốc được giữ; thêm surface, tint và layout token dành cho prototype. Không tự sửa tokens.toml của repository.'};
