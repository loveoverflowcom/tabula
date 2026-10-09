# Age War — CONTENT D01, bản gốc UNBALANCED

**Chỉ thiết kế và schema examples. C01 vẫn BLOCKED bởi D06 owner gate.**
Không có simulator, `GameRules`, bot, renderer, wire, migration hay đăng ký catalog.
Số liệu tự tác giả tạo làm điểm bắt đầu, không sao chép roster/statline của trò chơi
đã phát hành; chưa playtest, chưa chứng minh balance và không phải preset sẵn chơi.

Nguồn số liệu duy nhất: [`catalog.rs`](../../../games/age-war/src/catalog.rs).
[`schema.rs`](../../../games/age-war/src/schema.rs) định nghĩa đơn vị và descriptor boundary.
Các bảng ở đây là bản đọc của cùng 36/16/12/12/12 descriptors, không phải bảng thay thế
để runtime nạp JSON/TOML. Khi đổi số liệu phải đổi nguồn Rust và bản đọc cùng PR;
`tests/catalog.rs` kiểm tra cấu trúc/ref/coverage, không tự chứng minh tài liệu luôn sync.
Luật thứ tự contact/damage/cooldown/expiry: [RULES R1–R7](RULES.md).

## 1. Cách đọc và giới hạn chung

- Version của mọi descriptor là **1**: revision thiết kế, không phải RulesVersion hay replay format.
- 1 tick = 50 ms; 20 tick = 1 giây. 1 world unit = 1000 q nguyên.
- P/A/S = Physical/Arcane/Siege. Giáp theo thứ tự P/A/S, đơn vị bp; 10000 bp = 100%.
- Khoảng cách/reach tính từ mặt footprint, không phải center hay sprite. Half-width là nửa footprint.
- Basic period = windup + recovery. Projectile speed = 0 nghĩa là melee contact;
  shot bay dùng ceil(surface distance/speed), tối thiểu 1 tick, không retarget khi target chết.
- Kỹ năng `OnAttackContact` là modifier của **hit cơ bản đã có target**: `range` chỉ là schema bound,
  không kéo reach của unit thành 24000 q (card bound) và không tự tạo attack thứ hai.
- Kỹ năng autocast có target/cycle riêng, phải dừng movement trong windup/recovery;
  tất cả shot/summon/status/event capacity được check/reserve trước khi spend/start.
- Buff/debuff dùng `[apply, expiry)`. Tại expiry, status hết trước contact;
  DoT duration81 là chủ ý để pulse20/40/60/80 đều hợp lệ.
- Art hint nữ/nam/cấu thể chỉ phục vụ D02. Không được dùng để target, damage hay AI.
  19/28 human là feminine hint = 67,9%; 8 unit còn lại là cấu thể. D02 acceptance chưa mở.
- Power budget point chỉ là trọng số review **UNBALANCED**, không phải mana, damage hay currency.
  D01 không có energy mechanism; unit/turret skills Free, commander spells trả gold.

### 1.1 Dictionary các hard cap

| Biến | Giá trị draft | Ý nghĩa |
|---|---|---|
| lane/coordinate | 0…100000 q | RULES; Q authored bound100000; Cast point legal area5000…95000 vẫn cần check riêng |
| gold/pop/queue | 20000 / 24 / 5 | normal unit + drone chia sẻ population; queued unit reserve trước |
| base HP | 3000, giáp2000/1000/0 | không heal/shield; age không đổi HP |
| heal lifetime | 3500 bp spawn maxHP | grant thật; overheal không đếm, không reset khi age/cleanse |
| shield pool/lifetime | 2500 / 5000 bp spawn maxHP | pool tức thời và tổng grant; expiry không hồi ngân sách |
| guard | 2 charges mặc định, schema1…3 | hai direct P/A packet lớn nhất theo R4; không DoT/Siege |
| slow/root/knockback | 4000 bp / 12 ticks / 2000 q | slow không cộng; root không khóa basic attack |
| control immunity | 40 ticks sau end/cleanse | không extend trong lúc controlled; root/knockback chung protection |
| DoT sources/statuses | 3 sources/target;16 statuses/entity | mỗi family/source giữ strongest; overflow có thứ tự ổn định |
| drone | 2/cast,4/owner,8/match,1 pop mỗi drone | TTL240; không heal/shield/summon/XP/bounty |
| projectile | 128/match;TTL120 ticks | reserved cả burst; không tùy FPS drop shot |
| semantic deadlines | 512/match | đủ reservation trước commit; overflow là reject/wait có nghĩa |
| commander slots | 2 persistent slots; cooldown≤960 ticks | deadline không reset khi đổi age/card |
| tech total | attack+3000 / defense+1500 bp | research vượt effective cap bị reject; không paid no-op |
| time | target8–12min;fatigue12min;hard20min | target chưa đo; fatigue là source-less base loss theo RULES |

## 2. 36 unit gốc, sáu vai trò mỗi age

Hai bảng mỗi age ghép bằng ID. Không chỉ scale cùng sáu unit: loadout/counter thay đổi ở ít nhất hai vai trò mỗi lần đổi age. Giá, period, armor, reach và footprint là những trade-off độc lập; age cao không bảo đảm thắng đội hình thấp nếu mất formation.

### 2.1. Hoang sơ (Primitive)

| ID / tên | Vai trò | Art hint D02 | Gold | Pop | Train ticks | HP | Giáp P/A/S bp | Move q/tick | Half-width q |
|---|---|---|---|---|---|---|---|---|---|
| `stone_keeper` — Người giữ đá | tuyến đầu | nam | 35 | 1 | 36 | 150 | 1000/0/500 | 90 | 260 |
| `reed_hunter` — Nữ săn lau | tầm xa | nữ | 45 | 1 | 42 | 90 | 0/500/0 | 100 | 200 |
| `root_shield` — Khiên rễ | chống chịu | nữ | 65 | 2 | 56 | 280 | 2000/500/1000 | 55 | 420 |
| `quick_track` — Dấu chân nhanh | cơ động | nam | 50 | 1 | 40 | 105 | 500/0/0 | 145 | 190 |
| `leaf_mender` — Người vá lá | hỗ trợ | nữ | 55 | 1 | 48 | 100 | 0/1000/0 | 80 | 210 |
| `rolling_stone_rig` — Giàn đá cuộn | công thành | cấu thể | 85 | 3 | 70 | 160 | 500/0/1500 | 45 | 480 |

| ID | Basic damage/kind | Windup + recovery = period ticks | Reach / min q | Shot speed q/tick | Skill IDs@v1 | Counterplay draft |
|---|---|---|---|---|---|---|
| `stone_keeper` | 24 P | 8+18=26 | 850/0 | 0 | `guard` | Mở đường bằng pierce; guard có khoảng trống. |
| `reed_hunter` | 20 P | 10+22=32 | 5800/0 | 800 | `pierce` | Áp sát; HP thấp và không AOE. |
| `root_shield` | 22 P | 12+28=40 | 800/0 | 0 | `guard`, `knockback` | Arcane và shred; đi chậm dễ chịu chênh tầm. |
| `quick_track` | 18 P | 6+16=22 | 750/0 | 0 | `charge`, `dot` | Root cắt charge; giáp physical chống hit thường. |
| `leaf_mender` | 12 A | 8+26=34 | 3500/0 | 600 | `heal`, `dispel` | Diệt support trước; heal chỉ35% lifetime. |
| `rolling_stone_rig` | 55 S | 18+38=56 | 7000/2000 | 550 | `bombard` | Áp sát dưới2000q; tránh điểm AOE đã khóa. |

