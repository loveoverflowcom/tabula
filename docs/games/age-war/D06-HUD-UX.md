# Age War D06 — HUD/UX và bàn giao trước code gate

**2026-10-10 · DRAFT / OWNER REVIEW PENDING.** [D06 #124](https://github.com/loveoverflowcom/tabula/issues/124)
thuộc [#118](https://github.com/loveoverflowcom/tabula/issues/118). Rules/catalog
`0.1.0-d01` **UNBALANCED**, D05 reconstructed v05, runtime packs `0.5.0`,
preview `d06-hud-0.1`. Base `develop@74589b4`.

Đây là thiết kế và presentation tooling, không phải game. Chưa có lời duyệt
rules/art/rights hay ngoại lệ authority; **C01 #125 vẫn BLOCKED**. Đóng issue,
merge code tooling, ảnh đẹp hoặc test xanh không thay lời duyệt của owner.

## 1. Phạm vi và nguồn

- [RULES](RULES.md), [CONTENT](CONTENT.md), [MATH](MATH.md) là nguồn ý nghĩa.
  Preview lấy tên/giá/pop/timing/phép/tháp/tech trực tiếp từ catalog Rust D01
  qua `age_war_d05_pilot hud-catalog`, không nhập một bảng balance khác bằng JS.
- [D05 ledger](../../verification/age-war-d05/README.md) sở hữu bằng chứng pack,
  motion, timing oracle, memory/audio và các lỗ hổng G1–G8. D03 backdrop và D05
  nhân vật trong preview được trích từ chính pack đó; art giữ riêng ngoài Git.
- [Preview source](../../../games/age-war/design-tools/hud-preview/README.md),
  [screen/state → intent/event → asset map](../../../games/age-war/design-tools/hud-preview/states.mjs),
  [D06 ledger](../../verification/age-war-d06/README.md) liên kết thiết kế với bằng chứng.
- Art chưa được duyệt: xem được trong private review bundle không đồng nghĩa có
  quyền phân phối. Base states, turret đủ state và HUD icon production còn thiếu.
  Không che khoảng hở bằng công trình vẽ hộp hoặc nhân vật hình học.

## 2. Ownership: một HUD và một authority

| Surface | Owner khi C03 được phép | D06 hiện tại |
|---|---|---|
| Library/detail, độ khó/start-age/tutorial/settings, availability | CMP mobile / Leptos web shell; native launch theo GameHost | setup sheet là minh hoạ, không đăng ký game vào catalog |
| Pack loading, integrity/budget failure, cancel launch | host/resource loader; real bytes progress | fixture nêu rõ tiến độ mẫu; staging kiểm bytes thật trước export |
| Battlefield, HP/clock/economy, tray/queue/spells/tháp, aiming/codex/tech/tutorial, pause | Rust game presenter → `RenderList` → renderer hiện hữu | HTML/canvas là bản thiết kế; không đưa HTML vào native app |
| Kết quả, rematch/navigation | shell sau `GameHost Exited{outcome}` (doc 04 §1.1) | dialog kết quả chỉ biểu diễn tài liệu shell |
| Combat/legality/cost/cooldown/target/outcome | pure Rust rules trong authority local được duyệt, qua project/view_event | **chưa implement**; preview không tính combat hoặc phát canonical event |

CMP không vẽ lại thanh tuyển/phép của Rust, không chạy combat. Web giữ document
handoff ADR-011; mobile phải native Rust/Macroquad trong cùng app CMP theo
[ADR-0043](../../adr/0043-native-mobile-gamehost.md), không WebView fallback.
Preview HTML/Xvfb không là bằng chứng Android/iOS chơi được.

## 3. Bố cục chiến trường

Chiến trường ngang là trọng tâm. Camera review giữ focus window D03 **1920 lu**
(10000 q) theo chiều ngang; làn được dời trong khoảng trống của HUD, không đổi
world geometry. Ground row 490/698; lane band từ 135 lu trên ground đến 12 lu
dưới ground. Box bảo vệ làn bao gồm thân/vũ khí/marker của roster review, không
là collider hay bảo đảm mọi VFX nằm trong đó.

| Vùng | Nội dung và quy tắc |
|---|---|
| Thanh trên | HP hai căn cứ, age mở tech, nâng age, pause, clock. Không banner trang trí lớn |
| Minimap | Cửa sổ camera và hướng hai căn cứ; drag battlefield/arrow là đường input trong preview. Tap minimap/accessibility hit area là việc C03 |
| Dock trái | Gold, tổng XP, population gồm cả đã reserve; không gọi XP là số dư tiêu được |
| Dock giữa | Đúng **6 quân** của current/previous age + toggle; roster 36 quân vào codex |
| Trên tray | 5 purchase entry, từng target đủ cao để chọn/huỷ. Chỉ head train; blocked spawn vẫn là head |
| Dock phải | 2 socket tháp + đúng 2 phép chỉ huy. Autocast/passive skills không thêm nút |
| Lane | Nhân vật thật, hướng vẽ độc lập, marker ● ta / ◆ địch; không chỉ dựa màu áo |

Khung review: 844×390 touch (safe L/R47, B21), 1024×768 touch (T/B20),
1280×720 pointer, 1920×1080 pointer. Chữ 100/130/200%. Target tối thiểu 44 lu
(touch), 36 lu (pointer); đây là điều kiện layout, không lời hứa support thiết bị.
Ở compact, giá ở mặt thẻ; tên dài dùng ellipsis và detail sheet/accessible name.
Ở font lớn, bỏ dòng tên trên thẻ và giữ tên đầy đủ ở sheet. Không giảm font bằng
CSS transform để giả đáp ứng 200%. Font metrics/clipping thật phải kiểm browser.
Economy cao đủ ba dòng chữ lớn trong khoảng trống bên trái; age chip compact
dùng I–VI và giữ tên đầy đủ ở accessible name/tech sheet để tránh tràn chữ.

Portrait: giải thích “Xoay ngang để tiếp tục”, đề xuất pause qua host; rotate
trở lại vẫn cần Resume rõ. Không thu nhỏ desktop thành HUD không đọc được.
Insets thực lấy từ native host, không đóng đinh notch fixture vào production.

## 4. Input và trạng thái

[`states.mjs`](../../../games/age-war/design-tools/hud-preview/states.mjs) là
bảng máy đọc được: mỗi screen có owner, states, intent/payload, proposed events,
assets và a11y. Events là **đề xuất ViewEvent**, D01 chưa có event enum/ABI.
Tên payload chưa là wire contract; C01 phải xác lập typed IDs/version.

| Hành động | Mouse / keyboard | Touch | Phản hồi / cancel |
|---|---|---|---|
| Recruit | click, 1–6 | tap thẻ | cost/pop/train + lý do gold/pop/queue; không đổi fixture để giả đã mua |
| Chi tiết/codex | right-click hoặc focus + I; C mở codex | long-press 500ms | sheet tên đủ/role/counter hypothesis; long-press không gửi thêm Recruit |
| Bộ quân | G, nút ↔ | tap toggle | current/liền trước; age1 không có bộ trước; Tab giữ focus bình thường |
| Huỷ queue | click purchase; Backspace chọn head | tap ô rồi xác nhận | waiting hoàn100%; started/blocked head floor75%; production dùng projection |
| Phép | Q/E, arrows trên lane, Enter xác nhận | chọn phép → chạm/kéo → Xác nhận | Esc/right-click huỷ trước pause; cooldown số còn lại, không chỉ ring |
| Camera | drag lane, arrows khi không aim | kéo lane | chỉ Local; không đổi target/hit/collider canonical |
| Nâng age/tech | U nâng; R mở tech | thanh trên | nêu vàng + tổng XP +5s, research slot chung; không huỷ/refund |
| Tháp | T hoặc socket | tap socket | hai nhánh, build/refit cùng nhánh/sell với xác nhận và refund theo HP |
| Pause | Esc khi không aim | nút pause | Resume; restart/quit/resign có xác nhận. System Back là hợp đồng native còn thiếu |
| Rematch | nút tài liệu kết quả | tap | cùng config/new seed do host; không sửa lại outcome cũ |

State inventory bao gồm setup/difficulty/tutorial, loading/integrity/budget/native
unavailable, battle/queue, thiếu vàng/hết pop/queue full, spell ready/cooldown/aim/cancel,
age locked/research/transition/enemy age, turret build/refit/sell, critical base,
fatigue12 phút/hard limit20 phút, pause/restart/quit/resign, win/lose/draw/rematch,
portrait. Toolbar chọn fixture; không phải mọi variant đều có scene riêng đã render.

Accessibility design: native button semantics, focus-visible, modal focus trap
bằng `<dialog>`, Esc/restore focus, tên/giá/lý do bằng text, HP/value không chỉ màu,
đồng hồ/critical/fatigue có text. Countdown tương lai derive từ `ready_at_tick`
và projected logical time. Browser AT, keyboard/touch thực và screen reader còn
BLOCKED/NOT_RUN; chưa được gắn nhãn “accessible game”. Canvas Board Reader/action
bridge C03 phải dùng cùng View/Local, không lấy canonical State (I-5/I-6/I-10).

## 5. Art, motion, effects và ngân sách

Private `assets.json` kiểm kê từng crop: pack@version, logical AssetRef, hash page;
full-motion pages ghi full BLAKE3/bytes, source rect, logical box/pivot, duration,
loop và thời gian frame. Hai facing có atlas độc lập; không lật ảnh để giả parity.
36 unit icons là crop idle **DERIVED stand-in**, spell icons là crop VFX contact.
Không gọi đó là D04 portrait/icon production. Auxiliary turret study vẫn PARTIAL;
D03 base states không có trong bộ D05 đã nhận. SourceQA remote còn thiếu.

Browser storyboard 11s dùng toàn frame bake24Hz + exact marker/end poses:
walk/run → attack → skill (nếu có) → hit → death → idle. Time slider,0.5/1/2×,
2 commander cues/age và đổi age bằng fixture. Không mô phỏng lựa chọn mục tiêu,
projectile arrival, damage, income, training, research hoặc thắng/thua bằng JS.
Marker vẫn không có authority. Full/low/reduced đổi trình diễn; reduced bỏ burst,
không flash toàn màn/camera shake. Chưa có pixel QA của bản HTML.

D05 pilot đã chạy thật qua verified assets → RenderList → Macroquad;
[ledger](../../verification/age-war-d05/README.md) giữ evidence/limits. Web preview
không thay thế evidence C03 event-driven gameplay. Không có audio audition của
người; `--audio` ở pilot là đường nghe riêng, HTML preview mặc định im lặng.

Ngân sách đề xuất cần owner/chủ renderer quyết định, **chưa là số đo thiết bị**:

- Critical start chỉ scene/current needed unit-facings/UI; common SFX/VFX có bound.
- Current/previous/living older entities phải cùng tồn tại; G1 single-pack binding
  hiện không đáp ứng. C03 cần multi-pack residency hoặc composed match pack.
- D05 lineup d1 ~117MiB resident, d2 ~213MiB; renderer density theo DPI (G2) gây
  nặng khi stage chỉ0.44×. Đề xuất density theo texel need/host budget, có clamp,
  thay vì tải d2 vì điện thoại DPI3; chưa sửa renderer trong D06.
- Web disk cache target150MiB/native300MiB (doc04 §12) khác texture residency.
  Bundle review toàn6age lớn hơn web cache, không phải production download plan.
- Prefetch next age theo bound trong5s research, không nạp toàn roster6age. Không
  retire old group khi còn living/queued/impact refs; failure giữ error/retry rõ.
- Blend sprite cut G3, loop fade/steal G4, thin VFX G6 cần quyết định/owner review.
  Mục tiêu60FPS cần device/browser frame-time và GPU memory; Xvfb không chứng minh.

## 6. Clock/authority và code gate

[COMPATIBILITY](COMPATIBILITY.md) vẫn là **draft**, không ADR được duyệt.
Đề xuất A: offline PVE local authority, giữ Timer ABI,50ms/tick, host pump tối đa4
ordered tick rồi yield; bốn accepted input ⇒ version+4. Input semantic effective
next tick, host clock/bot/pause, projection/event/replay ordering theo RULES.
Không JS/Kotlin combat, không `Input::Player(Frame/Advance)`, không DB/render frame.
Chưa chọn A thì C01 không khởi động; invariant exception phải qua doc00 §7.1.

| Definition of Ready | Evidence hiện tại | Người/ngày duyệt thực |
|---|---|---|
| D01 rules/economy/roster/skills/transition | spec+catalog+numeric checks; chưa sim/balance | **PENDING / chưa ghi nhận** |
| D02 style + D03–D05 đủ approved assets | private delivery/D05 verified subset; base/turret/icon/SourceQA/rights gaps | **PENDING / chưa ghi nhận** |
| D06 tổng thể legible, motion/input ở khung review | executable preview, layout tests; browser/AT/visual review BLOCKED | **PENDING / chưa ghi nhận** |
| Offline PVE authority, clock A và lifecycle | COMPATIBILITY draft; không ADR exception đã duyệt | **PENDING / chưa ghi nhận** |
| Source/rules/art versions, exceptions và license | versions ở đầu tài liệu; missing ghi rõ | **PENDING / chưa ghi nhận** |
| Handoff checklist/test plan sang C01–C04 | bảng dưới; planned acceptance riêng | documented; không tự mở gate |

Khi owner review, ghi đúng người, ngày, source commit, rules/config/art versions,
quyết định A/B và exceptions. Thay luật/art sau đó cần version diff/review phần
đổi. D06 không tự tick ô acceptance của issue hoặc tự đóng #123/#124/#125.

## 7. Handoff C01–C04

| Công việc | Prerequisite và acceptance phải giữ |
|---|---|
| [C01 #125](https://github.com/loveoverflowcom/tabula/issues/125) | D06 owner approved + authority decision/ADR cần thiết; implement pure rules/project/view_event, typed commands/events/timestamps/config/rules hash. RULES R9 independent examples, transactional reject, conformance, replay/terminal exactly once, caps; cross-target equality chỉ sau đối chứng |
| [C02 #126](https://github.com/loveoverflowcom/tabula/issues/126) | C01;3 fair bot policies chỉ View, seeded/versioned pacing; equal-gold/equal-pop scenarios và side swaps. Headless ≥1000 matches local/on-demand theo issue, report uncertainty/WLD/timeouts/seed/replay; không claim cân bằng từ mirror50% |
| [C03 #127](https://github.com/loveoverflowcom/tabula/issues/127) | C01/C02 + approved art đầy đủ6age/36unit/12spell/tháp/base; typed intent↔ViewEvent map; giải G1/G2/G3/G4 có review. Mouse/key/touch no double-trigger, queue cancel, cooldown, age mixed residency. Same viewport/scene/state screenshot/video runtime so D06; font/safe area/theme,30/60FPS/spikes/reduced không đổi replay |
| [C04 #128](https://github.com/loveoverflowcom/tabula/issues/128) | C03; PVE đầu-cuối3difficulty + human motion/readability/listening/playtest; leak/rematch/long-match/frame/load/RAM/GPU measurements. Android/iOS native same-app open/input/Back/reopen/background/surface/DPI separately; adapter unavailable giữ BLOCKED, không gọi mobile complete |

Owner review checklist: mở cả6age; đọc tên dài ở100/130/200%; kiểm target/safe
area bằng browser thực; soi full motion và foot/weapon contact, support/spell
telegraph, crowd at cap48 rồi stress100; nghe mix; duyệt base/tháp/icons/rights;
chốt authority/lifecycle. Retain failing evidence, không regenerate baseline để
che lỗi. [D06 ledger](../../verification/age-war-d06/README.md) phân biệt test,
source-read, captures và phần chưa kiểm.
