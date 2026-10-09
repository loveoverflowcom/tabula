# Age War — kế hoạch QA, balance và đo hiệu năng

**D01 / kế hoạch, chưa là runtime acceptance.** Mốc thiết kế:
`develop@023ecf296352b2f3285c9dbd9c703fe867c9d6cb`, 2026-10-08.
Đọc [RULES](RULES.md), [MATH](MATH.md) và [COMPATIBILITY](COMPATIBILITY.md).
Luật là oracle; test không được sinh expectation bằng implementation cần kiểm tra.

## 1. Trạng thái và gate

| Phạm vi | Trạng thái ở D01 | Điều kiện để chạy/chấp nhận |
|---|---|---|
| Schema/config/toán nguyên scaffold | Chỉ `compiled`/`example-tested` nếu ledger ghi command và count thực tế | Assertion hữu hạn cho validation/toán; không gọi đây là rules conformance |
| GameRules, projection, GameBot, host integration | `NOT_IMPLEMENTED` | C01 được phép sau design → art → assets → duyệt HUD và quyết định compatibility |
| Runtime oracles, conformance, replay, small-seed CI | `NOT_IMPLEMENTED`; execution dự kiến `NOT_RUN` | C01/C02 tạo target thật, test selection không rỗng và oracle độc lập |
| Campaign 1.000 và 10.000 trận | **PLANNED / NOT_RUN** | Reducer + policy + runner có phiên bản; full run, retained seed/results và review |
| Balance, độ khó, “fun”, performance web/desktop | **Chưa đo / chưa kết luận** | Simulation chỉ là một nguồn; còn actual runtime và human playtest |
| Android/iOS native | `BLOCKED / NOT_IMPLEMENTED` tại base | #81/ADR-0043 native backend/package/device evidence; không WebView fallback |

File này không ghi bất cứ win-rate, thời gian frame hay runtime PASS nào. C01/C02
là các slice tương lai theo roadmap #118/#119. Các test hiện có của SDK hoặc
compile skeleton không thay thế test cho luật mới. Nếu một target chưa tồn tại,
ghi `NOT_IMPLEMENTED`; không chạy một command dự kiến rồi gọi zero-test là PASS.

## 2. Independent fixed oracles trước self-play

C01 cần fixture nhỏ với đầu vào/pre-state literal và expected bytes/số/event order
được review từ RULES. Reference model dùng bảng/sort/sweep đơn giản, không gọi
production damage/targeting/collision/refund helper để tính kết quả mong đợi.
Chỉ chia sẻ hằng định nghĩa được kiểm tra riêng; ghi rõ phần shared trust.

