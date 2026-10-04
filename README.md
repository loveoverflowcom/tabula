# Tabula

Tabula là runtime board game bằng Rust: luật xác định, SDK cho từng game,
projection theo người xem và công cụ replay. Nền tảng sở hữu cơ chế; từng game
sở hữu luật. Thêm game không được tạo nhánh xử lý theo `game_id` trong platform.

Đọc [mục lục tài liệu](docs/README.md) và
[doc 00 — Architecture Principles](docs/architecture/00-architecture-principles.md)
trước khi sửa code. [AGENTS.md](AGENTS.md) hướng dẫn cách làm việc;
[hai skills chính](.agents/skills/README.md) cung cấp workflow và kỹ thuật theo nhu cầu.

## Kiến trúc và trạng thái

| Thành phần | Trách nhiệm và phạm vi hiện tại |
|---|---|
| `tabula-core`, `tabula-game-api`, `games/*` | Kernel, hợp đồng game và luật thuần Rust; luật không đọc clock, socket hoặc DB |
| `tabula-registry` | Catalog và dispatch qua hợp đồng chung; discovery/setup có slice được mở theo ADR-0028 |
| `tabula-presentation`, `renderer-*` | Chuyển projection thành `RenderList`, xử lý input và render; trạng thái UI nằm ngoài canonical state |
| `apps/game-client` | Gameplay Macroquad native trên desktop và WASM trên web |
| `apps/web` | Shell Leptos CSR; discovery/setup và handoff local có phạm vi ADR-0028/0030 |
| `mobile/` | Một cây Compose Multiplatform cho Android/iOS theo ADR-0032; hiện là foundation, WebView gameplay và voice còn gated |
| `apps/desktop` | Shell Tauri tùy chọn; gameplay không phụ thuộc Tauri |
| `services/tabula-server`, `tabula-match`, `tabula-storage` | Kiến trúc multiplayer server-authoritative, Tokio/Axum và PostgreSQL; phần runtime ngoài slice được mở vẫn theo phase gate |
| `services/tabula-auth` | Skeleton backend auth dùng Kanidm theo [ADR-0034](docs/adr/0034-kanidm-auth-service-skeleton.md); TODO trong Rust, chưa có login/session runtime cho #54 |

Web shell và gameplay là hai WASM bundle/document riêng (ADR-011).
Hướng mobile dùng CMP quản lý UI/navigation, rồi nhúng gameplay Rust/WASM qua
`GameHost`; foundation chưa chứng minh game chạy trong WebView trên thiết bị.
Xem [ADR-0032](docs/adr/0032-compose-multiplatform-mobile-host.md) và
[mobile README](mobile/README.md) để biết phạm vi đã mở.

Với multiplayer, luồng thiết kế là: client gửi command → platform xác thực và
sắp thứ tự → game áp dụng `Input` → platform lưu kết quả → `project`/`view_event`
tạo `View`/`ViewEvent` cho từng người xem. Client không nhận canonical `State`.
Luồng online này không được suy ra từ việc demo local chạy được.

## Cấu trúc repo

| Thư mục / file | Nội dung |
|---|---|
| [`crates/`](crates/README.md) | Thư viện platform, SDK, presentation và adapters |
| [`games/`](games/README.md) | Mỗi game một crate; rules/bots/presentation tách bằng features |
| [`apps/`](apps/README.md) | Game runtime, web/admin và desktop shell |
| [`mobile/`](mobile/README.md) | `shared/`, `android/`, `ios/` trong một Gradle root |
| `services/`, `deploy/` | Binary server và cấu hình triển khai theo phase |
| [`xtask/`](xtask/README.md), [`justfile`](justfile) | Automation và lệnh tiện ích |
| `tests/` | Replay goldens và harnesses theo phạm vi triển khai |
| [`docs/`](docs/README.md) | Architecture, ADR, game/UI specs, queue và bằng chứng |
| [`.agents/skills/`](.agents/skills/README.md) | Skills chính; `.claude/skills` trỏ về cùng cây |
| [`tokens.toml`](tokens.toml), [`deps.toml`](deps.toml) | Nguồn authored cho design tokens và luật dependency |

Hai báo cáo gốc [nghiên cứu stack](rust-first-cross-platform.md) và
[nghiên cứu thị trường](deep-research-report.md) là nguồn lịch sử. Hợp đồng hiện tại
nằm trong docs 00–09 và ADRs; các báo cáo cũ không xác định tiến độ triển khai.

## Chạy local

Dùng toolchain pin trong [`rust-toolchain.toml`](rust-toolchain.toml), không chọn
version theo báo cáo nghiên cứu cũ. Web cần `trunk`; `just` là wrapper tùy chọn.

```bash
git clone https://github.com/loveoverflowcom/tabula.git
cd tabula
git switch develop

# Gameplay native local
cargo run -p tabula-game-client

# Shell discovery/setup, gameplay chưa được bind
cd apps/web
trunk serve
```

Từ repo root, `just wasm-serve` build/stage gameplay WASM riêng;
`just web-local-serve` build và serve shell cùng handoff gameplay local opt-in
(ADR-0030). Đây là local play, không phải tài khoản hoặc multiplayer server.
Lệnh và prerequisites mobile nằm trong [mobile README](mobile/README.md).

## Kiểm tra trước PR

```bash
cargo xtask check  # tương đương just check, portable core gate

# Skills, resource links và bridge
python3 .agents/skills/tabula-engineering/scripts/check_skills.py
python3 .agents/skills/tabula-engineering/scripts/test_check_skills.py
python3 .agents/skills/tabula-engineering/scripts/test_ai_doc_contracts.py
```

Kiểm tra skills cần Python 3 và PyYAML. CI còn kiểm tra feature matrix và WASM;
thay đổi game/mobile có các kiểm tra bổ sung trong [AGENTS.md](AGENTS.md).
Ghi đúng command, source ref, kết quả và phần chưa kiểm chứng khi báo cáo.

[Roadmap](docs/architecture/07-phases-and-implementation-roadmap.md) quy định gates;
[work queue](docs/work-plan/README.md) ghi các slice và prerequisites.
`LOCK NOW`, `EXPERIMENT`, `DEFER` là trạng thái quyết định, không phải dấu hiệu
một tính năng đã được implement hoặc chạy trên thiết bị.
