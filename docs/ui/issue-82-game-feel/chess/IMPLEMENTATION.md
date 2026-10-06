# Cờ vua — bàn chơi có chiều sâu, chuyển động có chủ đích

Thực hiện sau [Ma sói #84](https://github.com/loveoverflowcom/tabula/issues/84), bổ sung cho [#82](https://github.com/loveoverflowcom/tabula/issues/82) và umbrella UI #51. Source đối chiếu `develop@e75624ae870a74f62f0f734fbcf2f12043047dd4` ngày 06/10/2026. Ảnh và HTML là design reference với fixture mẫu, không phải Macroquad screenshot hoặc multiplayer evidence.

## Hướng thiết kế

Giữ Rust/Macroquad và hệ quân Staunton SVG nguyên bản của Tabula. Tăng cảm giác đang chơi trên bàn cờ thật bằng khung có độ dày, chất liệu tiết chế, bóng tiếp xúc dưới quân và rõ silhouette; không cần đổi engine hoặc làm 3D. Board là vùng chính, HUD gọn theo chiều rộng bàn. Sidebar lịch sử chỉ rộng vừa nội dung, không vùng trắng trống lớn như ảnh #82. Primary/focus/selected vẫn là tím; màu vật liệu bàn/quân là decorative game art, không override semantic danger/legal-target/threat.

- Quân trắng/ngà và đen/midnight giữ khác biệt rõ ở kích thước nhỏ; bóng không làm mất nét ở ô tối. Texture ở cường độ thấp, không che legal dots/check/last move. h1 là ô sáng.
- Hai player bars trên/dưới: tên/ghế và nhãn lượt. Không dùng “Local player” cho phiên có account/server authority. Fixture được ghi dữ liệu mẫu, không giả người/tài khoản thật.
- Chỉ có clock nếu View/capability cung cấp; bản thiết kế có thể dùng ván không đồng hồ. Không tự tạo rating, latency hoặc kết nối online.
- Move strip dùng coordinate hiện được cung cấp (e2–e4), không bịa SAN/full history ở reconnect. Move/check/outcome đều đến từ projection, không suy authority từ animation.
- Hành động phụ như đảo bàn/hòa/đầu hàng gom gọn; enabled/disabled/confirmation theo legality thật. Nút không làm canvas co giãn sau mỗi turn. Board-reader/keyboard path được giữ.
- Mobile tái bố trí player–board–player–history/actions theo cột, không sidebar nổi đè bàn; footer có chỗ thực, safe-area rõ; portrait/landscape đều giữ board vuông. 390 px ưu tiên ô ~44 dp; ở 320 px phải có input/Board Reader phù hợp, không tuyên bố ô nhỏ đạt 44 dp.

## Animation

Presenter hiện có MotionTimeline piece_move nhưng phần vẽ chủ yếu lerp origin. Mở rộng local choreography trên RenderList/Sprite/transform/opacity hiện có, không đưa progress/camera/particle vào State hoặc wire (I-10).

| Trigger | Choreography | Token/budget | Reduced motion và hủy |
| --- | --- | --- | --- |
| Hover/chọn quân | Tăng bóng tiếp xúc, nhấc 2–4 dp, vòng selected; legal targets có dot/ring | informative feedback 160 ms | Marker/nhãn tĩnh |
| Accepted move | Nhấc nhẹ → đường đi với arc thấp → hạ, scale 0.94→1.0 | piece-move 280 ms | Snap ô mới ngay |
| Capture | Quân bị ăn thu nhẹ + fade, quân đến hạ vào ô đích; cập nhật captured strip theo View | reuse informative fade, đề xuất160 ms | Xóa ngay, không ảnh ma |
| Castling | Vua và xe cùng một composition/timeline; endpoint theo cùng accepted move | piece-move 280 ms | Cả hai snap cùng lúc |
| En passant | Pawn bị ăn fade ở ô thật của nó, không giả ở destination | same capture profile | Xóa đúng ô ngay |
| Promotion | Chooser gần ô đích, quân lựa chọn upright; sau accepted projection đổi sprite | panel informative profile | Chooser/replace tĩnh |
| Đổi lượt | Emphasis chuyển giữa player bars; nhãn lượt luôn tồn tại | turn 160 ms | Đổi nhãn/marker ngay |
| Check | Vòng threat rõ trên vua + nhãn “Chiếu”; một emphasis ngắn, không pulse liên tục | reuse threat/turn feedback | Threat marker tĩnh |
| Rejected move | Discard PendingCommand/preview, trả quân đúng nguồn + feedback | invalid profile hiện có80 ms | Text/marker ngay |
| Kết thúc | Outcome nhỏ hiện cạnh bàn, spotlight phe thắng; bàn cuối còn nhìn thấy | win 800 ms, skippable | Kết quả tĩnh |

Timing dùng semantic profiles; hiện prose doc04 minh họa invalid120 ms trong khi token có80 ms: hòa giải tài liệu, không nhân hai nguồn thời lượng. Capture160 ms là đề xuất cần map vào profile hiện có hoặc thay authored token có rationale.

Animation không làm lệnh hợp lệ, không chờ mới gửi intent, không dừng clock và không trì hoãn quyền/terminal. Preview chỉ là PendingCommand tách khỏi authoritative View. Mất authority/dispose/blur thì cancel và xóa output theo host contract; đổi orientation/reset/reduced motion phải snap hoặc cancel timeline đúng target. Event stale >600 ms snap, queue bounded; resume render current projection thay vì replay mọi hiệu ứng cũ. Không claim reconnect/resync đã xong từ việc có UI busy.

## Hướng bàn và lỗi renderer

White: file a→h, rank8→1. Black: h→a, rank1→8. Flip đổi mapping ô/tọa độ/hit testing và vị trí player bars; **không xoay glyph/texture quân 180°**. Selection/last action giữ square identity. Promotion panel đổi placement theo ô đích nhưng quân chữ vẫn upright. Tránh lật toàn RenderTexture có cả chữ.

Lỗi lật trong #82 quan sát chắc ở Ma sói, chưa chứng minh Chess có cùng lỗi. Shared renderer cần texture test 4 góc, sprite/text trong/out clip/nested clip, native/browser, DPR1/2. Nút lệch cần kiểm baseline/glyph metrics/wrap tại `crates/tabula-render-macroquad/src/text.rs` và `crates/tabula-presentation/src/button.rs::draw_label`; không chỉ dịch chữ bằng offset theo một font.

## Ownership và source

- `games/chess/src/presentation/mod.rs`: piece move composition, board geometry/input, projection-driven transients.
- `games/chess/src/presentation/hud.rs`: compact bars/actions, check/outcome/promotion state.
- `crates/tabula-presentation/src/motion.rs`: shared profile/timeline contract; renderer mechanism ở `crates/tabula-render-macroquad`.
- Reuse `games/chess/assets/source/pieces/` và pack pipeline per-game. SVG gốc do Tabula tự tạo, provenance giữ tại `PIECE-PROVENANCE.md`; design preview không cần thay vector bằng imagegen.
- Prototype dùng chuỗi fixture e2–e4, d7–d5, e4–d5, g8–f6; promotion fixture riêng. Không phải full chess rules, SAN engine, replay hoặc mạng.

## Nghiệm thu triển khai

- [ ] Ảnh thật initial/select/move-midflight/capture/castle/en-passant/promotion/check/terminal ở White và Black; hình quân, chữ, coordinates và hit target cùng đúng chiều.
- [ ] Bốn theme, reduced motion, keyboard/touch/Board Reader; 1100×850, 1440×960, 390×844, 320×640, landscape thấp, DPR1/2. Không host/footer/action đè canvas hoặc label.
- [ ] Projection thay đổi giữa motion/flip/promotion, input tiếp trong animation, reject/cancel/reset/dispose; không ghost piece, duplicate endpoint hoặc command tạo từ frame cũ.
- [ ] Khác biệt fixture vs actual account/clock/history rõ; eligibility draw/resign/claim vẫn lấy View/capability. Animation không thay server/game authority.
- [ ] Đo frame pacing, input latency và memory trên platform mục tiêu; thiết kế không tự chứng nhận60 FPS/native mobile (#81).
- [ ] Focused presentation/headless checks + pixel capture thực; conformance nếu chạm contract/rules; `tabula-engineering` và `tabula-game-audit`, rồi `just check` cho PR code. Golden chỉ đổi sau review có chủ đích.

Lượt này chỉ bàn giao design/source/spec/issue. Không đổi luật, server, persistence, auth, protocol hoặc backend; không merge runtime. HTML là công cụ review, implementation tiếp tục qua Rust presentation/Macroquad.
