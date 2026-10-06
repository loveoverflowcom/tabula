# Dashboard — khôi phục Tabula Design 01

Tham chiếu cho yêu cầu sửa dashboard thấy trong [#82](https://github.com/loveoverflowcom/tabula/issues/82). Visual oracle là bộ **Tabula Design 01**, ngày 01/10/2026, được đọc lại từ bản bàn giao gốc `tabula-design-html-mjs-png.zip` (version 0).

`design-01/` giữ nguyên byte 16 file được chọn từ bộ 22 màn: source HTML/CSS/MJS và ba ảnh của màn 01. Đây là archive tham chiếu gốc, chưa sửa runtime hoặc thiết kế lại. `PROVENANCE.json` ghi hash archive/source/PNG và kích thước.

- Màn đầu gốc là `#library`, gộp Home + Catalog theo `docs/design-notes.md`.
- Desktop: `design-01/previews/desktop/01-library.png`, 1440×1408 full page.
- Mobile: `design-01/previews/mobile/01-library.png`, 390×844, bottom navigation fixed.
- Mobile full: `design-01/previews/mobile-full/01-library.png`, 390×1631; nav chỉ đổi thành in-flow khi capture để bàn giao toàn nội dung.
- Mở `design-01/standalone.html#library` bằng trình duyệt sau khi tải; file nguyên bản tự chứa CSS/JS. `index.dev.html` dùng source MJS khi chạy static server.

Original README mô tả gói đủ 22 màn; thư mục này chỉ lưu các reference liên quan dashboard cùng source prototype. Sample tên, đồng hồ, hồ sơ, AI, phòng và trạng thái là dữ liệu minh họa; khi triển khai dùng registry/capability/state thật.

UI runtime đối chiếu: `/` tại `develop@e75624ae870a74f62f0f734fbcf2f12043047dd4`, PNG trong #82 chụp Chromium thật 1100×850 DPR1. Design PNG là prototype; khác kích thước/locale nên so hierarchy, composition, art và tokens, rồi chụp baseline/app ở cùng viewport khi nghiệm thu.

Source runtime liên quan: `apps/web/src/views/{home,parts,library}.rs`, `apps/web/style/app.scss`, `docs/ui/screens/01-library.md`. Design 01 quyết định diện mạo cần khôi phục ở màn này; PR cập nhật tài liệu dashboard nếu quy định Design 02 hiện tại mâu thuẫn với yêu cầu đó. Các game redesign ở #84/#85 là việc riêng.

Trong lượt ghi issue đã đọc source và inspect các ảnh hiện trạng/gốc; chưa chạy browser hoặc app mới.
