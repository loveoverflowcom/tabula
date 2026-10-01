## Prompt PR A — spec và state contracts
```text
Pin HEAD develop, đọc AGENTS.md / doc 00 / doc 02 / doc 04 / doc 05 / doc 07 và current game-client/Chess/Tiles presentation. Dùng pack 03-gameplay viết spec cho màn 04–07 trong docs/ui/screens/ và loader/reconnect/error state matrix. Map UI actions vào Intent/Input, View vs Local/pending, snapshot/focus/describe; không thay luật. Đặc tả Chess/Tiles theo source thật, đánh dấu Caro/Werewolf future-gated. Tôn trọng shell/game separate document, privacy projection và BoardReader Phase 5 status/actions vs Phase 9 regions. Đừng dùng prototype rules hoặc mock network làm runtime.
```

## Prompt PR B — một presenter hiện có mỗi PR
```text
Sau foundation/spec, chọn Chess hoặc Tiles để cải thiện compact HUD bằng RenderList/Theme và shared widgets. Với Tiles sửa hitarea34 dp thành >=44 dp mà giữ icon visual gọn; với Chess dùng mono tabular clock cùng turn/lowtime state. Đưa rejected/fatal feedback ra UI bằng adapter được duyệt; pending không mutate authoritative View. Bảo toàn promotion/drag hoặc rotate/claim/camera và keyboard/focus. Mỗi PR chỉ một game+shared support thật cần; test hit-testing/input/describe/regression snapshots có review, run targeted checks rồi just check.
```

## Prompt PR C — runtime/a11y milestone sau gate
```text
Chỉ sau Phase 4/5 gate tương ứng, nối loader/reconnect/resync/failed UI với ConnState/asset delivery thật và BoardReader status+actions web. Read AGENTS.md / doc 00 / doc 04 / doc 05, apps/game-client/web/index.html, presenter describe and action dispatch. Giữ real progress/cancel, input disable trong resync, stale view handling, focus and zoom access; không báo recovery succeeded chỉ từ timer giả. ActionId→Intent là residual cần scope/test riêng; chưa có full regions thì ghi Phase 9 defer. Test disconnect/resync/reject, keyboard/AT actions và DOM focus/zoom, ghi native/mobile gaps.
```

