# Age War — đề xuất tương thích và quyền quyết định

**Trạng thái: DRAFT / CHƯA PHÊ DUYỆT.** Đây là bản đề xuất ADR hẹp của D01,
không phải ADR đã được đánh số hay một quyết định trong register. Đọc cùng
[RULES](RULES.md) và [QA-PLAN](QA-PLAN.md).

Mốc khảo sát: `develop@023ecf296352b2f3285c9dbd9c703fe867c9d6cb`, ngày 2026-10-08.
Các nhận xét hiện trạng dưới đây là **source-read**. D01 chỉ chuẩn bị tài liệu,
schema và kiểm tra toán/validation; chưa có `GameRules`, `GameModule`, vòng chạy,
registry launch, bot chạy thật hay số đo. [#119](https://github.com/loveoverflowcom/tabula/issues/119)
thuộc chuỗi [#118](https://github.com/loveoverflowcom/tabula/issues/118):
design → art direction → assets → duyệt HUD → C01. PR design không tự mở C01.

## 1. Vì sao cần quyết định riêng

[Doc 00 §1.2](../../architecture/00-architecture-principles.md#12-non-goals)
loại real-time action netcode; [doc 08 §6](../../architecture/08-first-games-validation-plan.md#6-what-these-four-do-not-cover)
không dùng game liên tục để chứng minh SDK board-game. Age War có chuyển động,
va chạm, cooldown và nhiều quyết định trong một giây. Gắn nhãn `Simultaneous`
hay dùng số nguyên chưa giải quyết được khác biệt tải và lập lịch.

**Đề xuất xin phép:** chỉ một first-party **offline PVE**, một người với một bot,
trong local authority hiện có. Khối authority tin cậy chạy rules/projection trong
SDK/app; presenter và renderer vẫn chỉ nhận `View`/`ViewEvent`. Không đưa authority
vào animation, Kotlin/Swift, JavaScript hay input cảm ứng. Đây là mở rộng hẹp của
local scope, không phải bằng chứng rằng client online được quyền quyết định.

[ADR-0030](../../adr/0030-local-discovery-gameplay-handoff.md) và
[ADR-0035](../../adr/0035-werewolf-local-simulator.md) cho thấy có local host được
cô lập, không cấp phép tự động cho game mới hay cadence mới. D01 chưa sửa các ADR
đó hoặc doc 00. Trước runtime, owner phải chấp nhận ngoại lệ sản phẩm và cách mở
local slice; nếu cần sửa contract chuẩn, phải làm đúng doc 00 §7.1 trong PR riêng.

Giữ nguyên I-1–I-16 và R1–R8. Không network PvP, prediction/rollback netcode,
ranked, reward/currency liên trận, SQL per frame, engine mới, physics/ECS chung,
platform branch theo game id, hay lách phase gate. Không quảng bá Age War là
một reference game thứ năm thay thế bốn benchmark của doc 08.

## 2. Contract được đề xuất

- Canonical tọa độ là số nguyên: 1 world unit = 1.000 subunit. Tốc độ,
  damage, HP, economy, thời hạn và so sánh quyết định kết quả đều không dùng float.
  Quy tắc chia/làm tròn và số dư chuyển động phải ghi rõ, không lệ thuộc FPS
- Quantum logic **50 ms = 20 tick/s**. Theo RULES R0/R1, mọi due time là **tick nguyên**;
  phase ordering trong tick quyết định expiry, control, contact và damage.
  Windup/cooldown/travel được author theo tick ngay từ data, không hứa hỗ trợ
  deadline millisecond nằm ngoài grid 50 ms hoặc tự round float/frame time
- Một lần dispatch/catch-up dự kiến gom tối đa **4 tick = 200 ms**. Mọi tick,
  deadline và collision trong phần đó vẫn được xử lý theo thứ tự. Gom công việc
  không có nghĩa nhảy một bước `x += velocity × 200` hoặc bỏ các hit ở giữa
- Người chơi và bot chỉ phát semantic command được RULES định nghĩa. Không có
  player command `Frame`, `Tick`, `Advance`, tọa độ mouse, `delta_time` hay damage
  tự khai. Timer/admin là system input, không phải quyền của người chơi
- I/O, clock thật, pacing và lifecycle ở shell. `Ctx.now` là thời gian đã được
  ghi nhận; rules không đọc `Instant`, `get_time`, frame time hay clock hệ điều hành
- Event là sự kiện có nghĩa và có timestamp logic cần thiết cho replay/animation;
  không ghi một event cho mỗi pixel. Nội suy hình ảnh nếu cần chỉ diễn giải các
  projection đã được phép, không quyết định position/hit/legality

RULES R0 đã đề xuất pop24/side, drone4/owner và8/match, shots128, pending
semantic deadlines512/match. C01 phải enforce/validate các bound đó và chốt thêm
per-input/output-event bound trước decoder/runtime; không xem data cap là đã
được enforce. Đổi cap cần decision và cập nhật oracle/config cùng nhau. 20 tick/s không phải 20 hoặc 60 player command/s.
Ví dụ timer 50 ms trong 10 phút tạo 12.000 timer input, chưa kể command. Đây là
phép tính thiết kế, **không phải throughput đã đo** và không tạo nghĩa vụ ghi DB.

## 3. Hiện trạng và khoảng hở có thật

| Điểm | Nguồn ở base | Hệ quả và việc còn cần |
|---|---|---|
| Pure apply / projection | [`rules.rs`](../../../crates/tabula-game-api/src/rules.rs), [doc 02 §3](../../architecture/02-game-module-and-sdk-design.md#3-gamerules--the-functional-core) | Có boundary phù hợp; D01 chưa triển khai nó cho game này. Presenter không được giữ canonical `State` |
| Thời gian của input | [`ctx.rs`](../../../crates/tabula-game-api/src/ctx.rs) | `Ctx.now` có một thời điểm/input, không có dt hay tick batch. Cursor tick/phase và cách nhận command next-tick phải có nghĩa rõ ràng |
| Timer carrier | [`input.rs`](../../../crates/tabula-game-api/src/input.rs), [`effect.rs`](../../../crates/tabula-game-api/src/effect.rs) | `Input::Timer` chỉ có `TimerId`; `SetTimer` có delay nguyên. Không có generation/payload/batch size để giả định dùng sẵn |
| Scheduler local | [`LocalMatch::advance_to`, `next_due_timer`, `interpret_effects`](../../../apps/game-client/src/lib.rs) | Timer được fire tại deadline, tie theo `(deadline, TimerId)`; timer bị remove trước apply. Vòng lặp drain **mọi** timer due, chưa có cap 4 tick hoặc yielding/backlog policy |
| Timer lỗi hoặc cũ | Cùng file và [`note_timer_error`](../../../apps/game-client/src/runtime_ui.rs) | Timer reject có thể làm local session dừng vì đã consume. Rules phải định nghĩa unknown/stale/re-arm; không dựa vào cancel để chứng minh không có callback cũ |
| Counter | [I-7](../../architecture/00-architecture-principles.md#7-architecture-invariants), [`runtime::system/command`](../../../crates/tabula-match/src/runtime.rs), [`replay_capture.rs`](../../../apps/game-client/src/replay_capture.rs) | Actor tăng version đúng một lần/input accepted. Local host ghi index từng attempt và hash accepted, không có một `state_version` wire riêng. Tick, input index và version không đồng nghĩa |
| Bot timing | [`GameBot`](../../../crates/tabula-game-api/src/bot.rs), [`run_game` bot loop](../../../apps/game-client/src/main.rs) | Trait cho `think_time` và effect cho deadline; vòng local hiện tại lấy seat rồi bỏ deadline, không gọi `think_time`, còn dựa `turn_of`. Chưa đáp ứng timed PVE hoặc ba difficulty công bằng |
| Pause/background | [`advance_frame`](../../../apps/game-client/src/lib.rs), [ADR-0030](../../adr/0030-local-discovery-gameplay-handoff.md), [`native_host.rs`](../../../apps/game-client/src/native_host.rs) | Local hiện tại để elapsed time qua dialog/blur; native seam mới ghi TODO suspend render/input, chưa thực thi; không tự đổi rules timer. Cần riêng quyết định freeze PVE, không tự kế thừa clock của Chess |
| Chạy online | [`runtime_ports.rs`](../../../crates/tabula-match/src/runtime_ports.rs), [`ClosedEffects`](../../../crates/tabula-match-http/src/native_ports.rs), [ADR-0042](../../adr/0042-isolated-match-reconnect-resync.md) | Chưa có real timer/bot executor trong isolated match; online effect adapter chỉ nhận empty/EndMatch. Reconnect hiện tại không chứng minh realtime scheduling/durability |
| Projection cost | Local apply rebuild project và dispatch mọi view event; actor [`prepare_updates`](../../../crates/tabula-match/src/runtime.rs) so sánh projection | Full View 20 lần/s có thể tốn CPU/bytes. Phải đo riêng projection/hash/render; không gửi State để giảm chi phí, không suy ra online bandwidth từ local heap |
| Physics và FPS | [Doc 04 §9.1](../../architecture/04-frontend-and-design-system.md#91-the-hard-invariant) | Existing animation chỉ trình bày. Fixed step vẫn có thể tunnel nếu chỉ test vị trí cuối; C01 phải giữ front-gap/footprint clamps và từng due tick. R3 dùng homing arrival/ground impact, không physical projectile sweep từ sprite |

Các bài test local có tên `late_frame_replays_the_timer_at_its_deadline_not_frame_arrival`,
`public_input_entry_cannot_overtake_a_due_timer` và
`cancellation_rearm_and_simultaneous_timer_order_are_stable` là điểm đọc/giữ
regression. Sự tồn tại của test không có nghĩa chúng đã chạy trong D01, hoặc chứng
minh game mới có catch-up bounded.

## 4. Hai phương án có giới hạn cho owner

### A. Giữ Timer ABI, gom ở host dispatch — khuyến nghị để mở C01

Mỗi quantum dùng một `Input::Timer` tại deadline 50 ms kế tiếp; reducer giải hết
các due facts theo tick/phase chính xác trong quantum đó. Host pump nhiều nhất bốn timer tick mỗi
lượt, rồi yield. Đây là **coalesced dispatch**, chưa phải một input chứa bốn tick.
Bốn timer accepted nghĩa là I-7 **+4**, không phải +1. Có thể chỉ present snapshot
cuối pump, nhưng không mất ordered semantic events/replay checkpoints.

Ưu điểm: reuse Input/Effect, giữ RNG per-input và truy vết rõ; không thêm wire.
Đổi lại vẫn khoảng 20 system input/s, cần sửa generic local host sau quyết định,
kiểm tra projection/event pressure và boundary khi command đến giữa quantum.
RULES R1 đề xuất validate/reserve tại acceptance, hiệu lực ở
`last_completed_tick + 1`; không cho Cast chưa ready được queue chờ. Host giữ
logged order và không catch-up qua effective tick của command đã accepted.
Đây là chính sách lượng tử hóa được khai báo, không phải exact off-grid input
time. C01 phải kiểm tra boundary này và rejected input tuyệt đối không mutate.

### B. Một input advancement chứa 1–4 tick — hoãn đến PR contract riêng

Một batch canonical duy nhất có cursor đầu/cuối xác định và xử lý đầy đủ mọi
due tick/phase bên trong. Một batch accepted tăng version **+1**, dù traversal bốn
tick. Giảm số dispatch/hash/checkpoint nhưng làm command cutoff, `Ctx.now`, re-arm,
stale generation, event timestamp, RNG domain và replay khó hơn. ABI hiện tại
không hứa có payload batch; không giả lập bằng `Input::Player(Advance)`.

Chỉ mở nếu owner phê duyệt generic local advancement seam với consumer và bound
cụ thể, cùng review/tests cho toàn bộ game bị ảnh hưởng. Khác grouping input cũng
khác RNG root/index; batch invariance phải được thiết kế và kiểm tra, không suy ra
từ same-input determinism. D01 không thêm variant vào SDK/protocol hoặc persistence.

Không chọn A/B thì dừng runtime tại skeleton. Một lựa chọn khác là bỏ yêu cầu liên
tục và thiết kế lại thành lượt, nhưng đó là thay đổi gameplay cần owner duyệt.

## 5. Quyết định finite trước C01

1. **Local exception:** chấp nhận offline PVE trust boundary và giới hạn trên,
   hoặc giữ gate đóng. D01/rules draft chưa là sự chấp nhận ngoại lệ
2. **Timing:** chọn A hoặc B; duyệt next-tick command cutoff của RULES R1 và sequence ở
   timestamp trùng deadline. Không có lựa chọn “batch tự hiểu từ frame time”
3. **Lifecycle:** đề xuất freeze simulation và bot khi Pause/background; Resume
   dùng accumulated paused duration để clock không catch-up thời gian đã pause.
   Back/reload/process death bỏ trận local chưa save. Phải quyết định rõ trước code;
   không tự sửa semantics của các game khác
4. **Catch-up:** không bỏ collision/event hoặc delta để theo kịp FPS. Nếu backlog
   còn, giữ thứ tự và defer command chưa đúng mốc; yield để UI còn phản hồi. Vượt
   bound đã chốt phải báo trạng thái bị chậm/dừng an toàn, không tự kết quả giả
5. **Bot:** clock/pacing của cả bot từ timeline logic, stable policy identity và
   seed; same View/legality/stats/economy như người. Không chạy choose mỗi render
   frame. Diff difficulty chỉ ở policy budget/pacing được QA định nghĩa
6. **Evidence:** numeric test scaffold chỉ chứng minh các assertion đã chạy.
   Trước runtime acceptance cần independent rules oracles, conformance, replay,
   lifecycle, measured perf và HUD/assets được duyệt; [QA-PLAN](QA-PLAN.md) ghi gate

Mỗi quyết định được ghi rõ accepted/rejected/deferred và người quyết định trong
review sau D01. Không mở network, registry rollout hoặc một phase khác bằng một
câu “game chạy được”. Chưa gán deadline thay owner cho các phần đang BLOCKED.

## 6. Mobile: dependency riêng, không fallback

Tại base khảo sát, [ADR-0043](../../adr/0043-native-mobile-gamehost.md) đã chấp nhận
hướng CMP + Rust/Macroquad **native cùng app** cho
[#81](https://github.com/loveoverflowcom/tabula/issues/81). PR #117 đã merge source
skeleton Android: control/lifecycle seam, fail-closed backend và empty packaged
runtime inventory. Chưa có native library/assets hoặc game/frame chạy thật.
[Android ledger](../../verification/mobile-native-host/android-skeleton.md) ghi
build/unit/compile và các blocker riêng; không coi các kết quả lịch sử đó là run
của D01. iOS vẫn chưa có child-controller adapter.

**Không có ADR-0048 trong cây base này.** Một ADR-0048 tương lai cho FFI/export hoặc
unsafe-policy phải được xác minh ở commit mới; không coi số ADR dự kiến là đã
phê duyệt. `#![forbid(unsafe_code)]` còn nguyên. Age War không thực hiện FFI,
engine patch, ABI packaging, native host hay thay bằng WebView.

Web/Desktop playable hoặc CMP Desktop Preview chỉ chứng minh chính target đó.
Mobile cần cùng-app open → native first usable frame → input/game change → Back
→ reopen, surface loss/background/orientation/late callback/stop-join và device
performance riêng Android/iOS. Native packaging unavailable thì không hiện Play;
không làm WebView fallback. Gate #81/ADR-0043 độc lập với D01 và approval HUD.

## 7. Cách sử dụng tài liệu này

D01 có thể được review như design/skeleton, với kết quả build/test hữu hạn được ghi
ở ledger PR. Không ghi status “compatible runtime”, “balanced”, “mobile ready”,
“60 FPS” hoặc “ADR accepted” từ file này. Khi C01 được phép, đối chiếu lại fresh
base, chọn phương án timing đã duyệt và bổ sung evidence ở đúng owner hiện có.