| Nhóm | Case cố định và expected consequence cần giữ |
|---|---|
| Simultaneous tie | Hai base còn 1 HP, mỗi bên có hit 1 sau mitigation trong cùng tick: cả hai về 0, **Draw**, đúng một EndMatch; đổi iteration/source order không thay kết quả. Draw do base precedes timeout cùng mốc |
| Tick-start liveness | Hai unit còn 50 HP có due contact 50 cùng tick: cả hai hit hoàn thành rồi chết. Unit đã chết trước tick không được ra hit; projectile/reward không được xử lý hai lần |
| Damage stages | Fixture riêng attack tech/action multiplier/kind mitigation/pierce/guard/shield/HP; không role/class multiplier ẩn. Literal base101, atk+1000bp, action15000bp, armor2000+def250−shred500, pierce2000bp, guard1000bp ⇒ 111→166→142→127 damage; shield50/HP100 ⇒ HP23. Thêm boundary0/1/equality/overkill và từng rounding. Guard2 charges chỉ giảm direct Physical/Arcane, không DoT/Siege; chọn hai eligible packet d3 lớn nhất, tie cast/subevent ID. Heal/shield hợp lệ được cap trước nhóm simultaneous damage theo RULES |
| Bounty owner | Ordinary enemy unit có purchase cost 101: bounty floor(15%) = 15; cost 100 ⇒ 15. Không bounty **target** turret/drone/base hoặc friendly/despawn/no-source; drone/turret source giết normal enemy paid80 vẫn credit owner12; diagnostic attribution chọn source có greatest post-mitigation contribution, tie cast ID sớm nhất; gold về opposing owner; exactly-once sau simultaneous death |
| Age transition | Mốc XP 60/140/240/360/500, cost 250/400/600/850/1.150 và research 100 tick: dưới ngưỡng/thiếu gold không được mở. HP base 1.370 trước/sau age vẫn 1.370 trên max 3.000; không heal/reset queue/cooldown/unit khi chuyển |
| Special cooldown | Hai special slot/age dùng `ready_at` persistent tương ứng: slot ready900 và queued purchase cost80 đều giữ nguyên khi age đổi; bấm slot mới không bỏ qua deadline cũ. Commander cooldown và global age-research slot là hai trục riêng; command chỉ đúng khi RULES cho phép; UI không là authority |
| Training/refund | Paid80: waiting/head hoàn80/60 theo R9; thêm cost101 boundary: waiting101/head floor(75%)75. Queue 5 entry và population cap 24: boundary đúng/overflow/invalid index; cancel không âm gold/pop, không spawn bản sao |
| Turret refund | Turret cost 101, currentHP 50/maxHP 100: floor(101×50/(2×100)) = 25; fullHP ⇒ 50; HP0 ⇒ 0. Floor một lần cuối, không floor tỷ lệ trung gian; dead/sold turret không refund lần hai |
| Lifetime caps | Unit spawn maxHP200: lifetime heal grant tối đa70; prior heal65 ⇒ grant tiếp≤5. Shield pool tối đa50 và lifetime grant tối đa100, kể cả pool cũ đã bị phá bởi damage hoặc expire. Recast/age không reset budget. Base/turret/drone không được heal hoặc shield |
| Control | Root tối đa12 tick; kết thúc12 ⇒ tại20 resisted, root đầu tiên được phép lại tại52. Root/knockback share immunity40 tick sau expiry/cleanse. Slow mạnh nhất, reduction≤40%; refresh không cộng strength. DoT apply0/interval4/expiry12 chỉ pulse4,8; không pulse12 |
| Multi-target/status | `chain`/`bombard` đúng cap/range và stable priority, mỗi target chỉ nhận một packet trong một cast; `volley` giữ đúng một target ID, ba shot18damage cách4tick được phép hit lại target còn sống, target chết thì remaining shots fizzle. `dot` apply0/interval4/expiry12 chỉ pulse4,8, không pulse12; `dispel` baseline chỉ bỏ ally negative status, không strip positive shield hoặc budgets; `charge`/`knockback` clamp biên/contact; `guard`/`pierce`/`shred` đúng modifier stage; `summon` đúng lifetime/pop, `burst_heat` có downtime không reset theo age |
| Summon | Owner3 drones: two-drone cast rejects exact no-op; owner2 với đủ pop có thể lên4. Hai drone dùng2 pop, life240 tick; caster death không despawn chúng. Chỉ death/lifetime/match-end loại drone, zero bounty và pop release once |
| Movement/collision | Front footprint hai phía không cross: tổng closing displacement≤prior gap; ally order giữ nguyên. Remainder1q parity/tick phải mirror đúng, zero-velocity side không nhận q. Blocked spawn và knockback clamp không overlap. Không lấy sprite/trajectory làm collider |
| Projectile/AoE | Homing arrival=`release+max(1,ceil(surface_gap/speed))`; projectile sau nhắm target đã chết fizzles, không retarget quân kế. Ground AoE surface đúng radius included; radius+1q excluded; không cap+1th target. Old impact giữ modifier snapshot sau age/tech |
| Event deadlines | Author deadline theo integer tick, boundary host49/50/51ms vào đúng ordered pipeline; hai due cùng timestamp; 1 tick ×4 so với pump 4 tick có cùng semantic order/outcome theo phương án A. 19 active ticks chưa grant XP/gold, tick20 grant đúng1/5; Pause không tăng remainder. Không nhân theo FPS; current Cast chưa ready bị reject chứ không queue tới next tick |
| End/anti-stall | Fatigue bắt đầu tick14400 (12min): true base loss mỗi active second theo R8; 720s⇒2,780s⇒3, cap12; hai base3000 không nhận hit khác ⇒ cùng2998 ở onset, zero bounty, pause không tiến mốc. Hard horizon tick24000 (20min), sau combat/base death; so exact HP fractions, equality Draw dù gold/XP/age khác. Deadline−1/deadline/deadline+1 và simultaneous base destruction |
| Total reject | Sai seat/phase, overflow/âm/out-of-range sau decode, thiếu resources, repeated command, ended: rejected input giữ canonical bytes và later legal RNG probe. Không “tiến thời gian một chút” rồi trả Err |
| Timer/admin | Unknown/stale/re-arm/cancel, repeated Pause/Resume, timer khi paused/ended. Theo RULES R1/R8, stale/premature/wrong-phase/ended timer reject với exact no-op. Generic accepted no-op nếu có vẫn I-7 +1, reject +0; callback bị shell fence không vào stream. C01 phải xử lý local reject-timer stopping gap đã ghi ở COMPATIBILITY |

