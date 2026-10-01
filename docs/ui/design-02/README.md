# Tabula Design 02 · seven scoped screen packs

Design only. The editable SVGs and PNG previews are static references with sample data, not runtime screenshots. Code comparison was pinned to develop @ 44f6b74e07648abc7191363d7582efc1fceab262. Future implementation must re-pin HEAD and verify phase gates.

## Download one small pack per issue

- [01-foundation](01-foundation/README.md): [ZIP](packages/01-foundation.zip) · [editable screen navigator](01-foundation/index.html) · [mobile and state preview](01-foundation/previews/mobile-states.png)
- [02-discovery](02-discovery/README.md): [ZIP](packages/02-discovery.zip) · [editable screen navigator](02-discovery/index.html) · [mobile and state preview](02-discovery/previews/mobile-states.png)
- [03-gameplay](03-gameplay/README.md): [ZIP](packages/03-gameplay.zip) · [editable screen navigator](03-gameplay/index.html) · [mobile and state preview](03-gameplay/previews/mobile-states.png)
- [04-replay](04-replay/README.md): [ZIP](packages/04-replay.zip) · [editable screen navigator](04-replay/index.html) · [mobile and state preview](04-replay/previews/mobile-states.png)
- [05-xiangqi](05-xiangqi/README.md): [ZIP](packages/05-xiangqi.zip) · [editable screen navigator](05-xiangqi/index.html) · [mobile and state preview](05-xiangqi/previews/mobile-states.png)
- [06-accounts](06-accounts/README.md): [ZIP](packages/06-accounts.zip) · [editable screen navigator](06-accounts/index.html) · [mobile and state preview](06-accounts/previews/mobile-states.png)
- [07-lobby](07-lobby/README.md): [ZIP](packages/07-lobby.zip) · [editable screen navigator](07-lobby/index.html) · [mobile and state preview](07-lobby/previews/mobile-states.png)

## Coverage and ownership

All 22 original input screens have an editable source. Screen 11's generic replay controller belongs to 04-replay; Xiangqi-specific PV and evidence extend it in 05-xiangqi. Screens 05 and 07 share an explicit adapter/recovery design containing distinct Caro and Ma sói panels. Seven purpose-built mobile boards and an additional four-theme parity source are included.

- 01-library → 02-discovery / Thư viện game
- 02-game-detail → 02-discovery / Chi tiết Cờ vua
- 03-new-match → 02-discovery / Thiết lập local
- 04-chess → 03-gameplay / Chess HUD
- 05-caro → 03-gameplay / Caro / Werewolf / Recovery
- 06-tiles → 03-gameplay / Tiles HUD
- 07-werewolf → 03-gameplay / Ma sói adapter
- 08-xiangqi → 05-xiangqi / Xiangqi Chơi
- 09-result → 04-replay / Kết quả ván
- 10-history → 04-replay / Ván của tôi
- 11-analysis → 04-replay / Replay core
- 12-learn → 05-xiangqi / Xiangqi Gia sư
- 13-settings → 01-foundation / Cài đặt
- 14-resources → 05-xiangqi / Tài nguyên
- 15-login → 06-accounts / Đăng nhập
- 16-register → 06-accounts / Tạo tài khoản
- 17-rooms → 07-lobby / Phòng chơi
- 18-room → 07-lobby / Phòng chờ
- 19-queue → 07-lobby / Ghép trận
- 20-profile → 06-accounts / Hồ sơ
- 21-friends → 06-accounts / Bạn bè
- 22-design-system → 01-foundation / Bộ thành phần
- 11-xiangqi-analysis → 05-xiangqi / Xiangqi Phân tích

## Shared foundation

01-foundation owns exact read-only snapshots of canonical TOML, generated CSS and JSON, plus a small component/state contract. The source of truth remains tokens.toml; no parallel product theme is introduced. Other packs contain their own scoped screens and reference foundation ownership.

## Approved M3 Expressive revision and verification

The original purple, sage and warm-board visual vocabulary is retained. The redesign reduces navigation chrome to a 176px rail and 64px topbar, replaces oversized editorial headings/heroes with compact task hierarchy, prioritizes the board and keeps 44dp action targets. Current Chess/Tiles module availability is distinguished from planned portal, online and Xiangqi features.

Each pack is small and contains no original 10.82MB ZIP, heavy external images, binaries, font files, node_modules or app bundle. Validation covers XML parsing, source syntax, image sizes, linked file existence and visual inspection. This is not a runtime, accessibility, network or AI certification.

The approved revision is grounded in the Google research and official Material Components component documentation. Connected groups, contained lists, expressive button sizes/shapes and floating toolbars are design references adapted to Leptos/Macroquad, not an already installed native kit. No production code is changed.
