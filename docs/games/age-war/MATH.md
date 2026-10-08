# Toán chiến đấu và ma trận counter · 0.1.0-d01

**UNBALANCED / phân tích khởi điểm, không phải kết quả mô phỏng.** Luật tính ở
[RULES R4](RULES.md#r4-damage-and-modifier-order); dữ liệu gốc ở
[`catalog.rs`](../../../games/age-war/src/catalog.rs). Các hàm
[`math.rs`](../../../games/age-war/src/math.rs) chỉ kiểm tra toán/spec hữu hạn,
không chạy trận, AI, collision hoặc balance.

## 1. Miền, làm tròn và ý nghĩa

20 tick/s, 1000 q/world unit, 10000 bp =100%. Chia damage ở **từng bước** lấy
floor; thời gian bay/số hit cần thiết lấy ceil. Thứ tự tech → action multiplier
→ armor+tech−shred → pierce → guard → shield → HP là cố định. Số nguyên lớn
được mở rộng trước nhân; không floor HP ratio rồi mới tính refund. Nội suy
sprite không đi vào bất cứ công thức nào.

Khi một guard đang hoạt động có outgoing penalty2000bp, action multiplier cho
đòn đánh của chính nó nhân thêm8000/10000, trước stage armor. Guard defense
chỉ tác động hai direct Physical/Arcane hit được chọn, không DoT/Siege và không
phải giáp thụ động vĩnh viễn. Không cộng trùng guard vào armor.

Các tỷ lệ DPS/EHP dưới đây là mô hình phân tích. Combat thật còn movement,
range/min-range, target lock, overkill, queues, windup bị ngắt, multi-hit,
đội hình, finite grant budgets và phản ứng người chơi. Một số lớn không tự
là “counter thắng” hay “game cân bằng”.

## 2. Công thức và ví dụ độc lập

### Damage với nhiều stage

Fixture tổng hợp, không giả vờ là stats một unit reference:
base80, attack tech1000bp, action12000bp, armor2500bp, không shred/pierce,
guard3000bp. Từng bước:80→88→105→78→54 HP. Không gộp các phân số thành một
phép nhân rồi floor cuối. Không guard ⇒78. Pierce5000bp làm armor còn1250,
105→91→63 sau guard. Positive packet cuối tối thiểu1; miss không tạo packet.
Guard mặc định catalog là1500bp, ví dụ3000 ở đây chỉ kiểm tra miền/schema toán.

### Chu kỳ DPS và TTK

Với damage thực trên một contact D, windup w, recovery r:

- Period P=w+r tick; DPS=D×20/P, giữ dạng phân số
- N=ceil(targetHP/D) contact; TTK=w+travel+(N−1)×P tick
- Flight=ceil(surfaceDistance/speed) tick, basic ranged tối thiểu1

Fixture54damage, w4/r16, gap5000q/speed2000q/tick: travel3, DPS1080/20=54HP/s.
HP300 cần6hit, TTK4+3+5×20=107tick=5,35s. Không thêm travel mỗi chu kỳ vào DPS:
nhiều basic shots có thể đang bay. Trên mục tiêu đứng yên và chưa có hit nào
trước đó, delay đầu có ảnh hưởng TTK; combat đổi target phải tính lại.

Ví dụ catalog không có skill/tech/guard: `stone_keeper`24Physical đánh
`root_shield` armor2000 ⇒19/contact; HP280 cần15hit, w8/r18 ⇒372tick=18,6s
từ lúc windup đầu bắt đầu. Đây là **solo stationary attack oracle**, không
là trận equal-gold:35gold đấu65gold, hai troop35gold có geometry khác một tank.

### Effective HP theo damage kind

EHP xấp xỉ liên tục=ceil(HP×10000/(10000−mitigation_kind)). Unit HP300 và
armor2500 ⇒400 EHP cho loại damage đó, không phải cả ba damage kind.
`iron_lotus` HP420: Physical3000 ⇒600; Arcane1500 ⇒495; Siege2000 ⇒525.
Counter có thể khai thác channel yếu dù armor Physical cao. Actual packet
floor/min1 khiến EHP không thay được số contact; guard finite không được
đưa vào denominator như một bonus thường trực.

### Gold/population efficiency

DPS/gold và DPS/pop là phân số với cùng định nghĩa DPS. Fixture54HP/s,
gold80/pop2 ⇒0,675HP/s/gold và27HP/s/pop. Cần thêm:

- Survival/gold theo từng damage kind, exposure/range và training bottleneck
- Damage hữu hiệu thay vì overkill; cost đồng đội bảo vệ ranged/siege
- Chi phí giữ pop của support, lượng heal/shield **còn** trong lifetime budget
- Thời gian trước age unlock, tiền dư và quân đã trả phí đang sống

Không so unit35gold với unit90gold rồi gán winner thành counter. Equal-gold
budget theo đội hình và equal-pop là hai experiment khác nhau; giữ phần gold
dư trong state và ghi training/arrival, không tự tặng troop cho hết tiền.

### AoE occupancy và channel

Expected effective damage cho một ground cast là tổng damage từng mục tiêu
**đang trong footprint area tại impact**, tối đa cap; không phải damage×toàn
bộ quân đối phương. Với tâm8000q, radius1500q, halfwidth500q: center10000
đúng biên được hit; center10001 không hit. Ba tâm6000/8000/10000 có cùng
halfwidth500 đều eligible, nhưng cardcap2 chỉ hit hai target theo R2 stable
priority. Kích thước body và đội hình quyết định occupancy, không mật độ VFX.

Giả thuyết benchmark: đội hình dồn≥3mục tiêu trong AoE làm bombard hấp dẫn;
đội hình thưa hoặc tiến ra khỏi center khi telegraph đang windup làm DPS
hiệu dụng thấp hơn. Chưa đo occupancy/win-rate.

### Heal/shield throughput có giới hạn

Nominal HPS=healGranted×20/cooldownTicks, chỉ khi còn deficit/eligible budget.
Unit spawnHP200 có lifetime heal70; đã grant65 thì dù card yêu cầu40 cũng
chỉ grant5. Shield pool≤50, lifetime grants≤100; prior90/pool20 thì cast40
chỉ grant10. Expire rồi cast lại vẫn prior100, grant0. Giá trị hỗ trợ là
thời gian/đội hình nó bảo toàn trước khi budget cạn, không HPS vô hạn.

Nếu armor2500, tối đa370 tổng HP+heal+shield được grant trên unitHP200 có
ceiling liên tục493,34 raw incoming damage, bỏ qua guard/contact floor. Không
coi ceiling này là guaranteed survival: target có thể chết trước heal, cast
có windup/range/cost và shield có pool/expiry riêng.

### Hoàn vốn technology và cơ hội tiến hóa

Stat tech attack +1000bp cost `80+40×age`; armor +250bp cost `100+40×age`,
research60tick. Ví dụ age3 attack cost200: `briar_blade`34Physical đối với
armor3000 cho23/contact; +10% ⇒37raw→25, tăng2/contact,40/26HP/s. Chỉ một
troop không chắc có nhiều extra kills; nghiên cứu cho đội quân hiện tại/
tương lai có giá trị khác mua ngay một troop mới.

Payback gold giả định=C_upgrade / expected additional bounty gold per second.
Nếu extra bounty=0 thì **không có payback bằng gold**, dù upgrade có thể giúp
phá base. Với kill-health420/bounty18 và một troop tăng40/26HP/s, xấp xỉ chỉ
0,06594extra gold/s; hoàn200gold mất khoảng3033s, quá hard20min. Đây là
chẩn đoán cơ hội/giả định, không kết luận upgrade phải hoàn vốn bằng kill gold.
C02 phải đo army size, hữu hiệu damage, tỷ lệ sống và thời điểm unlock.

Năm age costs cộng3250gold; XP cao nhất500 ⇒sớm nhất500s về XP nhưng gold
thụ động3250/5=650s nếu không chi/không bounty (opening180 làm thời gian save
thực614s). Tổng research25s có thể nằm trong thời gian save, không cộng cơ
học hai clock nếu chúng chồng nhau. Mua đội quân khiến timing thay đổi; không
đặt owner kỳ vọng ván nào cũng lên age6. Start-at-age6 fixture kiểm nội dung,
không dùng nó để chứng minh campaign progression cân bằng.

## 3. Counter giả thuyết có điều kiện

Ký hiệu leverage, không là measured W/L: P=pressure trước melee contact;
A=armor chịu được channel thông thường; X=pierce/shred/channel yếu khai thác
tanker; C=control/telegraph cản charge; B=burst/AoE phạt cluster;
H=hỗ trợ finite budget; V=cửa sổ sau hết guard/reload/heat; S=siege ép static;
“—”=không có lợi thế cơ chế riêng được tuyên bố. Mọi cell cần protected
composition, equal-gold/equal-pop và side swap trong C02, không solo mặc định.

| Attacker \ defender role | Frontline | Ranged | Tank | Skirmisher | Support | Siege |
|---|---|---|---|---|---|---|
| Frontline | — | V khi áp sát | X nếu có shred | A giữ contact | V khi áp sát | V dưới min-range |
| Ranged | P có màn chắn | B nếu volley | X nếu pierce | P nhưng dễ bị charge | P nếu tiếp cận target | P/B nếu có màn chắn |
| Tank | A giữ line | A đúng channel | — | A hấp thụ charge | V sau budget cạn | Không armor blanket; cần X |
| Skirmisher | charge/V | charge nếu vượt window | X nếu pierce | C đúng timing | pressure sau frontline | V dưới min-range |
| Support | H cho đồng đội | H/C/B tùy family | H/X qua đồng đội | C cắt charge | finite budget race | C/B chặn telegraph |
| Siege | B nếu cluster | B có line bảo vệ | X/S đúng channel | yếu khi bị áp sát | B nếu có AoE exposure | S/telegraph trade |

Khác biệt theo từng age, đọc cùng roster exact IDs:

| Age | Counterplay phải thể hiện ở benchmark | Negative control |
|---|---|---|
| Primitive | `quick_track` charge, `root_shield` guard, `leaf_mender` finite heal/cleanse, `rolling_stone_rig` min-range | Không cho charge vượt thân trước; heal không kéo dài vô hạn |
| Ancient | `salt_guard` shred và `sand_rider` pierce đối guard; `sun_axle_archer` volley bị mitigation từng viên | Shred không cộng3stack; volley không tự bắn mọi target |
| Feudal | `iron_lotus` có PhysicalEHP cao nhưng Arcane thấp; `mist_bow` slow và `red_wayfarer` charge yêu cầu tempo | Không dự đoán tank thắng mọi channel; +tech không heal |
| Arcane | `quartz_warden`/caster Arcane đổi weak channel; root/cleanse và chain punish formation | Root không chain-stun; defensive cleanse không xóa enemy shield |
| Industrial | `wire_bodyguard` burst_heat và `steel_flower_cannon` telegraph tạo window; skirmisher có thể tận dụng reload | Không bỏ heat/reload bằng target/age reset; AoE không hit unit đã ra khỏi điểm |
| Future | `light_cutter` pierce, `relay_keeper` bounded drones, `orbit_keeper` finite shield | Drone không là gold farm; shield budget không refresh; không free turret refit HP |

## 4. Ma trận số khởi điểm: số contact, không phải win rate

Các bảng dưới tính `ceil(defenderHP / max(1,floor(attackerDamage ×
(10000−defenderArmorOfAttackKind)/10000)))` từ basic catalog ở revision1.
Không skill, tech, guard, shield, heal, target/range/movement/flight, control,
burst, giá hay pop. Đây là analytic contact oracle, **không chạy gameplay**.
Không gọi attacker cần ít hit hơn là counter đã cân bằng: period/first hit và
kinh tế còn khác. C01 dùng thêm TTK/queue/collision và full composition tests.

Thứ tự F/R/T/K/H/S: Frontline/Ranged/Tank/Skirmisher/Support/Siege. Trong-age
là6bảng6×6; liền-age là5cặp6×6 theo hướng cũ→mới, cộng hướng mới→cũ để
không bỏ counter khi giữ quân cũ. Every age unit ID mapping ở CONTENT.

### Primitive → Primitive

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 8 | 4 | 15 | 5 | 5 | 8 |
| R | 9 | 5 | 18 | 6 | 5 | 9 |
| T | 8 | 5 | 17 | 6 | 5 | 8 |
| K | 10 | 5 | 20 | 7 | 6 | 10 |
| H | 13 | 9 | 26 | 9 | 10 | 14 |
| S | 3 | 2 | 6 | 2 | 2 | 4 |

### Ancient → Ancient

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 8 | 5 | 17 | 7 | 5 | 9 |
| R | 9 | 5 | 20 | 8 | 6 | 10 |
| T | 9 | 5 | 19 | 7 | 6 | 10 |
| K | 9 | 5 | 19 | 7 | 6 | 10 |
| H | 14 | 9 | 30 | 12 | 12 | 17 |
| S | 3 | 2 | 7 | 3 | 2 | 4 |

### Feudal → Feudal

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 9 | 5 | 19 | 7 | 6 | 10 |
| R | 10 | 6 | 23 | 9 | 7 | 12 |
| T | 10 | 5 | 20 | 8 | 6 | 11 |
| K | 10 | 6 | 23 | 9 | 7 | 12 |
| H | 16 | 11 | 33 | 13 | 14 | 19 |
| S | 3 | 2 | 6 | 3 | 2 | 4 |

### Arcane → Arcane

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 11 | 7 | 24 | 8 | 9 | 14 |
| R | 10 | 7 | 23 | 8 | 9 | 13 |
| T | 10 | 7 | 23 | 8 | 9 | 13 |
| K | 9 | 6 | 19 | 7 | 7 | 11 |
| H | 16 | 10 | 35 | 12 | 13 | 21 |
| S | 4 | 3 | 8 | 3 | 3 | 5 |

### Industrial → Industrial

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 10 | 6 | 24 | 8 | 7 | 12 |
| R | 10 | 6 | 23 | 8 | 7 | 11 |
| T | 10 | 6 | 24 | 8 | 7 | 12 |
| K | 11 | 6 | 25 | 8 | 8 | 12 |
| H | 15 | 9 | 30 | 12 | 12 | 17 |
| S | 3 | 2 | 7 | 3 | 3 | 5 |

### Future → Future

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 10 | 6 | 21 | 7 | 8 | 12 |
| R | 10 | 6 | 24 | 8 | 7 | 11 |
| T | 10 | 6 | 21 | 7 | 8 | 12 |
| K | 11 | 7 | 27 | 9 | 8 | 12 |
| H | 17 | 10 | 37 | 12 | 14 | 20 |
| S | 3 | 2 | 8 | 3 | 3 | 5 |

## 5. Liền-age: giữ quân cũ không bị xóa giá trị

### Primitive → Ancient

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 9 | 5 | 20 | 8 | 6 | 10 |
| R | 11 | 6 | 24 | 9 | 7 | 12 |
| T | 10 | 6 | 22 | 9 | 7 | 12 |
| K | 12 | 7 | 27 | 10 | 8 | 14 |
| H | 17 | 10 | 35 | 13 | 13 | 20 |
| S | 4 | 2 | 8 | 3 | 3 | 5 |

### Ancient → Primitive

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 6 | 4 | 13 | 5 | 4 | 7 |
| R | 8 | 4 | 15 | 5 | 5 | 8 |
| T | 7 | 4 | 14 | 5 | 4 | 7 |
| K | 7 | 4 | 14 | 5 | 4 | 7 |
| H | 11 | 7 | 22 | 8 | 9 | 12 |
| S | 3 | 2 | 5 | 2 | 2 | 3 |

### Ancient → Feudal

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 10 | 6 | 23 | 9 | 7 | 12 |
| R | 12 | 7 | 27 | 10 | 8 | 13 |
| T | 11 | 6 | 24 | 9 | 7 | 12 |
| K | 11 | 6 | 24 | 9 | 7 | 12 |
| H | 19 | 13 | 39 | 15 | 15 | 22 |
| S | 4 | 2 | 9 | 4 | 3 | 5 |

### Feudal → Ancient

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 7 | 4 | 14 | 6 | 5 | 7 |
| R | 8 | 5 | 17 | 7 | 5 | 9 |
| T | 8 | 4 | 16 | 6 | 5 | 8 |
| K | 8 | 5 | 17 | 7 | 5 | 9 |
| H | 12 | 8 | 25 | 10 | 10 | 14 |
| S | 3 | 2 | 5 | 2 | 2 | 3 |

### Feudal → Arcane

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 9 | 5 | 18 | 7 | 6 | 10 |
| R | 11 | 6 | 22 | 8 | 8 | 12 |
| T | 10 | 6 | 21 | 8 | 7 | 11 |
| K | 11 | 6 | 22 | 8 | 8 | 12 |
| H | 20 | 13 | 45 | 16 | 17 | 26 |
| S | 3 | 2 | 7 | 3 | 3 | 4 |

### Arcane → Feudal

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 9 | 6 | 17 | 7 | 7 | 10 |
| R | 8 | 5 | 16 | 7 | 7 | 10 |
| T | 8 | 5 | 16 | 7 | 7 | 10 |
| K | 9 | 5 | 20 | 7 | 6 | 10 |
| H | 13 | 8 | 25 | 10 | 10 | 15 |
| S | 3 | 2 | 6 | 3 | 3 | 4 |

### Arcane → Industrial

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 11 | 7 | 22 | 8 | 9 | 12 |
| R | 10 | 6 | 20 | 8 | 8 | 12 |
| T | 10 | 6 | 20 | 8 | 8 | 12 |
| K | 12 | 7 | 27 | 9 | 8 | 13 |
| H | 16 | 10 | 32 | 12 | 13 | 18 |
| S | 4 | 2 | 7 | 3 | 3 | 4 |

### Industrial → Arcane

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 8 | 5 | 17 | 6 | 6 | 9 |
| R | 8 | 5 | 17 | 6 | 6 | 9 |
| T | 8 | 5 | 17 | 6 | 6 | 9 |
| K | 9 | 5 | 18 | 7 | 6 | 10 |
| H | 15 | 10 | 33 | 12 | 12 | 19 |
| S | 3 | 2 | 6 | 2 | 2 | 3 |

### Industrial → Future

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 11 | 7 | 28 | 9 | 8 | 13 |
| R | 11 | 7 | 27 | 9 | 8 | 12 |
| T | 11 | 7 | 28 | 9 | 8 | 13 |
| K | 12 | 7 | 29 | 10 | 9 | 13 |
| H | 19 | 11 | 39 | 14 | 15 | 22 |
| S | 4 | 2 | 9 | 3 | 3 | 5 |

### Future → Industrial

| → | F | R | T | K | H | S |
|---|---:|---:|---:|---:|---:|---:|
| F | 8 | 5 | 16 | 6 | 7 | 9 |
| R | 9 | 5 | 20 | 7 | 6 | 10 |
| T | 8 | 5 | 16 | 6 | 7 | 9 |
| K | 10 | 6 | 23 | 8 | 7 | 11 |
| H | 14 | 8 | 27 | 11 | 11 | 16 |
| S | 3 | 2 | 7 | 2 | 2 | 4 |

## 6. Cách nghiệm thu counter thật

C02 phải kiểm tra full compositions trong-age và từng cặp liền-age theo hai
budget: equal-gold và equal-pop. Giữ gold dư, queue/training và period/flight
thực; đổi side cùng seed. Ghi contact exposure, damage hữu hiệu/overkill, skill
usage, cost, age time, W/D/L và uncertainty. Với chênh age, ghi thêm chi phí
advance sunk, đội quân cũ còn sống và thời gian vulnerability khi research.

Hiện trạng: analytic tables `documented`; math literal checks chỉ được gọi
`example-tested` khi [verification ledger](../../verification/age-war-d01/README.md)
ghi run thật. Matchup win-rate, campaign1000/10000, statistical CIs đo thực,
fun/playtest và performance **NOT_RUN / runtime harness NOT_IMPLEMENTED**.
