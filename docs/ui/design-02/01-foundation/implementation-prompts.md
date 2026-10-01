## Prompt PR A — spec và theme
```text
Trong loveoverflowcom/tabula, pin HEAD develop, đọc AGENTS.md, docs architecture 00/04/07/09, tokens.toml và pack 01-foundation. Đọc shared/material-component-map.md trong pack; tạo spec giao diện gọn và component/state matrix ở docs/ui/screens; tách màn 13 settings khỏi màn 22 showcase. Inventory token đã có, giữ một nguồn tokens.toml, đặc tả M3 Expressive hierarchy/tonal roles/shape/connected controls, typography, contrast, focus, 44 dp targets, normal/compact density và reduced-motion. Cho phép role/value changes có rationale và consumer, kiểm đủ bốn scheme; không dùng palette hardcoded từ reference image. Chỉ sửa token generator/schema nếu có consumer cụ thể và compatibility rõ. Không implement Phase 5 shell. Nghiệm thu bằng token generation/drift checks nếu sửa token; report phần source-read, test thật và chưa chạy.
```

## Prompt PR B — lát cắt component có consumer thật
```text
Sau PR A, pin lại HEAD, đọc AGENTS.md / doc 00 / doc 04 và spec foundation. Chọn consumer gameplay đã chạy (Chess hoặc Tiles), thêm bộ widget nhỏ RenderList-based cho button/icon-button/focus/dialog hoặc banner đúng nhu cầu, rồi migrate consumer đó. Không xây toàn bộ inventory trong một PR. Dùng Theme semantic tokens và component styling M3 Expressive đã duyệt; tonal grouping thay decorative border/shadow, keyboard/input/focus thống nhất, state layer và target 44 dp; giữ Local/UI state ngoài canonical State. Test hit area, disabled/focus/activation, reduced-motion, light/dark/high-contrast và regression presenter. Chạy targeted tests rồi just check, không auto-regenerate goldens.
```

## Prompt PR C — shell/preferences sau gate
```text
Chỉ khi Phase 4 exit được chứng minh và Phase 5 được mở, implement shell layout và trang settings (màn 13) bằng Leptos/component foundation; native shell chỉ ở phase phù hợp. Đọc AppState/preferences target trong apps/web/src/main.rs và doc 04, dùng generated style/tokens.css. Persist qua adapter/prefs được duyệt, tôn trọng OS reduced-motion, đảm bảo deep link/back/keyboard. Không thêm auth/privacy/storage behavior chưa có backend. Kiểm chứng resize 320–1440 px, zoom 200%, drawer/bottomnav/focus restoration và theme parity; ghi rõ platform chưa test.
```