Nếu RULES đổi trước duyệt, cập nhật literal bằng một rule decision được review,
không regenerate oracle từ reducer. C02 cần negative controls hoặc mutation nhỏ
chứng minh test phát hiện: heal-base, reset cooldown khi age, refund floor sai,
side-first damage, double bounty, body crossing khi nhảy4tick, homing-shot retarget sai và bỏ Draw khỏi denominator.

## 3. Contract, projection và replay

- Conformance fixture có legal script **không rỗng**, actual state transition,
  genuine rejected command rồi legal probe, và reachable terminal state. Tách
  fixed oracle khỏi `legal_commands` ↔ `apply` consistency check
- Test mọi Input variant, initial/paused/running/research/ended và mọi real Viewer.
  `View` distinct from `State`; renderer/bot chỉ nhận projection, không seed,
  input index, future bot queue hoặc canonical debug object
- Information model ở [game overview](../age-war.md): enemy gold/XP/queue/research/
  spell deadline đều public, không fog/hidden purchase. Seed/internal heap/future
  AI plan không đi vào View. Kiểm tra noninterference của các internals đó và
  actual capability classification; nếu declare hidden information thì thêm
  SecretModel + projection_security. “Offline” không miễn I-5/I-6
- Với phương án A ở COMPATIBILITY: bốn Timer input accepted ⇒ bốn version bump,
  giữ cả bốn accepted checkpoints và events. Coalesce frame presentation không
  bỏ events khỏi replay. Input index/rejected attempt không là state_version
- Replay record phải có seed/config/roster/rules identity, input index và logical
  time, semantic command/timer/admin, per-accepted-input hash. Ít nhất normal,
  exact-deadline/edge và pause/timeout trace; expectation committed độc lập với
  verifier. Replay từ snapshot giữa training, projectile flight, control expiry
- Same stream/debug/release và native/WASM/aarch64 byte/hash equality là các check
  thực thi riêng. WASM build không phải cross-target-tested. Khác timer grouping
  không tự được bảo đảm vì RNG root/index khác; không tuyên bố batch ABI đã có
- RULES_VERSION/hash phải bao trọn rule parameters có ảnh hưởng authority. Replay
  cũ được exact replay/migrate hợp lệ hoặc explicit Unsupported, không regenerate
  golden để che khác biệt. Xác minh actual CLI hỗ trợ game trước khi dùng; không
  thêm branch game-id vào platform vì runner cũ chưa biết Age War

## 4. Ba difficulty công bằng, cùng một game

Đây là **policy thiết kế**, chưa có bot implementation. Tất cả cùng start gold180,
income5/s, XP/research, HP/damage/cost/cooldown/pop/queue, collision và command
legality. Không trợ giá, spawn ngoài queue, extra sight, đọc State, phản ứng từ
input chưa projected, hoặc thắng bằng deadline host frame.

| Policy | Decision cadence | Candidate cap | Lookahead cap |
|---|---:|---:|---:|
| Easy | 30 tick = 1.500 ms | 3 | 0 tick |
| Normal | 15 tick = 750 ms | 12 | 40 tick = 2.000 ms |
| Hard | 8 tick = 400 ms | 24 | 80 tick = 4.000 ms |