### 2.2. Cổ đại (Ancient)

| ID / tên | Vai trò | Art hint D02 | Gold | Pop | Train ticks | HP | Giáp P/A/S bp | Move q/tick | Half-width q |
|---|---|---|---|---|---|---|---|---|---|
| `salt_guard` — Vệ binh muối | tuyến đầu | nữ | 50 | 1 | 40 | 180 | 1500/500/500 | 95 | 260 |
| `sun_axle_archer` — Xạ thủ mặt trời | tầm xa | nữ | 65 | 1 | 48 | 110 | 500/500/0 | 95 | 210 |
| `silent_bronze` — Khiên đồng lặng | chống chịu | nam | 90 | 2 | 62 | 350 | 2500/1000/1500 | 52 | 440 |
| `sand_rider` — Kỵ nữ cát | cơ động | nữ | 75 | 2 | 48 | 155 | 1000/0/500 | 160 | 240 |
| `banner_weaver` — Người dệt cờ | hỗ trợ | nam | 80 | 1 | 52 | 130 | 500/1500/500 | 82 | 220 |
| `spiral_ballista` — Nỏ trục xoắn | công thành | cấu thể | 115 | 3 | 76 | 210 | 1000/500/1500 | 42 | 480 |

| ID | Basic damage/kind | Windup + recovery = period ticks | Reach / min q | Shot speed q/tick | Skill IDs@v1 | Counterplay draft |
|---|---|---|---|---|---|---|
| `salt_guard` | 28 P | 8+18=26 | 900/0 | 0 | `shred` | Dispel stack; không để bốn contact liên tiếp. |
| `sun_axle_archer` | 24 P | 10+22=32 | 6500/0 | 900 | `volley` | Giáp physical hấp thụ từng viên volley. |
| `silent_bronze` | 26 P | 12+30=42 | 850/0 | 0 | `guard`, `shield` | Shred rồi tập trung hỏa lực; chờ shield hết100tick. |
| `sand_rider` | 26 P | 7+18=25 | 1000/0 | 0 | `charge`, `pierce` | Root ngăn đường chạy; chống bằng HP thay armor. |
| `banner_weaver` | 14 A | 8+28=36 | 4000/0 | 700 | `heal`, `shield` | Burn hai lifetime budget rồi dồn burst. |
| `spiral_ballista` | 72 P | 20+40=60 | 8000/2400 | 1100 | `pierce`, `knockback` | Tấn công trong dead zone; knockback cóimmunity. |

### 2.3. Phong kiến (Feudal)

| ID / tên | Vai trò | Art hint D02 | Gold | Pop | Train ticks | HP | Giáp P/A/S bp | Move q/tick | Half-width q |
|---|---|---|---|---|---|---|---|---|---|
| `briar_blade` — Kiếm sĩ vườn gai | tuyến đầu | nữ | 70 | 1 | 44 | 220 | 2000/1000/1000 | 100 | 260 |
| `mist_bow` — Cung thủ sương | tầm xa | nam | 85 | 1 | 50 | 135 | 500/1500/500 | 105 | 210 |
| `iron_lotus` — Khiên sen sắt | chống chịu | nữ | 120 | 2 | 66 | 420 | 3000/1500/2000 | 50 | 460 |
| `red_wayfarer` — Nữ lữ hành đỏ | cơ động | nữ | 100 | 2 | 50 | 185 | 1500/500/500 | 165 | 240 |
| `bell_physician` — Y sư chuông | hỗ trợ | nữ | 105 | 1 | 54 | 160 | 1000/2000/1000 | 85 | 220 |
| `flower_crossbow_rig` — Giàn nỏ hoa | công thành | cấu thể | 150 | 3 | 80 | 260 | 1500/1000/2000 | 40 | 500 |

| ID | Basic damage/kind | Windup + recovery = period ticks | Reach / min q | Shot speed q/tick | Skill IDs@v1 | Counterplay draft |
|---|---|---|---|---|---|---|
| `briar_blade` | 34 P | 8+18=26 | 950/0 | 0 | `root` | Root không khóa attack; tank vẫn đánh trong12tick. |
| `mist_bow` | 28 P | 10+22=32 | 7000/0 | 1000 | `slow` | Dispel slow rồi áp sát; chậm yếu khôngcộng. |
| `iron_lotus` | 30 P | 13+31=44 | 900/0 | 0 | `guard`, `shred` | Arcane và chênh tầm; shred cần giữ contact lâu. |
| `red_wayfarer` | 28 P | 7+18=25 | 1100/0 | 0 | `charge`, `volley` | Chặn đường chạy; volley dừng chuyển động. |
| `bell_physician` | 16 A | 8+28=36 | 4200/0 | 750 | `heal`, `dispel` | Đẩy support ra khỏi4500q; cleanse cooldown180. |
| `flower_crossbow_rig` | 90 S | 22+42=64 | 8500/2600 | 1200 | `bombard` | Charge vào minrange trước windup22tick. |

### 2.4. Huyền thuật (Arcane)

| ID / tên | Vai trò | Art hint D02 | Gold | Pop | Train ticks | HP | Giáp P/A/S bp | Move q/tick | Half-width q |
|---|---|---|---|---|---|---|---|---|---|
| `quartz_warden` — Hộ vệ thạch anh | tuyến đầu | nữ | 90 | 1 | 46 | 235 | 1500/2500/1000 | 102 | 270 |
| `lightning_guide` — Người dẫn chớp | tầm xa | nữ | 110 | 1 | 52 | 145 | 1000/2500/500 | 100 | 210 |
| `rune_knight` — Kỵ sĩ ký tự | chống chịu | nam | 145 | 2 | 68 | 450 | 2500/3500/2000 | 48 | 460 |
| `shadow_dancer` — Vũ công bóng | cơ động | nữ | 125 | 2 | 52 | 190 | 1000/2000/500 | 170 | 230 |
| `curse_unbinder` — Người gỡ nguyền | hỗ trợ | nữ | 135 | 1 | 56 | 180 | 1000/3000/1000 | 88 | 220 |
| `cracked_prism` — Lăng kính rạn | công thành | cấu thể | 185 | 3 | 84 | 285 | 1000/3000/1500 | 38 | 500 |

| ID | Basic damage/kind | Windup + recovery = period ticks | Reach / min q | Shot speed q/tick | Skill IDs@v1 | Counterplay draft |
|---|---|---|---|---|---|---|
| `quartz_warden` | 30 A | 8+19=27 | 1050/0 | 0 | `shield`, `shred` | Physical damage; shred chỉ giảm mitigation, shield pool có hạn. |
| `lightning_guide` | 32 A | 10+23=33 | 7200/0 | 1100 | `chain` | Giãn đội hình1600q; armor arcane mỗi jump. |
| `rune_knight` | 32 A | 14+32=46 | 950/0 | 0 | `guard`, `root` | Pierce rồi burst; root không bẫy structures. |
| `shadow_dancer` | 32 P | 6+18=24 | 1100/0 | 0 | `slow`, `pierce` | Dispel slow; arcane burst đánh HP thấp. |
| `curse_unbinder` | 20 A | 8+28=36 | 4500/0 | 800 | `dispel`, `heal` | Rải hai negative family vượt remove1 mỗi180tick. |
| `cracked_prism` | 96 A | 24+44=68 | 8800/2800 | 1300 | `chain`, `bombard` | Giãn quân; đánh trong dead zone2800q. |

