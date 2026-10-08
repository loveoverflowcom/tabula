# Age War — nghiên cứu tham chiếu D01

Version: `d01-research-0.1` · Ngày truy cập: **08/10/2026 (UTC)**

Phạm vi: [D01 #119](https://github.com/loveoverflowcom/tabula/issues/119), thuộc [roadmap #118](https://github.com/loveoverflowcom/tabula/issues/118).

Trạng thái: **DRAFT, chờ owner review**. Đây là nghiên cứu để thiết kế game gốc của Tabula; không phải bản mô tả đầy đủ luật game tham chiếu hay kết quả cân bằng.

## 1. Kết luận có thể dùng ngay

- **Tách mobile và Flash.** Mô tả mobile ghi 7 thời đại, 29 loại quân; trang Flash 2010 chỉ được dùng làm bằng chứng cho vòng lặp bảo vệ căn cứ, mua quân/tháp và tiến hóa. Không chuyển số liệu giữa hai bản. [S01][S02][S03]
- **Phối hợp quan trọng hơn DPS riêng lẻ.** Tài liệu Blizzard minh họa vai trò hỗ trợ và đường khắc chế. [S04][S05][S06]
- **Một lane thay đổi giá trị cơ chế.** Không có vòng ra sau, chạy vòng, trinh sát bản đồ hay rút quân tự do. Mọi đề xuất chuyển thể bên dưới là suy luận thiết kế của Tabula, chưa được gameplay chứng minh.
- **6 × 6 và 16 họ skill là baseline Tabula**, không phải roster hoặc luật Age of War 2. Ưu tiên 36 đơn vị có khác biệt về nhịp, đội hình và counterplay; cùng taxonomy để giảm thuật toán và tăng khả năng kiểm thử.
- **Deterministic không chứng minh hỗ trợ realtime.** Doc 00 §1.2 loại action-game netcode khỏi mục tiêu hiện tại. D01 cần quyết định tương thích hẹp trước khi triển khai continuous single-lane PVE; không ép thành lượt và không tự mở online PvP.

## 2. Cách đọc bằng chứng

| Nhãn | Ý nghĩa | Không được suy ra |
|---|---|---|
| `source-read` | Đã đọc trực tiếp trang nguồn, đúng mục và phiên bản ghi bên dưới | Đã chạy executable, đo collision hoặc xác nhận mọi build |
| `documented` | Nguồn tác giả/nhà phát hành mô tả tính năng | Tính năng hoạt động đúng, AI công bằng hoặc tuning đã đo |
| `inferred` | Lý do chuyển thể rút ra khi đặt cơ chế vào một lane | Đây là luật gốc hay quyết định đã duyệt |
| `proposed` | Thiết kế gốc để Tabula review và kiểm thử | Đã approved, implemented hoặc balanced |
| `chưa xác minh` | Nguồn đã đọc không đủ trả lời | Được phép lấp chỗ trống bằng số nhớ lại hoặc review người chơi |

Trong lượt này không cài/chạy bản mobile hoặc Flash, không reverse-engineer, không đo frame/timing và không tạo hay nhập asset. Reviews trên store/Newgrounds không được dùng để xác nhận luật, thuật toán AI hay lỗi sản phẩm.

## 3. Research matrix

| Nguồn / phiên bản / bằng chứng | Cơ chế hoặc thông tin xác nhận được | Nên học | Không phù hợp hoặc chưa biết khi chuyển sang một lane |
|---|---|---|---|
| **S01 — Google Play, mobile Android**, package `com.maxgames.aow2`. Trang ghi cập nhật 28/07/2026. `source-read`, mô tả nhà phát hành | 7 age, 29 unit types, turrets, phép toàn cục, 4 độ khó, 10 generals có chiến thuật riêng; có nhãn single-player | Quy mô roster; phân biệt lựa chọn quân, phòng thủ, hỗ trợ và đối thủ PVE | Không công bố đầy đủ stats, economy, targeting, collision, cooldown hay giới hạn quân. Mô tả marketing không chứng minh AI dùng cùng luật/tài nguyên |
| **S02 — App Store, mobile iOS**, app `1194118663`; trang ghi version 1.6.6, 24/02/2021. `source-read`, mô tả nhà phát hành | Mô tả cùng quy mô 7 age/29 unit types và nhóm tính năng mobile; lịch sử trang ghi bản đầu 11/04/2017 | Đối chiếu nguồn chính thức thứ hai cho định hướng mobile | Không suy ra Android/iOS có cùng executable, tuning hoặc sửa lỗi. Game Center/achievements không tự chứng minh PvP |
| **S03 — Newgrounds, Flash**, submission `537808`; ghi upload 31/05/2010, credit thiết kế Louissi. `source-read`, Author Comments | Phá căn cứ địch, giữ căn cứ mình; mua quân/tháp; tiến hóa mở quân và phương tiện phòng thủ | Vòng lặp công–thủ–tech dễ hiểu, ưu tiên quyết định có chi phí cơ hội | Không đủ xác nhận roster, số age, phép, economy hoặc AI của bản mobile. Chưa chạy Flash; không khẳng định chi tiết lane/collision từ phần mô tả |
| **S04 — Blizzard Classic guide, Dryad**. Tài liệu Warcraft III lưu trữ; chưa pin patch executable. `source-read`, tài liệu chính thức | Poison gây sát thương theo thời gian và slow; dispel bỏ buff địch/debuff bạn; có spell immunity. Hướng dẫn nhấn mạnh bảo vệ, hit-and-run và phối hợp | DoT/slow tạo giá trị hỗ trợ; dispel cho counterplay; caster cần điểm yếu | Kiting, thoát truy đuổi, bao vây và micro từng mục tiêu dựa trên không gian RTS. Không sao chép tên, lore, miễn nhiễm toàn diện hoặc thông số |
| **S05 — Blizzard Classic guide, Sorceress**. Chưa pin patch executable. `source-read`, tài liệu chính thức | Slow có autocast; Invisibility mất khi tấn công/dùng skill; Polymorph vô hiệu hóa đòn đánh, có hạn chế mục tiêu và dispel; mana tạo cạnh tranh giữa phép | Khống chế cần chi phí, điều kiện, thời hạn và đối sách; autocast không đồng nghĩa spam miễn phí | Trinh sát/vòng bản đồ bằng invisibility mất nhiều ý nghĩa ở một lane. Polymorph dễ khó đọc hoặc khóa quân giá cao; **không mặc định vào MVP** |
| **S06 — Blizzard Classic guide, Human Combos**. Hướng dẫn chiến thuật, không phải quy chuẩn cân bằng. `source-read` | Đề xuất frontline kết hợp slow/heal và đội quân hỗn hợp; lưu ý phối hợp thời điểm của phép diện rộng | Giá trị đội hình và thời điểm hỗ trợ, không chỉ DPS riêng từng quân | Hero micro, teleport, khai thác worker và địa hình nhiều hướng không thuộc baseline lane. Không dùng hướng dẫn làm chứng minh matchup/win rate |
| **S07 — TapTap, trang Age of War 2**. Nguồn tổng hợp, **reference phụ**. `source-read` | Trang có gallery và mô tả mobile; không nâng thành nguồn luật chính thức | Điểm truy tìm reference thị giác cho D02 | Link ảnh CDN được thử trả `Cache miss`; chưa xác minh pixels trong lượt này. Không khẳng định collision, target hoặc giấy phép từ gallery; không nhập ảnh vào pack |
| **S08 — #118, comment concept 08/10/2026**. `source-read` nội dung comment, không tái kiểm ảnh | Comment phân loại ba hình là concept/reference; chưa có bằng chứng tác giả/giấy phép, sprite hay gameplay chạy thật | Brief thị giác 2D mịn, fantasy–technology, nhiều nhân vật nữ; chuyển cho D02 | Phối cảnh và nhiều hàng quân trong concept không chốt lane/collision. Không coi comment hay việc lưu hình là quyền sử dụng asset hoặc luật đã duyệt |

### Điều chưa xác minh của game tham chiếu

1. Roster theo từng age, đầy đủ damage/HP/armor/range, animation timing và rounding.
2. Thu nhập theo thời gian, bounty, XP, giá lên đời, refund và tác dụng nâng cấp.
3. Population/queue, giới hạn projectile/AoE, thuật toán mục tiêu và xử lý cùng chết.
4. Có giữ quân, tháp, HP căn cứ, cooldown và status khi tiến hóa hay không.
5. AI có đọc tài nguyên/queue riêng của người chơi, phản ứng tức thì hoặc bonus ẩn hay không.
6. Build/patch cụ thể mà các bảng Blizzard áp dụng; khác biệt tuning giữa Flash, Android và iOS.

**Quy tắc:** các ô này không được biến thành dữ liệu tham chiếu giả. Mọi con số khởi điểm trong bộ D01 phải ghi là `proposed/unbalanced`; nếu sau này cần đo reference, ghi phiên bản, kịch bản và kết quả quan sát riêng.

## 4. Hệ quả thiết kế gốc cho Tabula

Các mục trong phần này đều là **`proposed`**, lý do là **`inferred`**. Đây là giả thuyết cần kiểm bằng rules oracle và balance benchmark, không phải kết luận từ một bài mô tả store.

### 4.1 Sáu thời đại, sáu vai trò

Giữ sáu slot dễ học: **cận chiến cơ bản / tầm xa / hộ vệ / đột kích / hỗ trợ / công thành**. “Đột kích” là vai trò phá nhịp tuyến trước hoặc tận dụng cửa sổ yếu, không ngầm cho phép đi xuyên quân hay có lane thứ hai. Roster cụ thể và thông số thuộc tài liệu units/skills; bảng sau chỉ đề xuất câu hỏi kiểm chứng.

| Age Tabula | Hai hướng khác biệt chiến thuật cần thử | Câu hỏi benchmark |
|---|---|---|
| Hoang sơ | `dot` tạo áp lực kéo dài; `charge` tạo nhịp tiếp cận rõ | Đội hỗ trợ sống đủ lâu để bù sát thương tức thời? Charge có bị hộ vệ chặn? |
| Cổ đại | `volley` xử lý nhóm mục tiêu giới hạn; `guard`/`shred` tạo đấu giáp | Gom quân có tăng nguy cơ volley? Shred có cửa sổ khai thác và hết hạn rõ? |
| Phong kiến | `pierce` cho khắc chế giáp; `heal` tăng giá trị bảo vệ tuyến trước | Quân xuyên giáp có trade-off khi gặp quân rẻ? Heal có bị overwhelm? |
| Huyền thuật | `slow`/`root` kiểm soát nhịp; `dispel` khôi phục nhịp | Khống chế có thể nối vô hạn? Mất support có làm đội hình đổi kết quả? |
| Công nghiệp | `bombard` buộc cân nhắc mật độ; `shred` mở cửa sổ đẩy | Telegraph có thể đọc được trong lane? AoE có thắng mọi composition? |
| Tương lai | `summon`/`shield` chia ngân sách sống sót; `burst_heat` đánh đổi burst và downtime | Summon có vượt population? Đối thủ khai thác giai đoạn quá nhiệt được không? |

“Đọc telegraph” ở đây có thể dẫn tới đổi queue, dùng shield/dispel hoặc ngừng dồn quân; không hứa né bằng micro khi baseline không cho lệnh di chuyển từng unit. Các khác biệt phải xuất hiện trong kết quả kịch bản, không chỉ đổi tên, hiệu ứng hoặc nhân stats theo age. Không yêu cầu mọi trận đi hết sáu age; preset kiểm thử từng age phải cho phép nghiệm thu toàn bộ 36 unit.

### 4.2 Taxonomy đúng 16 họ tái sử dụng

IDs dùng để liên kết tài liệu/data: `guard`, `pierce`, `shred`, `charge`, `volley`, `dot`, `slow`, `root`, `knockback`, `heal`, `shield`, `dispel`, `chain`, `bombard`, `summon`, `burst_heat`.

| Nhóm | Ý nghĩa cần giữ trong một lane | Rủi ro và oracle tối thiểu |
|---|---|---|
| `guard`, `pierce`, `shred` | Chặn sát thương; xuyên giáp ở đòn hiện tại; giảm giáp có thời hạn là ba tác dụng riêng | Thứ tự modifier, floor/clamp, expiry; phân biệt pierce với shred qua ví dụ số độc lập |
| `charge`, `knockback` | Tiếp cận/đẩy tuyến có windup và giới hạn dịch chuyển, không tự mở pathfinding RTS | Va chạm, biên căn cứ, chống nối đẩy vô hạn; cùng fixture đổi phe phải đối xứng |
| `volley`, `chain`, `bombard` | Nhiều mục tiêu giới hạn; chuỗi suy giảm; đạn AoE có telegraph là ba kiểu chọn mục tiêu riêng | Cap mục tiêu, không đánh lặp một ID, tie-break ổn định, overkill và vị trí impact |
| `dot`, `slow`, `root` | Độc/cháy là variant của `dot`; chậm và đứng yên cần giới hạn tổng thời gian | Stack/refresh, tick tại expiry, dispel; không khóa vĩnh viễn hoặc tạo damage khi match đã terminal |
| `heal`, `shield`, `dispel` | Hồi phục, hấp thụ và xóa status tạo lựa chọn bảo vệ, có cap/chi phí | Không overheal, shield không nhân vô hạn, dispel đúng phía và đúng nhóm status |
| `summon`, `burst_heat` | Triệu hồi có lifetime/pop budget; burst đổi lấy giai đoạn suy yếu/quá nhiệt | Không nhân quân vượt cap, không farm bounty vô hạn; heat/recovery không reset nhờ đổi age |

`dot`/`slow`/`dispel` tham khảo nguyên lý Blizzard, còn luật Tabula tự thiết kế. `burst_heat` là đề xuất riêng. Bảng này không thay skill card: mỗi card vẫn phải chốt trigger, filter/priority, cost, range, timeline, stack, dispel/immunity, interrupt, cue, budget, counterplay và oracle. Một unit có thể dùng lại họ skill với cấu hình khác; không cần 36 thuật toán chiêu độc lập.

### 4.3 PVE công bằng và tiến hóa có chi phí cơ hội

- AI dùng cùng command validation, economy, cooldown, population và projection được phép như người chơi. Khác độ khó bằng policy, độ trễ quyết định và lookahead **được công khai**, không bằng tiền/HP vô hạn hay đọc queue bí mật. Chưa có bằng chứng reference làm vậy; đây là chuẩn của Tabula theo doc 00 §6.5.
- Benchmark sáu policy: rush, counter-compose, turtle, siege, tech-rush, mixed-army. Chạy cả mirror/đổi phe và cặp age liền kề; same-seed replay không thay thế oracle cân bằng độc lập.
- Mua quân, tháp, nâng cấp và lên age cạnh tranh ngân sách. Lên age mở lựa chọn, không xóa phí đã trả hoặc tạo hồi đầy căn cứ/reset phép miễn phí. Rules phải chốt queue/quân sống/status/cooldown trước khi đo hiệu quả tech.
- Bounty/income cần chống snowball; heal/turret cần chống stalemate. Cụ thể hóa timeout/anti-stall trong RULES, đo win/draw/stall và thời gian age. Không kết luận cân bằng từ một công thức DPS hoặc một win rate tổng.
- Hai phép chỉ huy mỗi age được mở theo ngữ cảnh; không biến 12 phép thành 12 nút đồng thời. Phép mạnh phải có tín hiệu, cửa sổ phản ứng và trade-off để tránh vòng lặp “đợi xóa sạch màn hình”.

### 4.4 Chiến đấu liên tục, authority có giới hạn

Hình ảnh liên tục không đòi canonical rules phụ thuộc render frame. Cần thiết kế input/timer hoặc clock adapter có thứ tự xác định, số nguyên/fixed-point, giới hạn entity/event và cách catch-up hữu hạn. Render có thể nội suy chuyển động từ `View`; không dùng render delta quyết định hit, gold hoặc thắng thua (I-3/I-10).

Tuy vậy, doc 00 là contract board-game runtime, không phải lời cấp quyền làm realtime engine. Trước implementation cần owner quyết định ADR tương thích hẹp: local single-player authority nằm ở đâu, luồng logical time nào được phép, pause/resume và replay ra sao, capability nào có consumer cụ thể. D01 chỉ ghi quyết định đề xuất và câu hỏi còn mở. Không gửi/lưu 60 frame/s thành 60 player commands/s; không mở production server, PvP, rollback hoặc phase exit.

## 5. Giới hạn chuyển sang D02 và bước triển khai

1. **Art không phải luật:** D02 được brief bằng S08, nhưng phải thiết kế side-view đọc được contact line, tầm đánh và cue. Chiều sâu của concept đi vào hậu cảnh; không âm thầm tạo nhiều hàng collision.
2. **Reference không phải asset:** không đưa ảnh, tên riêng, nhân vật, nhạc hoặc bảng stats tham chiếu vào runtime. Quyền sử dụng ảnh S07/S08 chưa xác minh; cần nguồn/giấy phép riêng trước nhập pack.
3. **Documented không phải tested:** nghiên cứu này đạt `source-read`; gameplay correctness, fairness, performance và balance đều chưa được thực thi trong research. Các chỉ tiêu batch 1.000/10.000 trận thuộc kế hoạch kiểm thử, không phải kết quả.
4. **Một nơi chốt luật:** khi RULES/skill cards/data chốt khác giả thuyết ở §4, cập nhật research để giữ ranh giới proposal rõ. Không duy trì hai danh sách luật cạnh tranh.

### Owner review còn chờ

| TODO | Quyết định cần duyệt | Trạng thái |
|---|---|---|
| R-01 | Xác nhận baseline 6 age × 6 unit và ý nghĩa sáu vai trò | `PENDING_OWNER_REVIEW` |
| R-02 | Xác nhận 16 họ skill; invisibility/polymorph không thuộc baseline MVP | `PENDING_OWNER_REVIEW` |
| R-03 | Chốt scope continuous single-lane PVE và ADR tương thích trước implementation | `PENDING_OWNER_REVIEW` |
| R-04 | Chốt chuẩn AI cùng luật/tài nguyên/thông tin và rubric đo anti-stall/snowball | `PENDING_OWNER_REVIEW` |
| R-05 | D02 giải quyết source/license cho art tham chiếu và quy tắc đọc chiến tuyến | `PENDING_OWNER_REVIEW` |

## 6. Nguồn và vị trí đã đọc

Tất cả nguồn dưới được truy cập **08/10/2026**. Liên kết chỉ để kiểm chứng/đối chiếu, không tuyên bố quyền tái sử dụng nội dung.

- **S01:** [Max Games Studios trên Google Play — Age of War 2](https://play.google.com/store/apps/details?id=com.maxgames.aow2&hl=en), mục **About this game**, **Updated on**, nhãn single-player.
- **S02:** [Max Games Studios trên App Store — Age Of War 2](https://apps.apple.com/us/app/age-of-war-2/id1194118663), mô tả ứng dụng và **Version History**.
- **S03:** [Newgrounds — Age of War 2, submission 537808](https://www.newgrounds.com/portal/view/537808), **Author Comments**, **Credits & Info**, ngày **Uploaded**.
- **S04:** [Blizzard Classic — Dryad](https://classic.battle.net/war3/nightelf/units/dryad.shtml), **Control**, **Slow Poison**, **Abolish Magic**, **Spell Immunity**.
- **S05:** [Blizzard Classic — Sorceress](https://classic.battle.net/war3/human/units/sorceress.shtml), **Slow**, **Invisibility**, **Polymorph**, phần tips/counters.
- **S06:** [Blizzard Classic — Human Combos](https://classic.battle.net/war3/human/combos.shtml), **Replace Footmen with Knights**, **Use your spellcasters!**, **Balanced Human Army**.
- **S07:** [TapTap — Age of War 2](https://www.taptap.io/app/45930), gallery/index và mô tả. Nguồn phụ; một ảnh CDN không tải được trong lượt nghiên cứu.
- **S08:** [#118 — comment về ba concept do owner gửi](https://github.com/loveoverflowcom/tabula/issues/118#issuecomment-6057763344), phân loại reference và giới hạn bằng chứng; không tái kiểm pixels ở đây.

Contract nội bộ: [doc 00](../../architecture/00-architecture-principles.md) §1.2, §3.1, §5, §6.5, I-3/I-5/I-6/I-10; [doc 02](../../architecture/02-game-module-and-sdk-design.md); [doc 04](../../architecture/04-frontend-and-design-system.md); [doc 08](../../architecture/08-first-games-validation-plan.md). Nếu đề xuất ở đây xung đột doc 00, doc 00 thắng cho tới khi ADR hợp lệ thay đổi contract.

[S01]: https://play.google.com/store/apps/details?id=com.maxgames.aow2&hl=en
[S02]: https://apps.apple.com/us/app/age-of-war-2/id1194118663
[S03]: https://www.newgrounds.com/portal/view/537808
[S04]: https://classic.battle.net/war3/nightelf/units/dryad.shtml
[S05]: https://classic.battle.net/war3/human/units/sorceress.shtml
[S06]: https://classic.battle.net/war3/human/combos.shtml
[S07]: https://www.taptap.io/app/45930
[S08]: https://github.com/loveoverflowcom/tabula/issues/118#issuecomment-6057763344