Cả ba có **minimum response 6 tick = 300 ms** sau một thông tin mới trên View.
Cadence dùng logical clock; bounded search dùng số candidate/node/tick cố định,
không wall-time. Mỗi pending decision bind một frozen View revision/observation
time k; command của decision đó không submit trước k+6. View mới không reset due
time của pending decision, chỉ đi vào observation của decision kế tiếp. Giữ tối
đa một pending decision/seat; command stale được authority revalidate khi submit,
không tạo instant retry bỏ qua cadence/response floor. Queue/deadline/cooldown/age
vẫn phải được kiểm tra khi submit.
Đề xuất baseline search không branching: mỗi candidate chạy tối đa một rollout
projection-only theo horizon, nên Normal≤480 và Hard≤1.920 candidate-tick steps
(Easy chỉ≤3 current-state evaluations). Forecast không được đọc canonical State
hoặc future enemy intent. Nếu C02 chọn branching, phải chốt thêm node cap hữu hạn
và version policy trước run; cadence/lookahead riêng không chứng minh cost bound.
Stable tie-break và DetRng policy domain được version/pin; trace ghi View revision,
observation time, earliest response, evaluation count, command và result.

Oracle fairness kiểm tra identical rules parameters và authority path; quan sát
mới được decision quan sát ở tick k không gây command của decision đó sớm hơn k+6. Bỏ render frame/đổi FPS không đổi
scheduled choice. Difficulty chỉ đổi policy/pacing/resource budget trên.
Win-rate Easy < Normal < Hard cần đo trên cùng opponent/seed/side; nếu chưa có CI
đủ hẹp, ghi inconclusive, không suy ra “Hard mạnh hơn” từ candidate count.

## 5. Sáu benchmark policy, không phải sáu difficulty

Mỗi policy deterministic, chỉ đọc View và phát legal semantic commands. Benchmark
v0 dùng cadence 15 tick, min response6, candidate cap12, không search sâu; khác
heuristic/ưu tiên. Freeze policy version trước campaign để tránh thay bot cho vừa
kết luận. Dùng ordinal/tie-break ổn định, không “chọn tùy ý” hoặc xem tương lai.

| Policy | Mục tiêu / cách ra quyết định | Trade-off cần quan sát |
|---|---|---|
| Rush | Ưu tiên cheapest available frontline, lấp training queue trong pop budget; chỉ research khi không có pressure buy đáng giá | Áp lực sớm đổi lấy economy/tech, không luôn thắng trước turret |
| Counter | Chọn lớp có matchup tốt với opposing visible composition; so marginal damage/survival per gold theo bảng public | Phản ứng có độ trễ, dễ chịu phí đổi đội hình; không biết purchase chưa thấy |
| Turtle | Ưu tiên turret/phòng thủ và reserve đủ một emergency recruit; research lúc lane đã giữ | Bỏ pressure/tempo; anti-stall không cho camp vô hạn |
| Siege | Sau frontline tối thiểu, ưu tiên siege đối phó turret/base, hạn chế siege thiếu bảo vệ | Mạnh trước defensive static, yếu trước rush/counter melee |
| Tech-rush | Giữ lượng phòng thủ tối thiểu rồi tiết kiệm gold cho nghiên cứu ngay khi đủ XP/cost | Có cửa sổ tổn thương; XP threshold không được bỏ qua để test tech |
| Mixed | Luân phiên frontline/ranged/support/siege theo marginal value và public composition, research khi an toàn | Đội hình cân bằng trả complexity/cost, không được tối ưu biết trước seed |

Threshold “frontline tối thiểu/reserve/an toàn” phải thành literal policy config
và fixed tests trong C02, công khai trong run artifact. Các mô tả này chưa là
source bot hoặc measured meta. Bot difficulty dùng evaluator/pacing đã nêu ở §4;
benchmark dùng cùng budget để so chiến lược, không lẫn tăng tài nguyên với skill.

## 6. Campaign: seed, paired side swaps và confidence

**Cả hai campaign PLANNED / NOT_RUN, chưa có runner.** Full-match matrix gồm 39
cell: 15 cặp benchmark khác nhau + 6 mirror benchmark + 18 benchmark × difficulty
PVE (6×3). Mỗi sample là **một cặp hai trận**, cùng root seed/config/policy IDs:
A-left/B-right rồi A-right/B-left. Swap cả deterministic policy RNG stream/copy
identity tương ứng; không để seat đổi mang theo advantage ngầm. Đối chiếu điều
kiện chung từ seed cho các difficulty comparisons.

1. **1.000 trận diagnostic = 500 paired samples.** Mỗi cell 12 pairs, thêm một
   pair cho 32 cell đầu theo canonical cell order: 468+32 = 500. Mục tiêu tìm bug,
   thiếu coverage và hiệu ứng lớn; khoảng 24–26 trận/cell quá ít cho khẳng định
   cân bằng. Chạy đến terminal/horizon, không bỏ trận khó, Draw hay crash