### 2.5. Công nghiệp (Industrial)

| ID / tên | Vai trò | Art hint D02 | Gold | Pop | Train ticks | HP | Giáp P/A/S bp | Move q/tick | Half-width q |
|---|---|---|---|---|---|---|---|---|---|
| `wire_bodyguard` — Vệ sĩ dây thép | tuyến đầu | nam | 105 | 1 | 48 | 265 | 2500/1500/1500 | 105 | 280 |
| `wind_axle_shooter` — Xạ thủ trục gió | tầm xa | nữ | 130 | 1 | 54 | 165 | 1500/1000/1000 | 100 | 220 |
| `coal_barrier` — Xe chắn than | chống chịu | cấu thể | 170 | 3 | 72 | 530 | 3500/1500/3000 | 45 | 520 |
| `steam_scout` — Nữ trinh sát hơi | cơ động | nữ | 145 | 2 | 54 | 215 | 2000/1000/1000 | 175 | 250 |
| `coolant_engineer` — Kỹ sư băng | hỗ trợ | nam | 150 | 1 | 58 | 200 | 1500/2000/1500 | 90 | 230 |
| `steel_flower_cannon` — Pháo hoa thép | công thành | cấu thể | 220 | 3 | 88 | 320 | 2000/1000/3000 | 35 | 520 |

| ID | Basic damage/kind | Windup + recovery = period ticks | Reach / min q | Shot speed q/tick | Skill IDs@v1 | Counterplay draft |
|---|---|---|---|---|---|---|
| `wire_bodyguard` | 36 P | 8+20=28 | 1100/0 | 0 | `burst_heat` | Dùng tank hấp thụ burst rồi phản công khi heat cao. |
| `wind_axle_shooter` | 38 P | 11+23=34 | 7800/0 | 1400 | `pierce`, `volley` | Nhiều quân rẻ chia target, áp sát lúc volley. |
| `coal_barrier` | 36 P | 14+34=48 | 1100/0 | 0 | `guard`, `knockback` | Arcane; knockback không reset40tick immunity. |
| `steam_scout` | 34 P | 7+18=25 | 1200/0 | 0 | `shred`, `charge` | Root chặn charge; cleanse shred trước burst. |
| `coolant_engineer` | 22 A | 9+29=38 | 4500/0 | 900 | `shield`, `dispel` | Dồn burst vượt shield25%; shield budget cạn không nạp lại. |
| `steel_flower_cannon` | 112 S | 26+46=72 | 9300/3000 | 1400 | `bombard`, `dot` | Charge dưới3000q; dispel DOT không hoànhit. |

### 2.6. Tương lai (Future)

| ID / tên | Vai trò | Art hint D02 | Gold | Pop | Train ticks | HP | Giáp P/A/S bp | Move q/tick | Half-width q |
|---|---|---|---|---|---|---|---|---|---|
| `orbit_keeper` — Người giữ quỹ đạo | tuyến đầu | nữ | 125 | 1 | 50 | 295 | 2500/2500/2000 | 110 | 280 |
| `light_cutter` — Xạ thủ ánh cắt | tầm xa | nữ | 155 | 1 | 56 | 185 | 2000/2000/1500 | 105 | 220 |
| `star_anchor` — Khung neo sao | chống chịu | cấu thể | 200 | 3 | 76 | 580 | 4000/3000/3500 | 42 | 540 |
| `channel_runner` — Người băng kênh | cơ động | nam | 170 | 2 | 56 | 235 | 2500/1500/1500 | 185 | 250 |
| `relay_keeper` — Người điều hợp | hỗ trợ | nữ | 180 | 2 | 62 | 220 | 2000/3000/2000 | 92 | 230 |
| `prism_chorus` — Đàn lăng kính | công thành | cấu thể | 255 | 4 | 92 | 350 | 2000/2500/3500 | 32 | 540 |

| ID | Basic damage/kind | Windup + recovery = period ticks | Reach / min q | Shot speed q/tick | Skill IDs@v1 | Counterplay draft |
|---|---|---|---|---|---|---|
| `orbit_keeper` | 40 A | 8+20=28 | 1200/0 | 0 | `shield`, `guard` | Tiêu pool shield rồi dùng DoT/Siege vượt guard. |
| `light_cutter` | 42 P | 11+23=34 | 8200/0 | 1700 | `pierce`, `burst_heat` | Ép burst vào quân rẻ rồi áp sát lúc nguội nhiệt. |
| `star_anchor` | 40 A | 15+35=50 | 1200/0 | 0 | `root`, `knockback` | Cửa sổcontrolimmunity; siege từngoài reach. |
| `channel_runner` | 38 P | 7+18=25 | 1300/0 | 0 | `charge`, `slow` | Root/knockback ngắt quãng chạy; cleanse slow. |
| `relay_keeper` | 24 A | 9+29=38 | 4800/0 | 1000 | `summon`, `heal` | AOE vào drone; heal không áp dụng drone. |
| `prism_chorus` | 124 S | 28+48=76 | 9800/3200 | 1800 | `chain`, `bombard` | Giãn đội hình và áp sát dead zone3200q. |

### 2.7. Đổi lineup có ý nghĩa

- Hoang sơ: guard hữu hạn + charge cần đường chạy; DOT/đá rơi vượt phòng thủ theo cách khác pierce.
- Cổ đại: frontline đổi sang shred, ranged đổi volley, support shield; armor stack chỉ còn mạnh khi không bị giảm mitigation.
- Phong kiến: root chặn charge nhưng không khóa attack, slow mở kiting; đổi sang frontline control làm ưu tiên cleanse cao hơn.
- Huyền thuật: chuyển damage arcane và chain; giãn đội hình/giáp arcane thay vai trò giáp physical; support ưu tiên gỡ hostile control.
- Công nghiệp: burst_heat là burst có giới hạn nhiệt, siege dead-zone lớn hơn; bắn nhiều viên không tự xuyên tank.
- Tương lai: support summon tạo đúng hai drone dùng population thật, tank control thay guard; AOE và giữ capacity trở thành counter mới.

Đây là các giả thuyết counter cần fixture/playtest D03. Schema test chỉ so sánh loadout/kind thay đổi, không chứng minh counter nào thắng.

## 3. Full cards: 16 reusable skill families

Một unit/turret binding là đúng `id@design_version`, không tự sinh bản reskin riêng theo age.
Card dưới đây khai báo activation, cost, target/filter/priority, geometry, timing,
lifecycle, safety, cue, power budget, counterplay và oracle. Cue chỉ là key contract D05;
không có VFX/SFX asset, animation hay renderer trong D01.

### 3.1. `guard`

