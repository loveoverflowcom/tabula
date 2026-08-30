# Tabula — Nền tảng Board Game Cross-Platform bằng Rust

**Tabula** là một runtime nền tảng board game được xây dựng bằng Rust, cho phép tạo và triển khai nhiều trò chơi board độc lập (cờ vua, bài, social-deduction, tile-placement, party games) mà không cần sửa đổi code nền tảng.

```
                       ┌──────────────────────────┐
                       │      Rust Workspace      │
                       └────────────┬─────────────┘
                                    │
                 ┌──────────────────▼──────────────────┐
                 │         tabula-core                 │
                 │                                     │
                 │ Rules / State / Action / RNG        │
                 │ Turn / Replay / Validation          │
                 │ Visibility / PlayerView             │
                 │ NO Macroquad / NO HTTP / NO DB      │
                 └──────────┬────────────────┬─────────┘
                            │                │
              WASM/native   │                │ native
                            │                │
          ┌─────────────────▼───┐       ┌────▼──────────────────┐
          │ Client              │       │ Backend               │
          │                     │       │                       │
          │ Macroquad           │       │ Tokio + Axum          │
          │ 2D gameplay         │       │ WebSocket + HTTP      │
          │                     │       │ SQLx + PostgreSQL     │
          └──────────┬──────────┘       └────────┬──────────────┘
                     │                           │
        Web/WASM ────┤                           │
        Android ─────┤                           │
        iOS ─────────┘                           │
                                                 │
          ┌─────────────────────┐                │
          │ Leptos              │◄───────────────┘
          │                     │
          │ Landing / Lobby     │
          │ Account / Chat      │
          │ Shop / Dashboard    │
          │ Admin / CMS         │
          └─────────────────────┘
```

## 🎮 Tính năng chính

- **Runtime Game-Agnostic**: Lõi kinh doanh độc lập với logic game cụ thể
- **Multiplayer Server-Authoritative**: Máy chủ kiểm soát tất cả trạng thái game
- **Cross-Platform**: Web (WASM), Android, iOS, Desktop
- **Realtime Communication**: WebSocket cho multiplayer hiệu năng cao
- **Replay & Versioning**: Ghi lại toàn bộ trò chơi, hỗ trợ phiên bản game
- **Scalable Architecture**: Modular monolith có thể phát triển sang microservices

## 🛠️ Tech Stack

| Lớp | Công nghệ |
|-----|-----------|
| **Game Rules** | Pure Rust crate (không phụ thuộc) |
| **Shared Protocol** | Rust + Serde |
| **Gameplay Client** | Macroquad (2D graphics) |
| **Web Portal** | Leptos (SSR + reactivity) |
| **Backend** | Tokio + Axum |
| **Realtime** | WebSocket |
| **Database** | PostgreSQL + SQLx |
| **Asset Delivery** | Object storage + CDN |
| **Deployment** | Container + managed PostgreSQL |

## 📚 Cấu trúc dự án

```
tabula/
├── docs/
│   └── architecture/          # Hướng dẫn thiết kế chi tiết
│       ├── 00-architecture-principles.md    # Nguyên tắc & ADR
│       ├── 01-stack-and-repository-plan.md  # Stack & cấu trúc repo
│       ├── 02-game-module-and-sdk-design.md # Game SDK
│       ├── 03-backend-and-multiplayer-plan.md
│       ├── 04-frontend-and-design-system.md
│       ├── 05-data-protocol-and-replay.md
│       ├── 06-scaling-deployment-and-observability.md
│       ├── 07-phases-and-implementation-roadmap.md
│       ├── 08-first-games-validation-plan.md
│       └── 09-synthesis-and-decision-register.md
├── rust-first-cross-platform.md    # Nghiên cứu về stack Rust
├── deep-research-report.md         # Báo cáo nghiên cứu thị trường
└── README.md                       # File này
```

## 🚀 Bắt đầu nhanh

### Yêu cầu hệ thống