2. **10.000 trận qualification = 5.000 paired samples.** Sáu mirror ×400 pairs =
   2.400; 15 off-diagonal benchmark ×100 = 1.500; 18 difficulty cell ×61 =1.098,
   thêm một pair cho hai difficulty cell đầu =1.100. Tổng 5.000 pairs. Dùng ít
   nhất 61 common root seeds/cell cho comparisons difficulty. Tăng sample mục
   tiêu sau review nếu confidence còn rộng; “đủ 10.000” không tự thành balance PASS

Root seed manifest, deterministic derivation và canonical order được commit/review
trước run. Mọi trận ghi rules/config/policy versions và hashes, root/derived seeds,
side, commands, W/D/L, ending reason, duration, age reach times, gold/XP/spend,
composition/pop/queue utilization, damage/heal/shield/kill breakdown và faults.
Failure giữ seed + exact earliest divergence hoặc honest checkpoint window,
minimal reproducer riêng; không overwrite original log/golden.

### Metrics và inference

- W/D/L report cùng count và denominator **mọi trận đã bắt đầu**. Faults là lỗi
  cần xử lý, không chuyển thành loss hay silently discard. Run có fault không
  đủ runtime acceptance; cả missing/incomplete/fault counts phải xuất hiện
- Score cho policy A: win=1, Draw=0,5, loss=0. Report win probability riêng và
  score riêng; không dùng Wilson binomial cho fractional draw score hay giả định
  hai trận đổi phe độc lập. W/D/L probability, score và side difference dùng
  **paired, seed-block bootstrap95%** để giữ dependence
- Bootstrap resample theo root-seed pair, cùng block cho các difficulty được so;
  pin bootstrap seed/count (đề xuất 10.000 resamples). Report point estimate,
  interval, pair count, draw rate và CI method. Với nhiều comparison, khai báo
  primary hypotheses trước run và dùng Holm correction hoặc simultaneous interval
  phù hợp; không cherry-pick một cell có p nhỏ
- Mirror: mong side score0,5. Đề xuất equivalence margin ±5 percentage points;
  chỉ nói “không có material side advantage trong domain” khi whole95% paired CI
  nằm trong [0,45;0,55]. Interval rộng là **inconclusive**, không là symmetry PASS
- Composition/age: report cost-adjusted pick share, damage/gold, time-to-contact,
  survival và response windows theo tuổi/class. Zero-pick phải phân biệt unit
  không có cơ hội hợp lệ với dominated choice. Không đặt luật mọi unit50%win
- Trade-offs: rush vs turtle/siege, siege vs counter, tech-rush early risk vs later
  return, mixed flexibility vs cost. Cần ít nhất các phản ví dụ/match traces cho
  tuyên bố counter; aggregate win-rate không chứng minh chiến thuật có chiều sâu
- Difficulty: score differences Hard−Normal, Normal−Easy cùng opponent/root seeds,
  paired CI. Không chấp nhận mạnh hơn nhờ thay stats. Không bắt mọi matchup cùng
  thứ tự win-rate; giải thích heterogeneity và uncertainty

Campaign không chứng minh human skill curve hoặc fun. Human playtest sau HUD/assets
phải ghi learning time, readability, agency/counterplay, waiting frustration,
special clarity, tuổi ít thú vị, trận lặp và lý do muốn rematch. Không thay đánh
giá này bằng mô phỏng hay một câu “10.000 trận đều kết thúc”.

## 7. Small-seed CI và coverage

C01/C02 dự kiến per-PR dùng root seeds `0, 1, 47, 119`, hai sides, sáu cell regression
(rush–counter, turtle–siege, tech-rush–rush, mixed mirror, Normal–counter, Hard–Easy):
**48 trận**, cộng fixed oracles/conformance. Seeds này là cấu hình dự kiến;
D01 chỉ chạy các test schema/toán thật sự tồn tại. Campaign 1.000/10.000 là job
chủ động riêng sau đủ prerequisites, không hứa nightly/automation chưa thiết lập.

Coverage cần thấy: đủ sáu tuổi, research/queue cancel, hai special slot mỗi tuổi,
shield/heal/control caps, turret purchase/sale, bounty ordinary, không bounty drone,
anti-stall, hard horizon, Draw và both-base tie. Nếu random/small seeds không reach
branch, thêm reachable directed fixture; không mark branch green vì match khác PASS.
Không giảm common portfolio definition-of-done (doc08 §7.1 có 100k self-play) bằng
campaign 10k; việc đáp ứng hoặc sửa gate rộng cần decision/evidence riêng.

