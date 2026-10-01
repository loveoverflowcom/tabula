## Prompt PR A — spec discovery/setup
```text
Pin HEAD develop của loveoverflowcom/tabula, đọc AGENTS.md / doc 00 / doc 02 / doc 04 / doc 07, GameMetadata/GameCapabilities và registry scaffold. Dùng pack 02-discovery để viết docs/ui/screens cho01–03: routes+aliases, desktop/mobile layout, reusable components, typed-data mapping, config/state transition và empty/loading/error matrix. Inventory game availability ở HEAD, phân biệt implemented vs proposed. Không implement registry/shell trước phase gate; nếu metadata thiếu consumer cụ thể thì ghi decision/dependency nhỏ, không hardcode game branches vào core/platform.
```

## Prompt PR B — vertical slice sau gate
```text
Sau foundation và khi registry Phase 4 + shell Phase 5 gate được chứng minh, implement library/detail/setup cho đúng một game hiện đã playable qua registry interfaces. Read AGENTS.md / doc 00 / doc 02 / doc 04 và spec 02. Không link game crate hoặc branch game_id từ shell; configuration được module/authority validate. Search/filter/resume/config summary phải hoạt động với typed data; game/version/resource unavailable có reason+recovery, không fake success. Handoff sang game runtime theo ADR-011, không mount gameplay SVG vào DOM. Test config boundaries, unsupported capability, navigation/back và accessibility; run targeted gates+just check.
```

