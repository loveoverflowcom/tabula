# Nguồn và asset trình diễn

- `village.webp`: derivative của cảnh ImageGen `exec-7c159190-5976-4b13-a976-58a083eb5808.png`, nguyên bản được tạo trong phiên này.
- `village-dawn.webp`: derivative của cảnh bình minh ImageGen `exec-5fbe68ab-88ae-4f3d-9117-65c61b8f62bf.png`, giữ bố cục cùng làng.
- `werewolf.png`: giữ nguyên runtime `games/werewolf/assets/werewolf@2x.png`, source commit `e75624ae870a74f62f0f734fbcf2f12043047dd4`, SHA Git blob lấy qua GitHub connector.
- `portraits.mjs`: 12 SVG chân dung trung tính do agent tạo bằng code; màu tóc/da/áo là identity mẫu, không biểu diễn role. Mặt nạ và bài che giống nhau.

| Asset | Bytes | SHA-256 |
|---|---:|---|
| village.webp | 225300 | `8090f92c623e34646acb66b1e813985242ef5d699b22092328dcfac5cc6f7607` |
| village-dawn.webp | 199664 | `5f4634fd3f18b4dae291738150e97de3f724de8536e79ec0798714e457c8ece6` |
| werewolf.png | 212880 | `4ba002f53e1a5560fc86c13dd2be19d3c2bfed16ccc5a95cd12e35cc5a1f5b04` |

SVG/PNG/JPG xuất bởi `export_preview.py` là ảnh thiết kế tĩnh. Không phải bằng chứng browser, native Macroquad hay game authority. Browser chạy mẫu vẫn chưa được kiểm thử vì Chromium không có trong môi trường và download bị chặn.

Đã chạy syntax check cho MJS và compile Python. Đã nhìn pixel của desktop đêm, desktop bình minh và mobile 4×3: chân dung/lá bài dựng thẳng, CTA nằm trong dock, footer ngoài bàn, không chồng lấn ở các export này.
