# Ma sói — bàn làng có chuyển động

Thiết kế bổ sung cho [#82](https://github.com/loveoverflowcom/tabula/issues/82), thực hiện trước Cờ vua. Source đối chiếu: `develop@e75624ae870a74f62f0f734fbcf2f12043047dd4` ngày 06/10/2026. Đây là design reference với dữ liệu mẫu; không phải ảnh chạy Macroquad hoặc chứng nhận multiplayer.

## Hướng thiết kế

Giữ Rust/Macroquad. Chuyển màn chơi từ lưới nút số sang một cảnh làng 2D: người chơi là các chân dung/ghế quanh lửa trại, chọn bằng cách chạm vào nhân vật. Nền có chiều sâu và ánh sáng nhưng phải dịu sau vùng tên, trạng thái và mục tiêu. Màu điều khiển dùng primary tím hiện có; ánh lửa và trăng là artwork của game, không đổi token danger/selected/focus của nền tảng.

- Desktop: bàn làng chiếm phần lớn khung; lá bài riêng ở vùng nhỏ bên cạnh; header có vòng/pha/thời gian; log công khai gọn. Một CTA theo ngữ cảnh, không hàng dài nút ngang.
- Mobile: lưới avatar tài khoản 4×3; lá bài riêng thu vào drawer. 390×844 dành màn đầu cho 12 ghế và CTA; 320×640/landscape có bố cục gọn riêng và vùng phụ cuộn. Dock/host/footer có slot thật và safe inset.
- Vai của người khác giữ bí mật: không hiện hình/viền/âm thanh theo vai. Artwork riêng chỉ hiện khi projection của đúng viewer cho phép. Avatar công khai không lấy từ role assignment.
- Simulator: chuyển ghế, pha tiếp theo và ván mới ở nhóm công cụ phụ; luôn ghi mô phỏng cục bộ. Trong sản phẩm không biến các nút simulator thành quyền của người chơi.
- Trạng thái mục tiêu: chưa chọn → nhấc nhẹ chân dung → vòng selected + chữ “Đã chọn” → gửi → biên nhận riêng. Không bấm/hover tự gửi; hủy/chuyển mục tiêu vẫn dễ dàng.
- Phù thủy: hai bình minh họa rõ khác nhau, tên và số lượng được phép thấy; chọn bình rồi chọn chân dung. Tiên tri/Bảo vệ/Thợ săn dùng cùng bàn nhưng hướng dẫn riêng. Không thêm luật hoặc vai mới.

## Avatar tài khoản dùng chung với dashboard

Yêu cầu bổ sung cho [#84](https://github.com/loveoverflowcom/tabula/issues/84): avatar trong game phải là cùng avatar tài khoản đã dùng ở dashboard/header. Đổi ghế, vai, pha hoặc trạng thái sống không đổi hình đại diện. Trạng thái game là marker riêng bên ngoài ảnh; không che mặt bằng mặt nạ/role art.

### Hiện trạng source và contract cần bổ sung

Source `develop@e75624ae` hiện chưa cung cấp avatar dashboard: `apps/web/src/views/home.rs` là resume/catalog, `parts.rs::TopBar` chỉ có navigation/locale; `account.rs::ProfileFacts` hiển thị ID bất biến. `crates/tabula-session-http/src/lib.rs::SelfProfileResponse` chỉ có `version` và `account_id`. Không giả định có field `avatar_url`, tên hoặc resolver sẵn có.

`SeatEntry` trong `crates/tabula-core/src/seat.rs` phân biệt `SeatId` với `Occupant::Human(UserId)`, Bot và Empty. `games/werewolf/src/rules/projection.rs::SeatView` có seat/alive/status/role, không có profile. Host/resource layer cần truyền dữ liệu hiển thị công khai được phép; game rules/projector không gọi profile/network.

| Dữ liệu hiển thị đề xuất | Ownership và cách dùng |
| --- | --- |
| Public subject/occupant reference | Host xác định đúng occupant hiện tại; không lấy seat number làm account identity |
| Display label được phép | Cùng nguồn với dashboard; thiếu thì dùng nhãn khách/default chung, không tự bịa profile |
| Managed avatar asset reference + revision | Resolver chung dashboard/header/game; cache theo subject/ref/revision, crop tròn nhất quán; không fetch URL tùy ý trong game |
| Shared fallback | Cùng initials nếu host đã cho phép display label; nếu chưa có label dùng neutral default; cùng fallback cho load/error/offline |
| Seat → public display map | Host-owned typed presentation input đề xuất, không phải field protocol hiện có; refresh/vacate phải xóa avatar occupant cũ |

Không xuất ID account thô lên UI, không log dữ liệu profile không cần thiết, không đưa profile vào canonical game state/replay. Mapping đổi viewer/occupant phải cập nhật ngay và bỏ callback asset cũ; late load không được gắn avatar người cũ vào ghế mới. Layout giữ nguyên kích thước khi ảnh đang tải/lỗi; tải trước/cache nhỏ gọn qua resource boundary hiện có.

### Proof trong bộ design

`avatar-fixtures.json` là danh sách **tài khoản mẫu**, không phải account thật; cùng `account-avatars.mjs` và `assets/account-avatars/*.svg` cấp ảnh cho mini dashboard/profile, header và ghế. `avatar-sync` minh họa cùng người ở hai bề mặt. Bộ SVG là asset mẫu gốc không mặt nạ; trong sản phẩm resolver dùng ảnh tài khoản thật nếu nguồn công khai đã được triển khai và cho phép. Đây là target contract cần implementation, không tuyên bố đã kết nối API avatar.

## Responsive ưu tiên bàn chơi và hành động

| Viewport tham chiếu | Bố cục và ưu tiên |
| --- | --- |
| 1440×960 desktop | Vòng ghế quanh làng; bài riêng và narrator bên phải; một CTA ở dock |
| 390×844 portrait | Header/pha gọn, avatar 4×3, bài riêng thu thành thanh mở drawer; cả 12 ghế và CTA trong màn đầu |
| 320×640 portrait ngắn | Thu khoảng trắng/cảnh phụ; giữ avatar, nhãn và hit target; panel phụ cuộn, CTA sticky có slot thật |
| 844×390 landscape thấp | Bàn bên trái, thông tin/phím hành động bên phải; panel cuộn trong vùng được dành, không phủ ghế |

Không co toàn bộ vòng desktop thành hình thu nhỏ. Chữ quan trọng tối thiểu 12 px trong reference, touch target ít nhất 44 dp; tên dài truncate có accessible label/full name khi mở thông tin. Bài riêng trong drawer có nút đóng, focus trap/restore và Escape; blur/mất quyền che mặt trước ngay. Header/footer/dock có ownership rõ và `env(safe-area-inset-*)`; CTA nằm trong slot của layout kể cả sticky, không đè hàng avatar cuối hoặc host footer.

390×844 là yêu cầu màn đầu trong điều kiện viewport tham chiếu; font scale 200%, keyboard mở, safe insets lớn hoặc copy dài được cuộn có kiểm soát, không giữ lời hứa fit bằng cách giảm chữ. Log/simulator và helper text là phần ưu tiên thấp, có thể collapsed/scroll. Drawer mở là thao tác chủ động với modal riêng, không ép card vào bàn mobile.

Tham khảo [Town of Salem 2 trên Steam](https://store.steampowered.com/app/2140510/Town_of_Salem_2/) về bầu không khí làng social deduction; lựa chọn của Tabula là cảnh làng làm tâm điểm, HUD gọn và panel phụ có thể thu. Avatar tài khoản và bố cục mobile được thiết kế riêng cho Tabula; không đưa screenshot/artwork Steam vào asset pack.

## Animation cần triển khai

Thời lượng dưới đây lấy từ profile semantic hiện tại, không hardcode lại trong game. Progress, camera, particle và selection thuộc `GamePresentation::Local` (I-10).

| Trigger | Chuyển động | Profile hiện có | Reduced motion / hủy |
| --- | --- | --- | --- |
| Bắt đầu ván | Mặt sau chung đi từ chồng bài đến ghế, stagger 40 ms | card-deal 280 ms | Hiện ngay các mặt sau; không lộ thứ tự/vai |
| Chủ động xem bài mình | Nhấc nhẹ, thu ngang đến giữa, đổi mặt được phép, mở ra và lóe sáng nhẹ | card-reveal 480 ms | Đổi mặt ngay; mất quyền/đổi viewer/blur thì che ngay, không chờ animation |
| Chọn mục tiêu | Chân dung nâng 2–4 dp, vòng focus/selected xuất hiện | turn/vote feedback 160 ms | Vòng và nhãn tĩnh, giữ hit area tối thiểu 44 dp |
| Phiếu công khai được chấp nhận | Token phiếu bay từ ghế đến mục tiêu; số phiếu theo projection cập nhật | vote 160 ms | Cập nhật marker ngay; chỉ dùng khi luật cho phép công khai |
| Chuyển đêm/ngày | Ánh trăng sang ánh bình minh, title pha ngắn và biểu tượng đổi | phase-change 480 ms | Bỏ wash; pha/nhãn cập nhật ngay, skippable |
| Người chết được công bố | Chân dung dịu màu, dấu trạng thái xuất hiện; không lật vai nếu chưa cho phép | presentation local, reuse fade profile | Nhãn tĩnh “Đã bị loại”, không chỉ đổi màu |
| Kết thúc | Sắp xếp bảng kết quả trên cảnh làng, highlight phe thắng | win 800 ms | Bảng tĩnh; bỏ celebration, skippable |
| Cảnh nền | Lửa nhấp nhẹ, tối đa 12 ember; fog rất chậm ngoài vùng chữ | ambient, budget riêng | Tắt hoàn toàn; pause khi hidden/background |

Animation không đổi trạng thái luật, không trì hoãn gửi intent/ack/pha/hạn. Event quá 600 ms cũ thì snap; queue có giới hạn; reconnect không diễn lại toàn bộ quá khứ. Hành động đêm bí mật **không** tạo ready count, token, cue hoặc hiệu ứng thời gian cho ghế khác. Prototype dùng thao tác mẫu cục bộ, không chứng minh điều này ở runtime.

## Lỗi bắt buộc sửa trước polish

1. **Lật dọc vùng clip**: #82 đã quan sát mặt bài, role art và log bị lật, mặt sau/nút ngoài clip không bị. Kiểm `crates/tabula-render-macroquad/src/draw.rs` (`configure_clip_viewport`, camera/scissor/UV) với texture bất đối xứng đánh dấu bốn góc. Chưa có chẩn đoán root cause. Không xoay ảnh nguồn để bù lỗi renderer. Kiểm riêng texture/text trong và ngoài clip, nested clip và clip reset.
2. **Text/nút lệch**: kiểm metrics ascent/descent/baseline tại `crates/tabula-render-macroquad/src/text.rs` và `crates/tabula-presentation/src/button.rs::draw_label`, nhãn tiếng Việt, wrap và icon spacing. Đây là vị trí cần kiểm, chưa xác nhận nguyên nhân.
3. **Host đè footer ở 390×844**: `apps/game-client/web/standalone.css::.runtime-access` và footer `games/werewolf/src/presentation/render.rs` cùng có owner/layout slot rõ; không dùng fixed overlay không dành chỗ.

## Bàn giao và nghiệm thu triển khai

Ownership: `games/werewolf/src/presentation/{mod,render,assets}.rs` cho game feel; `crates/tabula-presentation/src/motion.rs` cho hợp đồng chung; renderer chỉ sửa cơ chế vẽ. Asset runtime riêng tại `games/werewolf/assets/`, version/hash/budget theo pipeline hiện có. Nền mới là art gốc được tạo bằng imagegen; role art tham chiếu pack hiện tại. Đừng tải artwork/role atlas của mọi game vào global startup.

- [ ] Cùng account hiển thị cùng avatar/fallback ở dashboard/header/game; đổi seat/role/phase/death không đổi ảnh. Avatar lỗi/offline/late-load/occupant-change không lưu ảnh người cũ; Bot/Empty/guest có marker và fallback riêng không lộ vai.
- [ ] Dashboard/game dùng chung asset resolver và public display contract đã review; không tự thêm trường profile chưa có vào protocol. Khi chưa có nguồn avatar công khai, reference dùng fixture rõ nhãn và runtime dùng neutral fallback.
- [ ] 390×844: 12 ghế và CTA trong màn đầu; 320×640 và 844×390 không horizontal overflow/đè footer; 44 dp hit target, font-scale 200%, long names, safe insets và keyboard không mất CTA hoặc quyền đóng drawer.
- [ ] Drawer focus trap/restore/Escape và conceal ngay khi blur/viewer/permission đổi; drawer/log scroll không cuốn primary CTA khỏi vùng điều khiển.
- [ ] Ảnh thật ở 1200×880, 1100×850, 390×844, 320×640 và landscape thấp; DPR1/2; chữ và artwork cùng đúng chiều, vùng chọn trùng chân dung.
- [ ] Các vai hiện có, reveal/conceal, selection/submit/skip, phiếu công khai, bình minh, người chết, terminal đều có trạng thái rõ. Snapshot/ảnh không chứa bí mật ngoài projection.
- [ ] Bốn theme và reduced motion: chữ dễ đọc, focus rõ, không chỉ dùng màu; keyboard path hoạt động. Touch/native chỉ nghiệm thu sau chạy trên nền tảng đó.
- [ ] Mất focus, thay viewer/role/pha, người chết và dispose che private art ngay; không còn frame/cue cũ. Animation có cancel/snap và không thay deadline.
- [ ] Đo frame pacing/memory trên thiết bị mục tiêu; benchmark mới quyết định budget, không gắn nhãn 60 FPS từ design. Static cảnh + sprites trước; không đòi shader/ECS mới.
- [ ] Cập nhật `docs/ui/screens/07-werewolf.md` và rubric/presentation-review đang mô tả presenter cũ; giữ scope ADR-0035 và hướng native mobile của #81.
- [ ] PR code dùng `tabula-engineering` + `tabula-game-audit`, targeted checks, conformance/privacy checks nếu chạm game contract, rồi `just check`. Xuất ảnh Macroquad thật; headless snapshot không thay pixel proof.

Phạm vi lượt này: artwork, bố cục, source design, animation spec và issue. Không sửa runtime/luật hoặc mở các gate online/voice/mobile. Prototype HTML là công cụ review; gameplay implementation tiếp tục qua RenderList/Macroquad, không port thành DOM.
