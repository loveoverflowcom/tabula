## Mục tiêu
Redesign Ma sói trước Cờ vua theo yêu cầu chủ dự án từ [#82](https://github.com/loveoverflowcom/tabula/issues/82): **bàn làng có nhân vật và chuyển động**, hình/bài đúng chiều, nút thẳng hàng; giảm cảm giác một màn hình đầy nút số. Giữ Rust/Macroquad và primary tím của Tabula.

**Ảnh dưới đây là bản thiết kế với dữ liệu mẫu**, xuất từ SVG; không phải screenshot Macroquad. HTML/CSS/MJS có choreography mẫu để review, không chứa game rules/network. Source đối chiếu `develop@e75624ae870a74f62f0f734fbcf2f12043047dd4`, ngày 06/10/2026.

## Thiết kế mới
![Ma sói — bàn làng ban đêm, bài riêng đúng chiều](https://github.com/loveoverflowcom/tabula/raw/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/desktop-night.jpg)

<details>
<summary>Bình minh, phiếu công khai và mobile</summary>

![Ma sói — bình minh](https://github.com/loveoverflowcom/tabula/raw/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/desktop-day.jpg)
![Ma sói — phiếu công khai](https://github.com/loveoverflowcom/tabula/raw/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/desktop-vote.jpg)
![Ma sói — mobile 4×3, cuộn trang và footer riêng](https://github.com/loveoverflowcom/tabula/raw/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/mobile-night.jpg)

</details>

- Chọn trực tiếp **chân dung quanh lửa trại**; 12 avatar trung tính, không mã hóa vai. Mobile dùng lưới 4×3, không thu nhỏ vòng desktop.
- Bài riêng và lời quản trò là vùng phụ; một CTA theo pha. Selected/focus có viền + nhãn, không chỉ màu.
- Nền đêm/bình minh cùng bố cục, ánh sáng chuyển mượt; artwork dịu sau chữ. Vai, tên, trạng thái và nút là typography live.
- Phiếu chỉ minh họa công khai khi được phép; hành động đêm không có ready count/effect/cue cho ghế khác. Simulator controls ở vùng phụ.
- Mobile có slot thực cho dock/footer, được cuộn; ảnh mobile là full-page 390×1160, không hứa mọi nội dung nằm trong 390×844.

## Animation bàn giao
Tham chiếu profile trong `docs/ui/tokens.json` và doc 04 §9; progress ở `GamePresentation::Local` (I-10).

| Trigger | Choreography | Duration/profile | Reduced motion |
| --- | --- | --- | --- |
| Chia bài | Mặt sau chung bay tới ghế, stagger 40 ms | card-deal 280 ms | Hiện ngay mặt sau |
| Mở bài mình | Nhấc nhẹ → thu ngang → đổi mặt được phép → mở + highlight | card-reveal 480 ms | Đổi mặt ngay |
| Chọn người | Chân dung nâng 2–4 dp, vòng selected và nhãn | 160 ms | Marker tĩnh |
| Phiếu công khai | Token tới mục tiêu, counter theo projection | vote 160 ms | Counter cập nhật ngay |
| Đêm → ngày | Crossfade hai cảnh, sun/moon và phase title ngắn | phase-change 480 ms, skippable | Pha mới hiện ngay |
| Công bố người chết | Dịu chân dung, thêm nhãn; không tự lật vai | reuse fade profile | Nhãn tĩnh |
| Kết thúc | Composition phe thắng trên bàn làng | win 800 ms, skippable | Bảng tĩnh |
| Ambient | Lửa/ember tối đa 12 hạt, fog nhẹ ngoài chữ | budget presentation | Tắt; pause khi hidden |

Mất quyền/đổi viewer/blur/phase phải **che private art ngay**, không chạy nốt flip. Animation không giữ input, ack, phase hoặc deadline; event cũ hơn 600 ms snap, queue bounded. Reduced motion và đổi cảnh phải cancel cả WAAPI/timeline còn chạy. Prototype chỉ minh họa, không chứng minh privacy/runtime.

## Sửa lỗi từ #82 trước polish
1. **Vùng clip bị lật dọc**: kiểm `crates/tabula-render-macroquad/src/draw.rs`, camera/viewport/scissor/UV với texture bất đối xứng đánh dấu 4 góc. Chưa xác nhận root cause; không xoay asset nguồn để bù renderer.
2. **Nhãn/nút lệch**: kiểm actual glyph ascent/descent/baseline trong `text.rs` và `button.rs::draw_label`, wrap tiếng Việt/icon spacing. Vị trí nghi vấn từ source, chưa phải chẩn đoán.
3. **Host đè footer ở 390×844**: phối hợp `standalone.css::.runtime-access` với `games/werewolf/src/presentation/render.rs`; mỗi vùng có owner/slot riêng.

## Source và nghiệm thu
[Bộ source, SVG/PNG/JPG và art](https://github.com/loveoverflowcom/tabula/tree/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf) · [Hướng triển khai + acceptance đầy đủ](https://github.com/loveoverflowcom/tabula/blob/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/IMPLEMENTATION.md) · [HTML](https://github.com/loveoverflowcom/tabula/blob/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/index.html) · [Motion MJS](https://github.com/loveoverflowcom/tabula/blob/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/preview.mjs) · [Exporter](https://github.com/loveoverflowcom/tabula/blob/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/export_preview.py) · [Nguồn ảnh/hash](https://github.com/loveoverflowcom/tabula/blob/a5a5224c2662459c614b494aaf1479ce35af5df5/docs/ui/issue-82-game-feel/werewolf/ASSETS.md)

- [ ] Sửa đảo clip/baseline/footer; ảnh Macroquad thật ở 1200×880, 1100×850, 390×844, 320×640, landscape thấp, DPR1/2. Texture/text/target cùng đúng hướng.
- [ ] Reveal/conceal, các vai hiện có, target/submit/skip, dawn/death/public ballot/terminal rõ; projection là authority, không tiết lộ private choices.
- [ ] Bốn theme, reduced motion, keyboard/focus, touch target và interruption/viewer change; đo frame pacing/memory trên platform được nghiệm thu.
- [ ] Game presentation sở hữu art/motion; renderer chỉ cơ chế, asset pack per-game. Cập nhật screen/rubric Ma sói đang mô tả presenter cũ.
- [ ] PR dùng `tabula-engineering` + `tabula-game-audit`, targeted checks rồi `just check`; source design không thay runtime golden/receipt của #82.

**Phạm vi:** thiết kế và issue; runtime/luật chưa sửa. ADR-0035 local simulator giữ scope hiện tại; online/voice/native mobile có issue/gate riêng (#81). Thứ tự: issue này trước, issue Cờ vua tiếp theo; #51 giữ vai trò umbrella UI.

**Bằng chứng lượt thiết kế:** đã inspect 4 ảnh tĩnh, syntax MJS và compile exporter PASS; chưa chạy browser prototype hoặc game runtime (NOT_RUN). Không có đổi runtime, PR hay merge vào develop.