| Trường | Giá trị exact |
|---|---|
| Identity | `guard` @v1; family `Guard` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | chính nguồn; chính nguồn |
| Priority | chính nguồn |
| Geometry | range=0 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=2; contact=instant tại windup completion/contact; recovery=4; cooldown=120; duration=40 ticks |
| Exact effect | `Guard { mitigation_bp: 1_500, outgoing_penalty_bp: 2_000, hit_charges: 2, packet_filter: DirectPhysicalAndArcane }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | hết charge hoặc timeout, điều nào đến trước |
| Dispel / immunity | positive tag; D01 không có offensive dispel để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.guard.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 35 review points, UNBALANCED |
| Counterplay | Dùng DoT/Siege, tiêu hai charge bằng packet đủ lớn hoặc đợi hết40tick trước khi dồn damage. |
| Oracle cần C01 fixture | Trong40tick, guard giảm15% cho hai packet direct physical/arcane lớn nhất mỗi tick; tiêu thụ một charge/packet. Action multiplier8000bp khi active; không guard DoT/Siege. |

### 3.2. `pierce`

| Trường | Giá trị exact |
|---|---|
| Identity | `pierce` @v1; family `Pierce` |
| Activation / trigger | tại contact hit cơ bản; hit cơ bản hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; unit/drone/structure còn sống |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=24000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=0; contact=instant tại windup completion/contact; recovery=0; cooldown=0; duration=0 ticks |
| Exact effect | `Pierce { damage: 0, pierce_bp: 1_800 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.pierce.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 30 review points, UNBALANCED |
| Counterplay | Đổi sang HP hoặc giáp arcane; nhiều quân rẻ khiến xuyên giáp mất lợi thế. |
| Oracle cần C01 fixture | Mitigation1000bp còn820bp (=floor1000×0.82); damage=0 sửa hit cơ bản, không sinh hit thêm. |

### 3.3. `shred`

| Trường | Giá trị exact |
|---|---|
| Identity | `shred` @v1; family `Shred` |
| Activation / trigger | tại contact hit cơ bản; hit cơ bản hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=24000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=0; contact=instant tại windup completion/contact; recovery=0; cooldown=0; duration=80 ticks |
| Exact effect | `Shred { mitigation_loss_bp: 750 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; lấy expiry muộn hơn; không cộng magnitude |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | negative tag; ally cleanse được phép gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.shred.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 35 review points, UNBALANCED |
| Counterplay | Cleanse debuff hoặc diệt nguồn; nhiều nguồn shred không cộng magnitude. |
| Oracle cần C01 fixture | Bốn hit chỉ giữ strongest750bp; refresh lấy expiry muộn hơn, không cộng thành3000bp. |

### 3.4. `charge`

| Trường | Giá trị exact |
|---|---|
| Identity | `charge` @v1; family `Charge` |
| Activation / trigger | autocast khi ready; quãng chạy không bị ngắt |
| Cost | 0 gold (Free) |
| Target / filter | chính nguồn; chính nguồn |
| Priority | chính nguồn |
| Geometry | range=0 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=0; contact=instant tại windup completion/contact; recovery=0; cooldown=160; duration=80 ticks |
| Exact effect | `Charge { bonus_damage: 45, required_run_q: 2_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | tiêu ở hit kế tiếp hoặc timeout, điều nào đến trước |
| Dispel / immunity | positive tag; D01 không có offensive dispel để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.charge.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 40 review points, UNBALANCED |
| Counterplay | Root/knockback cắt quãng chạy; chặn bằng quân rẻ để hấp thụ hit tăng cường. |
| Oracle cần C01 fixture | 1.999 q chưa đủ, 2.000 q đủ; một hit kế tiếp nhận +45 và tiêu thụ buff; hết 80 tick không hit thì mất. |

### 3.5. `volley`

| Trường | Giá trị exact |
|---|---|
| Identity | `volley` @v1; family `Volley` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=7000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=6; contact=projectile: { speed_q_per_tick: q(900), lifetime: ticks(12) }; recovery=12; cooldown=140; duration=0 ticks |
| Exact effect | `Volley { damage: 18, kind: Physical, shots: 3, spacing: 4 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.volley.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 45 review points, UNBALANCED |
| Counterplay | Tấn công lúc nguồn đang windup; giáp physical giảm từng viên, không chỉ tổng burst. |
| Oracle cần C01 fixture | Contact ba viên cách nhau 4 tick; mục tiêu chết thì viên đã khóa mất, không retarget. |

### 3.6. `dot`

| Trường | Giá trị exact |
|---|---|
| Identity | `dot` @v1; family `Dot` |
| Activation / trigger | tại contact hit cơ bản; hit cơ bản hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=24000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=0; contact=instant tại windup completion/contact; recovery=0; cooldown=0; duration=81 ticks |
| Exact effect | `Dot { damage: 8, kind: Arcane, pulses: 4, interval: 20 }` |
| Stack / refresh | một DoT mạnh nhất/source; tối đa 3 hostile sources/target; expiry muộn hơn; giữ next pulse; không thêm hit tức thời |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | negative tag; ally cleanse được phép gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.dot.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 30 review points, UNBALANCED |
| Counterplay | Dispel trước pulse tiếp theo; heal có hạn nên không thể kéo dài vô tận. |
| Oracle cần C01 fixture | Không tick ngay tại contact; bốn pulse tại +20/+40/+60/+80, expiry81tick nên pulse80 hợp lệ; apply lại giữ next pulse, không sinh hit ngay. |

### 3.7. `slow`

| Trường | Giá trị exact |
|---|---|
| Identity | `slow` @v1; family `Slow` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=7000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=120; duration=80 ticks |
| Exact effect | `Slow { reduction_bp: 3_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; lấy expiry muộn hơn; không cộng magnitude |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | negative tag; ally cleanse được phép gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.slow.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 35 review points, UNBALANCED |
| Counterplay | Dispel; slow yếu hơn không cộng thêm và không refresh hiệu lực mạnh. |
| Oracle cần C01 fixture | Move 100 q/tick thành 70; hai slow 30% và 40% chỉ dùng 40%, không thành 70%. |

### 3.8. `root`

| Trường | Giá trị exact |
|---|---|
| Identity | `root` @v1; family `Root` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=7000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=180; duration=12 ticks |
| Exact effect | `Root` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | negative tag; ally cleanse được phép gỡ; 40 ticks sau controlled end/cleanse, không refresh khi đang controlled |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.root.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 45 review points, UNBALANCED |
| Counterplay | Dispel hoặc dùng quân làm mồi; cửa sổ miễn control ngăn khóa vĩnh viễn. |
| Oracle cần C01 fixture | Root 12 tick khóa move, không khóa attack; sau ending/cleanse có40tick miễn root/knockback; khi còn controlled, lần tiếp theo không extend. |

### 3.9. `knockback`

| Trường | Giá trị exact |
|---|---|
| Identity | `knockback` @v1; family `Knockback` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=7000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=180; duration=0 ticks |
| Exact effect | `Knockback { distance_q: 1_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; 40 ticks sau controlled end/cleanse, không refresh khi đang controlled |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.knockback.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 40 review points, UNBALANCED |
| Counterplay | Quân tank làm tuyến đệm; structures miễn đẩy; control immunity chặn chuỗi đẩy. |
| Oracle cần C01 fixture | Đẩy1000q ra xa nguồn, dừng ởbiên/ally footprint; không xuyên quân/damage; khi kết thúc nhận40tick immunity. |

### 3.10. `heal`

| Trường | Giá trị exact |
|---|---|
| Identity | `heal` @v1; family `Heal` |
| Activation / trigger | autocast khi ready; HP thiếu, còn lifetime budget |
| Cost | 0 gold (Free) |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | HP/maxHP thấp nhất (cross-multiply) → gần nhất → id |
| Geometry | range=4500 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=160; duration=0 ticks |
| Exact effect | `Heal { hp: 45 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.heal.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 35 review points, UNBALANCED |
| Counterplay | Dồn burst hoặc áp sát support; lifetime heal cap ngăn stall. |
| Oracle cần C01 fixture | Chỉ regular ally còn sống, thấp HP ratio nhất (tie entity id); overheal không tích trữ; lifetime tổng heal <=35% maxHP. |

### 3.11. `shield`

| Trường | Giá trị exact |
|---|---|
| Identity | `shield` @v1; family `Shield` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | incoming committed threat → HP ratio thấp nhất → gần nhất → id |
| Geometry | range=4500 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=180; duration=100 ticks |
| Exact effect | `Shield { hp: 70 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | positive tag; D01 không có offensive dispel để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.shield.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 40 review points, UNBALANCED |
| Counterplay | Dùng burst vượt shield pool25% maxHP hoặc đợi100tick; lifetime grant không tự nạp lại. |
| Oracle cần C01 fixture | Pool shield tức thời<=25% spawnmaxHP; strongest grant, lifetime tổng grant<=50%; overheal không đếm; hết100tick mất. |

### 3.12. `dispel`

| Trường | Giá trị exact |
|---|---|
| Identity | `dispel` @v1; family `Dispel` |
| Activation / trigger | autocast khi ready; có hostile status được phép gỡ |
| Cost | 0 gold (Free) |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | root → slow → hostile DoT/shred → oldest application → status/source id |
| Geometry | range=4500 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=180; duration=0 ticks |
| Exact effect | `Dispel { remove_up_to: 1 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.dispel.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 35 review points, UNBALANCED |
| Counterplay | Dùng nhiều debuff khác family hoặc ép support dùng cleanse trước. |
| Oracle cần C01 fixture | Bỏ một negative status theo root→slow→DoT/shred, rồi oldest application→status id→source id; không trả HP đã mất. |

### 3.13. `chain`

| Trường | Giá trị exact |
|---|---|
| Identity | `chain` @v1; family `Chain` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=7000 q; AOE radius=0 q; max_targets=3 |
| Timing | windup=4; contact=instant tại windup completion/contact; recovery=8; cooldown=160; duration=0 ticks |
| Exact effect | `Chain { damage: 42, kind: Arcane, jumps: 3, jump_range_q: 1_600, retention_bp: 8_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.chain.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 55 review points, UNBALANCED |
| Counterplay | Giãn đội hình hơn 1.600 q; từng entity chỉ bị một lần mỗi cast. |
| Oracle cần C01 fixture | Ba hit trước giáp là 42/33/26 (floor riêng mỗi jump); chọn nearest chưa-hit rồi entity id. |

### 3.14. `bombard`

| Trường | Giá trị exact |
|---|---|
| Identity | `bombard` @v1; family `Bombard` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | ground khóa tại cast; điểm ground; contact lọc địch sống |
| Priority | distance tâm impact → stable entity ordinal/id |
| Geometry | range=8000 q; AOE radius=1800 q; max_targets=4 |
| Timing | windup=10; contact=ground delay: { delay: ticks(20) }; recovery=16; cooldown=180; duration=0 ticks |
| Exact effect | `Bombard { damage: 70, kind: Siege }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.bombard.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 65 review points, UNBALANCED |
| Counterplay | Di chuyển khỏi điểm khóa trong 20 tick; minimum range của siege mở cửa áp sát. |
| Oracle cần C01 fixture | Khóa ground tại cast; contact sau 20 tick, bán kính 1.800 q, tối đa 4 entity nearest tâm rồi entity id. |

### 3.15. `summon`

| Trường | Giá trị exact |
|---|---|
| Identity | `summon` @v1; family `Summon` |
| Activation / trigger | autocast khi ready; có target hợp lệ |
| Cost | 0 gold (Free) |
| Target / filter | chính nguồn; chính nguồn |
| Priority | chính nguồn |
| Geometry | range=0 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=10; contact=instant tại windup completion/contact; recovery=12; cooldown=300; duration=240 ticks |
| Exact effect | `Summon { count: 2, drone: DRONES[0], lifetime: 240 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.summon.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 65 review points, UNBALANCED |
| Counterplay | AOE diệt drone; drone không nhận heal/shield, không XP/bounty. |
| Oracle cần C01 fixture | Sinh tối đa hai drone trong khoảng trống; cap 4/owner, 8/match; TTL 240; đầy cap reject trước commit. |

### 3.16. `burst_heat`

| Trường | Giá trị exact |
|---|---|
| Identity | `burst_heat` @v1; family `BurstHeat` |
| Activation / trigger | autocast khi ready; còn đủ heat budget toàn burst |
| Cost | 0 gold (Free) |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=7000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=5; contact=projectile: { speed_q_per_tick: q(1_200), lifetime: ticks(10) }; recovery=14; cooldown=30; duration=0 ticks |
| Exact effect | `BurstHeat { damage: 22, kind: Physical, shots: 3, spacing: 3, heat_per_shot: 25, heat_limit: 100, decay_per_tick: 1 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.burst_heat.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 55 review points, UNBALANCED |
| Counterplay | Ép burst vào tank rồi phản công trong lúc nguồn đang xả nhiệt. |
| Oracle cần C01 fixture | Precheck cần heat+75<=100; ghi 25/viên khi phóng; decay1/tick; không đổi target trong burst. |

## 4. Hai commander spells mỗi age, 12 variants

Slot First/Second là slot0/slot1 duy trì xuyên age. Cả hai card của age mới dùng lại
`ready_at` cũ; đổi age không tạo cooldown mới. Gold/cooldown spend tại accepted start;
whiff/interrupt không refund. Unit tech không tăng damage/heal/shield của commander.
Commander `range=100000q` tính từ mặt base của owner, đủ phủ lane hợp lệ.
`Cast(slot,pointQ)` chỉ nhận point5000…95000; point khóa geometry lúc accept.
AoE0 chọn eligible footprint chứa point; AoE>0 chọn footprint giao vùng quanh point,
rồi priority/maxTargets. Chain chọn initial target tại point rồi hop; không chọn đồng minh
bất kỳ quanh base. Commander summon đặt hai drone theo anchor point và collision/formation
constraints; không cho sinh bên kia tuyến địch hay chồng footprint. Autocast unit summon tự
đặt hai drone ở khoảng trống cạnh nguồn theo RULES, không dùng player point.

Các full cards riêng bên dưới giữ cùng `SkillEffect` family cơ chế với phần3, có exact
parameters riêng; không dùng tên spell để mở thêm mechanic không được mô tả.

| Age | Slot | ID / tên | Family | Gold | Cooldown ticks / s | Geometry range/AOE/max | Exact payload |
|---|---|---|---|---|---|---|---|
| Hoang sơ | 0 | `stonefall` — Mưa đá rễ | Bombard | 60 gold | 700/35.00 | 100000/2200/4 | Bombard { damage: 100, kind: Physical } |
| Hoang sơ | 1 | `green_rest` — Nhịp lá xanh | Heal | 50 gold | 600/30.00 | 100000/3000/3 | Heal { hp: 85 } |
| Cổ đại | 0 | `sun_javelin` — Lao mặt trời | Pierce | 75 gold | 760/38.00 | 100000/0/1 | Pierce { damage: 160, pierce_bp: 3_000 } |
| Cổ đại | 1 | `iron_chorus` — Hợp xướng đồng | Shield | 80 gold | 700/35.00 | 100000/3000/3 | Shield { hp: 110 } |
| Phong kiến | 0 | `briar_net` — Lưới gai mềm | Root | 75 gold | 760/38.00 | 100000/1800/2 | Root |
| Phong kiến | 1 | `marching_drum` — Trống bước dài | Charge | 80 gold | 800/40.00 | 100000/3000/3 | Charge { bonus_damage: 100, required_run_q: 0 } |
| Huyền thuật | 0 | `prism_arc` — Cung chớp pha lê | Chain | 90 gold | 840/42.00 | 100000/0/4 | Chain { damage: 90, kind: Arcane, jumps: 4, jump_range_q: 1_400, retention_bp: 8_000 } |
| Huyền thuật | 1 | `clear_moon` — Trăng gỡ nút | Dispel | 65 gold | 640/32.00 | 100000/3000/3 | Dispel { remove_up_to: 2 } |
| Công nghiệp | 0 | `fuse_rain` — Mưa ngòi ngắn | Bombard | 100 gold | 920/46.00 | 100000/2400/5 | Bombard { damage: 170, kind: Siege } |
| Công nghiệp | 1 | `coolant_mist` — Sương làm nguội | Slow | 80 gold | 760/38.00 | 100000/2000/4 | Slow { reduction_bp: 4_000 } |
| Tương lai | 0 | `relay_swarm` — Bầy trạm nhỏ | Summon | 90 gold | 960/48.00 | 100000/0/1 | Summon { count: 2, drone: DRONES[1], lifetime: 240 } |
| Tương lai | 1 | `pulse_wake` — Vệt xung sáng | BurstHeat | 110 gold | 960/48.00 | 100000/0/1 | BurstHeat { damage: 95, kind: Arcane, shots: 3, spacing: 4, heat_per_shot: 30, heat_limit: 100, decay_per_tick: 1 } |

### 4.1. Mưa đá rễ (Hoang sơ, slot0)

| Trường | Giá trị exact |
|---|---|
| Identity | `stonefall` @v1; family `Bombard` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 60 gold |
| Target / filter | ground khóa tại cast; điểm ground; contact lọc địch sống |
| Priority | distance tâm impact → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=2200 q; max_targets=4 |
| Timing | windup=8; contact=ground delay: { delay: ticks(20) }; recovery=10; cooldown=700; duration=0 ticks |
| Exact effect | `Bombard { damage: 100, kind: Physical }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.stonefall.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 90 review points, UNBALANCED |
| Counterplay | Di chuyển khỏi điểm khóa trong 20 tick; minimum range của siege mở cửa áp sát. |
| Oracle cần C01 fixture | Khóa ground tại commit; sau20tick hit100 physical, bán kính2200q, tối đa4 entity. Không retarget sau khi khóa. |

### 4.2. Nhịp lá xanh (Hoang sơ, slot1)

| Trường | Giá trị exact |
|---|---|
| Identity | `green_rest` @v1; family `Heal` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 50 gold |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | HP/maxHP thấp nhất (cross-multiply) → gần nhất → id |
| Geometry | range=100000 q; AOE radius=3000 q; max_targets=3 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=600; duration=0 ticks |
| Exact effect | `Heal { hp: 85 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.green_rest.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 80 review points, UNBALANCED |
| Counterplay | Dồn burst hoặc áp sát support; lifetime heal cap ngăn stall. |
| Oracle cần C01 fixture | Hồi85HP cho tối đa3 regular ally theo HP ratio/id; lifetime35% và overheal vẫn áp dụng, không heal structures/drone. |

### 4.3. Lao mặt trời (Cổ đại, slot0)

| Trường | Giá trị exact |
|---|---|
| Identity | `sun_javelin` @v1; family `Pierce` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 75 gold |
| Target / filter | địch; unit/drone/structure còn sống |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=12; contact=projectile: { speed_q_per_tick: q(2_000), lifetime: ticks(60) }; recovery=12; cooldown=760; duration=0 ticks |
| Exact effect | `Pierce { damage: 160, pierce_bp: 3_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.sun_javelin.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 105 review points, UNBALANCED |
| Counterplay | Đổi sang HP hoặc giáp arcane; nhiều quân rẻ khiến xuyên giáp mất lợi thế. |
| Oracle cần C01 fixture | Một projectile hit160 physical, giảm30% mitigation, floor một lần cuối; TTL60tick, không retarget. |

### 4.4. Hợp xướng đồng (Cổ đại, slot1)

| Trường | Giá trị exact |
|---|---|
| Identity | `iron_chorus` @v1; family `Shield` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 80 gold |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | incoming committed threat → HP ratio thấp nhất → gần nhất → id |
| Geometry | range=100000 q; AOE radius=3000 q; max_targets=3 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=700; duration=100 ticks |
| Exact effect | `Shield { hp: 110 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | positive tag; D01 không có offensive dispel để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.iron_chorus.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 100 review points, UNBALANCED |
| Counterplay | Dùng burst vượt shield pool25% maxHP hoặc đợi100tick; lifetime grant không tự nạp lại. |
| Oracle cần C01 fixture | Grant110 shield cho tối đa3 regular ally; duration100tick; tổng grant lifetime50% maxHP vẫn bị chặn. |

### 4.5. Lưới gai mềm (Phong kiến, slot0)

| Trường | Giá trị exact |
|---|---|
| Identity | `briar_net` @v1; family `Root` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 75 gold |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=1800 q; max_targets=2 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=760; duration=12 ticks |
| Exact effect | `Root` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | negative tag; ally cleanse được phép gỡ; 40 ticks sau controlled end/cleanse, không refresh khi đang controlled |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.briar_net.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 100 review points, UNBALANCED |
| Counterplay | Dispel hoặc dùng quân làm mồi; cửa sổ miễn control ngăn khóa vĩnh viễn. |
| Oracle cần C01 fixture | Tối đa2 regular enemy trong AOE1800q bị root12tick; không khóa attack; từng target nhận miễn control40tick sau root end/cleanse (root end12 thì ready52). |

### 4.6. Trống bước dài (Phong kiến, slot1)

| Trường | Giá trị exact |
|---|---|
| Identity | `marching_drum` @v1; family `Charge` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 80 gold |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=3000 q; max_targets=3 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=800; duration=120 ticks |
| Exact effect | `Charge { bonus_damage: 100, required_run_q: 0 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | tiêu ở hit kế tiếp hoặc timeout, điều nào đến trước |
| Dispel / immunity | positive tag; D01 không có offensive dispel để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.marching_drum.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 110 review points, UNBALANCED |
| Counterplay | Dùng quân rẻ hấp thụ boosted hit hoặc kite đến120tick; buff không tăng move/armor và không hồi cooldown. |
| Oracle cần C01 fixture | Tối đa3 regular ally nhận +100 ở hit kế tiếp, không cần run; tiêu thụ mỗi buff một lần hoặc hết120tick. |

### 4.7. Cung chớp pha lê (Huyền thuật, slot0)

| Trường | Giá trị exact |
|---|---|
| Identity | `prism_arc` @v1; family `Chain` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 90 gold |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=0 q; max_targets=4 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=840; duration=0 ticks |
| Exact effect | `Chain { damage: 90, kind: Arcane, jumps: 4, jump_range_q: 1_400, retention_bp: 8_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.prism_arc.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 120 review points, UNBALANCED |
| Counterplay | Giãn đội hình hơn 1.600 q; từng entity chỉ bị một lần mỗi cast. |
| Oracle cần C01 fixture | Bốn hit arcane trước giáp90/72/57/45; mỗi hop <=1400q; entity đã-hit không được chọn lại. |

### 4.8. Trăng gỡ nút (Huyền thuật, slot1)

| Trường | Giá trị exact |
|---|---|
| Identity | `clear_moon` @v1; family `Dispel` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 65 gold |
| Target / filter | đồng minh; normal unit còn sống; loại drone/base/turret |
| Priority | root → slow → hostile DoT/shred → oldest application → status/source id |
| Geometry | range=100000 q; AOE radius=3000 q; max_targets=3 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=640; duration=0 ticks |
| Exact effect | `Dispel { remove_up_to: 2 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.clear_moon.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 90 review points, UNBALANCED |
| Counterplay | Dùng nhiều debuff khác family hoặc ép support dùng cleanse trước. |
| Oracle cần C01 fixture | Tối đa3 regular ally, mỗi target bỏ tối đa2 negative status theo root→slow→DoT/shred, oldest application/status/source id; không trả HP đã mất. |

### 4.9. Mưa ngòi ngắn (Công nghiệp, slot0)

| Trường | Giá trị exact |
|---|---|
| Identity | `fuse_rain` @v1; family `Bombard` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 100 gold |
| Target / filter | ground khóa tại cast; điểm ground; contact lọc địch sống |
| Priority | distance tâm impact → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=2400 q; max_targets=5 |
| Timing | windup=14; contact=ground delay: { delay: ticks(30) }; recovery=10; cooldown=920; duration=0 ticks |
| Exact effect | `Bombard { damage: 170, kind: Siege }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.fuse_rain.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 140 review points, UNBALANCED |
| Counterplay | Di chuyển khỏi điểm khóa trong 20 tick; minimum range của siege mở cửa áp sát. |
| Oracle cần C01 fixture | Khóa ground; delay30tick; siege170 trong2400q, tối đa5 entity; hit cuối trước expiry; không damage ngoài vùng. |

### 4.10. Sương làm nguội (Công nghiệp, slot1)

| Trường | Giá trị exact |
|---|---|
| Identity | `coolant_mist` @v1; family `Slow` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 80 gold |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=2000 q; max_targets=4 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=760; duration=80 ticks |
| Exact effect | `Slow { reduction_bp: 4_000 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; lấy expiry muộn hơn; không cộng magnitude |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | negative tag; ally cleanse được phép gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.coolant_mist.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 105 review points, UNBALANCED |
| Counterplay | Dispel; slow yếu hơn không cộng thêm và không refresh hiệu lực mạnh. |
| Oracle cần C01 fixture | Tối đa4 regular enemy trong2000q; move giảm40% trong80tick; slow không cộng và không kéo dài slow mạnh bằng slow yếu. |

### 4.11. Bầy trạm nhỏ (Tương lai, slot0)

| Trường | Giá trị exact |
|---|---|
| Identity | `relay_swarm` @v1; family `Summon` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 90 gold |
| Target / filter | ground khóa tại cast; điểm ground; contact lọc địch sống |
| Priority | distance tâm impact → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=8; contact=instant tại windup completion/contact; recovery=10; cooldown=960; duration=240 ticks |
| Exact effect | `Summon { count: 2, drone: DRONES[1], lifetime: 240 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | expiry phase trước contact tại đúng expiry tick |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.relay_swarm.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 125 review points, UNBALANCED |
| Counterplay | AOE diệt drone; drone không nhận heal/shield, không XP/bounty. |
| Oracle cần C01 fixture | GroundPoint khóa tại accept; hai drone HP60/damage12/period28/TTL240 phải vừa owned-side interval, không qua blocker; thiếu space/pop2/cap4owner8match reject trước gold/cooldown; source chết không despawn. |

### 4.12. Vệt xung sáng (Tương lai, slot1)

| Trường | Giá trị exact |
|---|---|
| Identity | `pulse_wake` @v1; family `BurstHeat` |
| Activation / trigger | lệnh người chơi; có target hợp lệ |
| Cost | 110 gold |
| Target / filter | địch; normal unit còn sống; loại drone/base/turret |
| Priority | gần nhất → stable entity ordinal/id |
| Geometry | range=100000 q; AOE radius=0 q; max_targets=1 |
| Timing | windup=8; contact=projectile: { speed_q_per_tick: q(2_400), lifetime: ticks(48) }; recovery=10; cooldown=960; duration=0 ticks |
| Exact effect | `BurstHeat { damage: 95, kind: Arcane, shots: 3, spacing: 4, heat_per_shot: 30, heat_limit: 100, decay_per_tick: 1 }` |
| Stack / refresh | chỉ magnitude mạnh nhất; không cộng; giữ expiry; không reset budget |
| Expiry | chỉ contact; không để status thường trú |
| Dispel / immunity | không có status để gỡ; none |
| Interrupt | knockback/death hủy windup; root chỉ ngắt charge; không refund |
| Cue keys | `age_war.pulse_wake.activate`, `.contact`, `.expire`, `.audio` (mỗi suffix dùng cùng prefix) |
| Power budget | 145 review points, UNBALANCED |
| Counterplay | Ép burst vào tank rồi phản công trong lúc nguồn đang xả nhiệt. |
| Oracle cần C01 fixture | Ba projectile95 arcane cách4tick, heat30/shot; cần heat+90<=100 trước commit; decay1/tick và TTL48tick; khóa target. |

## 5. Turret: hai branch mỗi age, tối đa 12 models

Mỗi base có **hai socket thật**, không phải hai turret/age cộng dồn thành12 trên field.
Direct/Control là lựa chọn trong một socket. Turret đứng yên, không dùng population,
không được heal/shield, không tự đổi model/HP/reload khi advance age. Construction80tick;
refit/cancel/refund theo [RULES R7](RULES.md#r7-research-age-transition-and-structures).
Refund bán model dùng 50% paid gold × currentHP/maxHP, floor một lần cuối; pending shot giữ snapshot cũ.
Bounty khi turret bị phá =0. Giáp mọi model trong draft là P/A/S=2000/1500/1000bp.
Skill binding vẫn là đúng full card phần3, không được tự cấp extra range/targets.

| Age | Branch | ID / tên | Gold | Build ticks | HP | Basic damage/kind | w+r ticks | Reach/min q | Speed q/tick | Skill@v1 |
|---|---|---|---|---|---|---|---|---|---|---|
| Hoang sơ | Direct | `reed_slinger` — Cột lau ném | 100 | 80 | 350 | 24 P | 10+30 | 6500/0 | 850 | pierce |
| Hoang sơ | Control | `spore_post` — Cột bào tử | 110 | 80 | 320 | 14 A | 10+34 | 6200/0 | 750 | dot |
| Cổ đại | Direct | `bronze_spiral` — Trục đồng xoắn | 140 | 80 | 410 | 34 P | 12+32 | 7000/1000 | 1000 | pierce |
| Cổ đại | Control | `sand_net_post` — Cột lưới cát | 150 | 80 | 370 | 18 P | 12+36 | 6500/0 | 900 | root |
| Phong kiến | Direct | `mist_crossbow` — Nỏ sương | 180 | 80 | 470 | 36 P | 12+30 | 7400/0 | 1200 | volley |
| Phong kiến | Control | `lotus_ram_post` — Cột chày sen | 190 | 80 | 430 | 24 P | 14+36 | 6500/0 | 1000 | knockback |
| Huyền thuật | Direct | `split_prism_post` — Cột lăng kính tách | 220 | 80 | 500 | 40 A | 14+32 | 7800/0 | 1300 | chain |
| Huyền thuật | Control | `rune_frost_post` — Cột sương ký tự | 230 | 80 | 460 | 22 A | 12+34 | 7100/0 | 1200 | slow |
| Công nghiệp | Direct | `fuse_cannon_post` — Cột pháo ngòi | 260 | 80 | 570 | 60 S | 20+42 | 8200/2200 | 1500 | bombard |
| Công nghiệp | Control | `cooling_scatter_post` — Cột tán nguội | 270 | 80 | 520 | 28 P | 12+32 | 7600/0 | 1600 | burst_heat |
| Tương lai | Direct | `orbit_cutter_post` — Cột cắt quỹ đạo | 300 | 80 | 640 | 52 P | 14+32 | 8800/0 | 1900 | pierce |
| Tương lai | Control | `gravity_anchor_post` — Cột neo trọng lực | 310 | 80 | 580 | 28 A | 14+36 | 8000/0 | 1800 | knockback |

## 6. Tech dictionary và advancement

Mỗi ID mua một lần. Attack tech chỉ tăng **normal-unit basic attack mới commit**;
không tăng spell, DoT đã apply, drone, turret, heal, shield hay base. Armor tech chỉ
áp dụng normal-unit mitigation, không tăng max/currentHP. Current/previous age offers;
flag cũ và aggregate cap còn nguyên sau age. Một research slot dùng chung stat tech/age advance,
concurrent với recruitment/combat. Không refund research, không mua tech đã qua cap.

| ID@v1 | Age | Kind | Gold | Research ticks | Bonus bp | Aggregate cap |
|---|---|---|---|---|---|---|
| `primitive_attack` | Hoang sơ | Attack | 120 | 60 | 1000 | 3000 |
| `primitive_armor` | Hoang sơ | Armor | 140 | 60 | 250 | 1500 |
| `ancient_attack` | Cổ đại | Attack | 160 | 60 | 1000 | 3000 |
| `ancient_armor` | Cổ đại | Armor | 180 | 60 | 250 | 1500 |
| `feudal_attack` | Phong kiến | Attack | 200 | 60 | 1000 | 3000 |
| `feudal_armor` | Phong kiến | Armor | 220 | 60 | 250 | 1500 |
| `arcane_attack` | Huyền thuật | Attack | 240 | 60 | 1000 | 3000 |
| `arcane_armor` | Huyền thuật | Armor | 260 | 60 | 250 | 1500 |
| `industrial_attack` | Công nghiệp | Attack | 280 | 60 | 1000 | 3000 |
| `industrial_armor` | Công nghiệp | Armor | 300 | 60 | 250 | 1500 |
| `future_attack` | Tương lai | Attack | 320 | 60 | 1000 | 3000 |
| `future_armor` | Tương lai | Armor | 340 | 60 | 250 | 1500 |

| Đích | Gold | Total XP ≥ (không spend) | Duration ticks |
|---|---|---|---|
| Cổ đại | 250 | 60 | 100 |
| Phong kiến | 400 | 140 | 100 |
| Huyền thuật | 600 | 240 | 100 |
| Công nghiệp | 850 | 360 | 100 |
| Tương lai | 1150 | 500 | 100 |

XP là1/giây logical active, không có kill XP. Income5gold/giây và start180 là baseline;
advance unlock offer, không reset base/unit/cooldown/queue/status/heat/lifetime budget.
Không ép trận lênage6; các fixture per-age của D03 sẽ dùng toàn bộ catalog để kiểm tra từng tuổi độc lập.

## 7. Drone data và commitments còn thiếu

Drone do `summon` tạo, không nằm trong36 normal recruit. Full-card payload tham chiếu
exact `DRONES[0]` hoặc `DRONES[1]` từ cùng nguồn Rust, không lấy default ngầm từ support:

| Model ID@v1 | Nguồn | HP/pop | Giáp P/A/S bp | Move / half-width q | Attack | w+r=period ticks | Reach/min/speed q |
|---|---|---|---|---|---|---|---|
| `reed_drone` | unit `summon` / `DRONES[0]` | 50/1 | 0/0/0 | 130/160 | 10Physical | 6+24=30 | 750/0/0 |
| `relay_drone` | commander `relay_swarm` / `DRONES[1]` | 60/1 | 0/0/0 | 130/160 | 12Physical | 6+22=28 | 750/0/0 |

Cả hai đúng2/cast, lifetime240tick; không heal/shield/summon/tech,
XP/bounty0. Nguồn chết không despawn drone đã commit. Global population24 vẫn áp dụng;
cap4/owner8/match không thay population reservation. D01 chưa thực thi drone movement/attack.

## 8. Bằng chứng D01 và residual

- Tests gọi actual Rust descriptors: 36-unit/six-age/six-role completeness;
  sixteen family coverage; exact id/version bindings;12spells/12turrets/12techs;
  full-card local validation; art-hint inventory; finite timing/lifetime caps.
- Hostile tests: ID rỗng/Unicode/oversize, ratio bounds toàn u16 domain,
  duration/targets/stack/control/projectile/cost/cardinality sai và raw no-mutation.
- `request_simulation`, `request_ai_plan`, `request_snapshot_migration` đều trả
  typed PhaseGateError; không có success/panic stub. Version/payload nào cũng unsupported.
- Math examples chỉ là pure integer oracles theo [RULES](RULES.md), không phải simulator.
- Không có gameplay test, SDK conformance, cross-target replay equality, balance,
  performance measurement, native/browser visual evidence hay production availability.
- C01 phải có thực thi/tick reservations, reachable state fixtures, transactional rejection,
  projection, timers, replay/versioning và các QA cases đã định nghĩa trong [QA-PLAN](QA-PLAN.md).
  D01 build xanh không mở D06 gate.
