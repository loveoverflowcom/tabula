# Nguồn asset và public identity mẫu

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/issue-82-game-feel/werewolf).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


| Asset | Nguồn / mục đích |
| --- | --- |
| `assets/village.png`, `assets/village.webp` | Cảnh làng gốc do ImageGen tạo trong phiên redesign; art nền của game |
| `assets/village-dawn-original.png`, `assets/village-dawn.webp` | Artwork bình minh có sẵn trong pack, không encode role/private state |
| `assets/werewolf.png` | Role art runtime hiện có từ `games/werewolf/assets/werewolf@2x.png`, `develop@e75624ae870a74f62f0f734fbcf2f12043047dd4`; chỉ ở bài riêng |
| `assets/account-avatars/account-01.svg` … `account-12.svg` | Portrait SVG gốc của fixture thiết kế, bỏ toàn bộ mặt nạ/sigil; không phải avatar tài khoản thật |
| `avatar-fixtures.json` | 12 public-subject mẫu với tên, initials, avatar asset/version; nguồn chung cho board/header/đối chiếu hồ sơ |
| `account-avatars.mjs` | Resolver cùng origin + fallback initials + generation guard dùng chung |
| `portraits.mjs`, `portrait-data.json` | Adapter tương thích lấy cùng fixture, không còn hình hoặc mask cũ |

Không gọi avatar API bên ngoài, không lấy dữ liệu từ tài khoản thật. `sample-public-subject-*` chỉ là stable fixture key; không phải token/auth identity đã được backend hỗ trợ. Khi tích hợp, host cung cấp identity/public display resolver riêng, không dùng seat index hoặc role assignment để chọn avatar.

## Kích thước và hash asset

Bản source PNG gốc giữ trong ZIP tổng; pack tải tối ưu dùng hai WebP, role PNG và 12 avatar SVG. Đây là size file thiết kế, không phải memory/FPS budget runtime.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `assets/account-avatars/account-01.svg` | 1384 | `6eba18842d363c628894c0522b9c13e4c37aef25cfcd74e5f444045cd68a0cef` |
| `assets/account-avatars/account-02.svg` | 1387 | `8dcd5931ed2b52916a1f11a5286e9fa997b57c10f51b69ab4e5960a2e043391e` |
| `assets/account-avatars/account-03.svg` | 1385 | `88580d8ba04511e59e5e7ded1d5e6367f33d1fe5061d11b700f2beaedce7bb36` |
| `assets/account-avatars/account-04.svg` | 1390 | `d6e75597625a9c0376837fe10d0cff0385e0b68208ea8edca5b5b9bef94a278b` |
| `assets/account-avatars/account-05.svg` | 1382 | `ed5d6450a8349e8a4631c6ec571a591473239c132792b3c91f4cc568512055cf` |
| `assets/account-avatars/account-06.svg` | 1388 | `4275d74d86a557433a1dcc5d6d6d4b6a2ae45b0f19c8b59adc883bb34a77d11b` |
| `assets/account-avatars/account-07.svg` | 1383 | `2632f326419d34f218d94381853aa07dfb6c79d352bea1c395301947c7489223` |
| `assets/account-avatars/account-08.svg` | 1386 | `def89f2d9568cc64f35f5fa9e383dc89ed294c3e285dfe288c4f6473e4626ed8` |
| `assets/account-avatars/account-09.svg` | 1381 | `a0997c71aa8667fad264213b73554aade7bc3a7fa3b8cd2b1c5e247fb0224275` |
| `assets/account-avatars/account-10.svg` | 1389 | `5440398a4162effffa6e4e0e6295d7799e869c449267e497df36c185b35bbe7d` |
| `assets/account-avatars/account-11.svg` | 1380 | `25cea7a306b3aee600cdbcd0c9e8be9aafb9e1a2e845ad43b381c881ca37057b` |
| `assets/account-avatars/account-12.svg` | 1388 | `3659af01c209b403689a80c6adc7b0aa4a5f26f97da38fde1e7ddf35720c6d98` |
| `assets/village-dawn-original.png` | 3051337 | `d49fb3223bb7cc99bfb7d71ad6d821d1f87b0a2d7f0251ccf1942a0e51da5733` |
| `assets/village-dawn.webp` | 199664 | `5f4634fd3f18b4dae291738150e97de3f724de8536e79ec0798714e457c8ece6` |
| `assets/village.png` | 2753512 | `9bd72067406f625900a88548bab2721e48ec90d7c4dcc68d096a31988df6212d` |
| `assets/village.webp` | 225300 | `8090f92c623e34646acb66b1e813985242ef5d699b22092328dcfac5cc6f7607` |
| `assets/werewolf.png` | 212880 | `4ba002f53e1a5560fc86c13dd2be19d3c2bfed16ccc5a95cd12e35cc5a1f5b04` |
