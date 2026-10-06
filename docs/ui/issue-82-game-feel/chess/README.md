# Cờ vua — Bàn gỗ trầm

Bản thiết kế • dữ liệu mẫu • mô phỏng cục bộ. Bàn cờ 2D có khung gỗ trầm, ô màu ivory/plum, quân Staunton rõ hình và cùng hướng. Primary/focus tím; viền brass là chất liệu của quân cờ. Không dùng Unicode, emoji hoặc ảnh quân cờ được sinh bằng imagegen.

## Mở bản tương tác

Chạy server static từ thư mục pack rồi mở `chess/index.html`. Module đọc `model-data.json` qua HTTP; mở trực tiếp bằng `file://` không phải đường chạy được hỗ trợ. Không có dependency tải từ CDN.

`pieces.svg` và `PIECE-PROVENANCE.md` được lấy từ asset thiết kế hiện có trong repository `develop`, giữ nguyên silhouette. HTML và exporter cùng sử dụng các symbol này.

## Các tình huống mẫu

- Chuỗi cố định: `e2 → e4`, `d7 → d5`, `e4 → d5`, `g8 → f6`. Trang mở ở sau nước thứ hai, chọn tốt e4 để thấy gợi ý mẫu d5. Đây là bốn nước hợp lệ từ bàn xuất phát; prototype không có bộ kiểm luật đầy đủ.
- Chọn quân chỉ thay đổi selection. Nút “Xem nước mẫu…” thực hiện nước kế tiếp của chuỗi cố định. Không click-to-send, không lịch sử SAN được suy đoán.
- Hướng trắng: files a…h, ranks 8…1; hướng đen: files h…a, ranks 1…8. Đổi map của ô và tọa độ; hình quân luôn upright.
- Phong cấp là tình huống riêng: vua trắng e1, vua đen e8, tốt trắng a7 và ô a8 trống. Modal có Hậu/Xe/Tượng/Mã bằng cùng bộ sprite; không ghép vào lịch sử bốn nước.
- Không đồng hồ. Không giả lập online, reconnect, chat hoặc voice.

## Chuyển động mẫu

Nước đi dùng lift/arc/settle 280 ms; quân bị bắt shrink/fade 160 ms; dấu lượt 160 ms. Ô nguồn/đích giữ marker sau nước đi. Trạng thái mẫu cập nhật trước khi choreography chạy; animation không giữ trạng thái gameplay.

Đổi hướng, đặt lại, chuyển tình huống, giảm chuyển động, blur và hidden đều cancel/snap về endpoint đã commit. Chọn ô khi animation đang chạy cũng cancel trước khi render để tránh hai bản sao quân cờ. Giữ focus của cùng tọa độ sau khi dựng lại các ô. Modal dùng native `<dialog>` và nút Q/R/B/N có nhãn; Tab/Enter/Space có đường keyboard chuẩn. Không tuyên bố hỗ trợ phím mũi tên như một widget chess grid.

`window.designPreview` cung cấp `advance`, `flip`, `setReduced`, `reset`, `openPromotion`, `promote`, `selectSquare`, `getState`. State trả về position, step, orientation, selection, fixture, promoted, captured, coordinate order và số animation đang giữ.

## Ảnh review và kiểm chứng

`export_preview.py` đọc cùng `model-data.json` và cùng silhouette từ `pieces.svg`. Chạy với Python, Pillow và Inkscape để tái xuất các reference:

| Reference | Kích thước | Nội dung |
| --- | --- | --- |
| `desktop.svg/png/jpg` | 1440 × 960 | Sau hai nước, e4 được chọn, d5 là gợi ý mẫu |
| `mobile.svg/png/jpg` | 390 × 1050 | Bố cục dọc, bàn có cùng position |
| `black-orientation.svg/png/jpg` | 1440 × 960 | Hướng đen, tọa độ đảo, quân giữ upright |
| `promotion.svg/png/jpg` | 1440 × 960 | Tình huống riêng và modal Q/R/B/N |

Đây là **static vector references**, không phải browser screenshots hoặc ảnh Macroquad thật. Exporter bố trí cùng thiết kế và dữ liệu, không thay thế layout engine của browser. Shadow trong SVG dùng ellipse tương thích Inkscape; HTML dùng CSS drop-shadow.

- Source syntax và model/coordinate checks: PASS.
- Xuất SVG → PNG/JPG, kiểm tra kích thước, nhìn desktop/mobile/hướng đen/phong cấp: PASS.
- Browser interaction, animation playback, viewport screenshot và accessibility tree: **NOT_RUN** vì browser runtime không có trong môi trường này.
- Rust/Macroquad runtime, native mobile, network/conformance: **NOT_RUN**; không sửa runtime trong lượt thiết kế này.

Thông số/choreography triển khai và điều kiện nghiệm thu thực tế ở `IMPLEMENTATION.md` của pack. Không coi các ảnh reference này là bằng chứng renderer đã sửa camera/UV/baseline.