- Rust 1.70+ (cài đặt qua [rustup](https://rustup.rs/))
- PostgreSQL 14+ (cho backend)
- Node.js 18+ (nếu sử dụng tooling frontend)

### Cài đặt

```bash
# Clone dự án
git clone <repository-url>
cd tabula

# Kiểm tra cấu trúc Rust workspace
cargo --version
cargo metadata --format-version 1 | jq '.workspace_members'
```

### Đọc tài liệu kiến trúc

**Cho người mới:**
```
00 (Architecture Principles)
→ 01 (Stack & Repository)
→ 07 (Roadmap)
```

**Để implement game:**
```
00 → 02 (Game SDK)
→ 08 (First Games)
→ 04 (Frontend)
```

**Để work trên backend:**
```
00 → 03 (Backend & Multiplayer)
→ 05 (Protocol & Replay)
→ 06 (Scaling & Observability)
```

**Để work trên client:**
```
00 → 04 (Frontend & Design System)
→ 05 (Protocol)
```

## 📋 Quy ước code

- **Crate prefix**: `tabula-` (ví dụ: `tabula-core`, `tabula-backend`)
- **Game crates**: `tabula-game-<slug>` trong thư mục `games/`
- **Invariants**: Ký hiệu `I-*n` được định nghĩa tại doc 00 §7
- **Decisions**: Ký hiệu `ADR-*nnn` (Architectural Decision Record) tại doc 00 §10

## 📖 Các tài liệu chính

| # | File | Khi nào đọc |
|---|------|-----------|
| 00 | Architecture Principles | **Luôn đọc trước** — Các invariant, ADR |
| 01 | Stack & Repository Plan | Tạo crate, thêm dependency, setup CI |
| 02 | Game Module & SDK Design | Implement game contract |
| 03 | Backend & Multiplayer Plan | Implement gateway, sessions, persistence |
| 04 | Frontend & Design System | Implement Leptos shell, Macroquad client |
| 05 | Data Protocol & Replay | Đồng bộ wire protocol, serialization |
| 06 | Scaling & Observability | Deploy, monitor, scale |
| 07 | Phases & Roadmap | Planning & execution |
| 08 | First Games Validation | Chọn & implement game reference |
| 09 | Synthesis & Decision Register | Quick answers & LOCK/EXPERIMENT/DEFER |

## 🎯 Status Markers

Các tài liệu kiến trúc sử dụng ba trạng thái:

- **LOCK NOW** — Quyết định chính thức. Không bàn lại mà không ADR mới.
- **EXPERIMENT** — Hướng đã chọn, chi tiết chưa kiểm chứng. Build behind a seam.
- **DEFER** — Cố tình không xây ngay. Giữ tường seam, không viết code.

## 💡 Kiến trúc chính

### Core (`tabula-core`)
- Trạng thái game & action handling
- Turn management
- Replay & validation
- Player view & visibility
- **Không phụ thuộc**: Macroquad, HTTP, Database

### Backend
- **Framework**: Tokio + Axum
- **Protocol**: WebSocket + HTTP
- **Database**: PostgreSQL + SQLx
- Qản lý session, match actors, persistence

### Client
- **Gameplay**: Macroquad (2D graphics)
- **Portal**: Leptos (web SSR + reactivity)
- **Targets**: Web (WASM), Android, iOS, Desktop

## 🔄 Multiplayer Flow

1. **Client** gửi action qua WebSocket
2. **Backend** nhận, validate, apply vào `tabula-core`
3. **Backend** broadcast state update cho tất cả clients
4. **Client** render state mới

## 📊 Thị trường & Context

- **Board Game thị trường**: ~18.95 tỷ USD (2026), dự báo 30.06 tỷ USD (2031)
- **Online board games**: ~2.72 tỷ USD (2026)
- **Competitors**: Board Game Arena (1347 games), Chess.com (250M+ users)
- **Opportunity**: Tabula là platform cho độc lập game creators

## 🤝 Đóng góp

Xem tài liệu kiến trúc trước khi contribute. ADR và Invariants là mandatory reference.

## 📝 License

[Chưa xác định — Xem CLAUDE.md hoặc LICENSE file khi có]

## 📞 Contact

Maintainer: Love Overflow  
Email: manh.pd1@kiotviet.com

---

**Bắt đầu từ đâu?**
- Tìm hiểu tổng quan? → Đọc `docs/architecture/00-architecture-principles.md`
- Muốn implement game? → Đọc `docs/architecture/02-game-module-and-sdk-design.md`
- Cần setup dev environment? → Đọc `docs/architecture/01-stack-and-repository-plan.md`
- Xem timeline? → Đọc `docs/architecture/07-phases-and-implementation-roadmap.md`