## 8. Performance budgets đề xuất, chưa đo

Budgets dưới đây là **proposal**, cần owner duyệt và ghi reference hardware/OS trước
C01 acceptance. Workload tối đa lấy từ cap chính thức của RULES (hai bên pop24,
queue5, hai turret sockets/side, shots128/pending512/drone4-owner8-match draft bounds); không bench riêng một unit rồi
suy ra crowded battle. Time measurement ở external harness/shell, không `Instant`
trong rules (I-1/I-3).

| Metric / phạm vi | Budget đề xuất | Protocol |
|---|---|---|
| Một 50ms apply, không render | p95≤0,5ms; p99≤1,5ms | Native release, cả quiet/crowded/tie/deadline storm; report allocations và event count |
| Pump≤4 tick, authority+hash+projection | p95≤4ms; p99≤8ms | Giữ mọi tick/collision; report từng thành phần và backlog, không dùng chỉ mean |
| Canonical bytes / projected bytes | State≤64KiB, full View≤16KiB | Exact canonical codec at real max occupancy; encoding/size không là RAM peak |
| Presentation frame target | 60Hz target: p95≤16,7ms; p99≤33,3ms | Thực browser/desktop riêng; total frame, dropped frames và renderer CPU/GPU, không chỉ reducer |
| Input-to-visible semantic feedback | p95≤100ms foreground | Timestamp intent/accepted projection/visible frame; bot min response300ms là rule pacing riêng |
| Background/Pause | Không authority advance hoặc bot choice trong freeze mode đã duyệt; inactive GPU stopped | Actual lifecycle observation; clock offset và event order sau Resume không replay paused gap |

Nếu full View16KiB được encode20 lần/s, ceiling thiết kế là320KiB/s trước events;
đây là arithmetic local projection pressure, không measured network bandwidth.
Không open network/DB để đo giả hoặc ghi60 DB commit/s. Count `apply`, hash,
projection và render độc lập để biết bottleneck thuộc owner nào.

Protocol run: pin commit/toolchain/features/target/rules/bot/config/asset hash;
release/debug ghi riêng. Record CPU/architecture/RAM/OS, browser/backend/device,
power/thermal state, viewport/DPI, vsync/cache/cold-warm state. Warmup rồi ít nhất
30 full-match samples/workload và ≥10.000 input samples để có tail hữu ích;
report quantile method/count, p50/p95/p99/max, dropped frames, CPU/RSS/GPU peak,
allocations, timer/event rate và catch-up debt. Giữ raw logs/traces ở ignored output
hoặc CI artifacts; không commit binary/capture/generated receipt vào source.

Kiểm tra stall50/200/1.000/5.000ms, hidden/background rồi Resume: mỗi pump≤4ticks,
không event/collision bị mất, command không vượt due time, UI không deadlock.
Không dùng elapsed-time assertion nhiễu làm per-PR correctness gate; perf job
controlled riêng so regression với baseline và confidence trước khi đổi budget.

Android/iOS chỉ đo sau native backend/package thật theo #81/ADR-0043, trên từng
real device: first usable frame cold/warm, frame pacing, touch latency, CPU/RAM,
background/surface loss/Back/reopen. Web/Desktop/stand-in data không là mobile
performance. ADR-0048 chưa có ở base; không giả định FFI/unsafe approval.

## 9. Acceptance report và stopping condition

C01/C02 report đúng changed claim, owner, independent oracle/domain, exact command,
cwd/toolchain/features/seed/count, artifact, status và residual. `PASS` chỉ cho
nonempty check đã thực thi; compile/source-read/headless/screenshot/device là
các evidence kind riêng. Không gộp overlap test counts thành tổng trận đã chơi.

D01 xong khi docs được reconcile với skeleton và declared scope, focused validation
thật được ghi và review không có blocker thiết kế bỏ sót. Dừng tại approval
art/assets/HUD/local exception. Runtime acceptance chỉ mở sau prerequisites;
measurement/campaign pending hoặc inconclusive vẫn là open gate, không thành PASS
vì hết lần kiểm tra hay tự động đóng #118/#119/#81.
