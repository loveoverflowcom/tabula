## Prompt PR A — spec theo #48, không code game
```text
Pin HEAD develop, đọc AGENTS.md / doc 00 / doc 02 / doc 04 / doc 07 / doc 09 và issue #48+comment ownership. Dùng pack 05-xiangqi viết docs/ui/screens cho08/11extension/12/14, state+capability+platform matrix và proposed adapter boundary; reuse generic replay controller từ nhóm UI 04 (replay chung). Hiện chưa có games/xiangqi, đánh dấu proposed paths thay vì claim implementation. Chốt mode/fairplay/evidence identity/stale/cancel/fallback/resource-license states. Hòa giải No ML/plugin phases chỉ qua ADR/vision scope được duyệt, không tự sửa phase gate hoặc implement engine/rules trong PRspec.
```

## Prompt PR B — board/mode slice sau rules gate
```text
Chỉ khi issue #48 vision/ADR đã hòa giải và Xiangqi SDK rules+projection/replay slice thật có tests, implement Play/Analyze mode shell/presentation dùng một state authority. Read AGENTS.md / doc 00 / doc 02 / doc 04 + spec 05; gameplay Macroquad, document shell Leptos, Local analysis branch riêng accepted match. Cho no-engine/no-LLM capability UI đúng, không fake scores/PV. Reuse foundation+generic replay; validate all executed moves qua rules / apply. Test branch isolation, mode permissions, legal command rejection và unsupported platform, run existing conformance+targeted UI gates.
```

## Prompt PR C — engine/evidence/resources, một adapter mỗi PR
```text
Sau engine adapter thật được duyệt và đúng artifact license/distribution gate, nối Analyze/Resources rồi Learn theo từng PR nhỏ. Pin HEAD, read issue #48/spec 05 và adapter API; analysis request gắn game/session/rules/position digest/engine revision/budget/request ID, cancel và discard stale results. Render score/PV có budget+source, optional LLM chỉ giải thích evidence; missing LLM vẫn structured hints, missing engine báo unavailable. Không auto-download/cloud fallback, không mutate match từ tutor. Test late-result/vị trí mới, cancel/timeout/crash, hash/license/compatibility failures, fairplay authority và offline capabilities riêng native/web. Distinguish doubles vs engine integration evidence.
```

