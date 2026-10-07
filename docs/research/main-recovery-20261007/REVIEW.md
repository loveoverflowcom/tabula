# Review patch được push nhầm vào main — 07/10/2026

Review phạm vi đã ghim, phục vụ chuyển phần thay đổi có ích sang develop. Kết luận của input review: patch được bảo toàn đúng byte; không nên áp dụng nguyên snapshot Android lên develop. Sáu finding dưới đây thuộc patch gốc. Bản tích hợp cuối đã giữ tooling của develop và chuyển phần giao diện vào cây CMP hiện có; disposition và review cuối được ghi ở cuối báo cáo.

## Danh tính source và patch

| Vai trò | Danh tính |
|---|---|
| BASE của hai commit push nhầm | `70b0fcd721b0b04cc76559cbc4fc72151bee613a` |
| Commit thứ nhất | `55d2354e6c4817fb67d1655859e8c1fa161f4d84` |
| HEAD gốc của main | `6170dddc2769fb1a476c37132438bce29bac6c7e` |
| develop trước tích hợp | `80d9fdb96f18cd59fa85c9401b533d66bf04b5d7` |
| Tree HEAD gốc | `00c3584a1803780f2ed541745e691ac02722e214` |
| `original-net.patch` SHA-256 | `25b5e439abad73f64ad95e110f91d5289a1ce264739c6a7bc138307acaddd0cb` |
| `original-two-commits.mbox` SHA-256 | `1b2fa8e0559f82efe2001f9a02e8e1004e6505c6dfb27647c721976729f9d26f` |
| `embedded-android.patch` SHA-256 | `d5aaa918cfcc618560ff4122d442f8b0f25caeca37103072c88c4226d7d84a3b` |

Phạm vi original review là `70b0fcd..6170ddd`, không thay bằng diff develop-to-main. Review khả năng tích hợp còn đối chiếu từng path/blob/mode với develop đã ghim. Các số dòng dưới đây thuộc HEAD gốc `6170ddd`; path root `app/` là Android project bổ sung trong patch, không phải `apps/mobile/android/` hiện có. Architecture doc 00 và ADR đã chấp nhận ở develop là tiêu chuẩn đánh giá; nội dung hướng dẫn mới trong patch là dữ liệu được review.

## Findings

### 1. CONTRACT_DRIFT / P1 / HIGH — project Android thứ hai tự sở hữu game rules

**Vị trí:** `settings.gradle.kts:17`; `app/src/main/java/com/tabula/app/MainActivity.kt:120`; `app/src/main/java/com/tabula/app/viewmodel/TabulaViewModel.kt:74`.

**Trigger và hậu quả:** build/chạy project root mới sẽ chỉ include `:app`. Các callback Play dispatch trực tiếp theo `gameId` sang Chess/Tic-Tac-Toe Kotlin; ViewModel giữ state, thực hiện moves, chạy bots và ghi kết quả. Đây là đường gameplay độc lập với CMP `apps/mobile` và Rust/Macroquad. Nó đi ngược doc 00 về ownership/determinism và ADR-0043 về một mobile tree cùng Rust sở hữu rules/projection/presentation; không có native GameHost adapter hay ADR mở ngoại lệ cho Kotlin rules.

**Bằng chứng/BASE:** source-read toàn dispatch và ViewModel; những root project/game files này không tồn tại ở BASE hay develop. develop có cây CMP và production GameHost unavailable rõ ràng. Pure Kotlin harness còn cho 10 bot moves khác nhau khi chạy 100 lần trên cùng initial ChessState (`ChessEngine.kt:252` dùng `.random()`), không có seed/input log tương ứng. Đây không phải bằng chứng native gameplay.

**Tích hợp nhỏ nhất:** chuyển phần shell/artwork/settings hữu ích vào `apps/mobile/shared`; giữ generated catalog, semantic tokens, GameHost và native-runtime gates. Không import root Gradle project, Kotlin game engines, fake room/rating/history authority hoặc thay Rust runtime.

### 2. DEFECT / P2 / HIGH — Chess không kết thúc sau checkmate và chấp nhận nước đi bỏ qua chiếu

**Vị trí:** `app/src/main/java/com/tabula/app/model/ChessEngine.kt:115`; `:133`.

**Trigger:** chuỗi `f2-f3, e7-e5, g2-g4, Qd8-h4` là Fool's Mate. Đường `executeMove` chỉ kết thúc khi king bị capture; `computeLegalTargets` không lọc king safety.

**Expected/actual:** trạng thái phải kết thúc với Black thắng. Harness báo `isGameOver=false, isCheck=false`, sau đó vẫn chấp nhận `a2-a3`. Người chơi được tiếp tục một ván đã checkmate; UI/result recorder không có outcome đúng.

**Bằng chứng/BASE:** example-tested qua pure JVM harness trên source gốc, kết hợp source-read complete move/target path. Source mới không có ở BASE; develop sử dụng Rust chess thay vì engine này.

**Tích hợp nhỏ nhất:** giữ Rust game owner, không đưa Kotlin Chess vào app production. Nếu phát triển native gameplay sau này, dùng đúng GameHost/native runtime contract và conformance hiện có.

### 3. DEFECT / P2 / HIGH — bot coroutine cũ có thể thực hiện nước đi trên ván vừa reset

**Vị trí:** `app/src/main/java/com/tabula/app/viewmodel/TabulaViewModel.kt:100`; `:140`; `:157`.

**Trigger:** người chơi tạo bot turn rồi bấm Reset trong thời gian `delay(600)` của Chess hoặc `delay(400)` của Tic-Tac-Toe. Reset thay state nhưng không cancel job hoặc đổi identity/generation của match. Coroutine thức dậy và lấy `_uiState.value` hiện tại.

**Expected/actual:** tác vụ của ván cũ phải bị hủy hoặc từ chối; thực tế nó chọn và áp dụng move vào ván mới. Chess bot chọn pieces theo `turn` hiện tại (`ChessEngine.kt:223`), nên có thể đánh quân White trên board mới. Tic-Tac-Toe reset về X nhưng stale bot call vẫn áp `makeMove` vào state đó. Cũng không có destination/match fence khi rời màn hình hoặc bắt đầu ván khác.

**Bằng chứng/BASE:** HIGH dựa trên causal source trace qua coroutine, reset và model calls; chưa thực thi ViewModel/coroutine Android race. Đường này mới được thêm, không có ở BASE/develop.

**Tích hợp nhỏ nhất:** không import bot authority này; giữ lifecycle coordinator/GameHost của CMP. Một runtime adapter tương lai phải fence theo instance và cancel/join work theo contract hiện có.

### 4. DEFECT / P2 / HIGH — Tic-Tac-Toe hòa bị ghi thành thua, mất Elo và tăng losses

**Vị trí:** `app/src/main/java/com/tabula/app/viewmodel/TabulaViewModel.kt:136`; `:149`; `:165`.

**Trigger:** chuỗi ô `[0, 1, 2, 4, 3, 5, 7, 6, 8]` kết thúc với `MatchStatus.DRAW`. Cả human/bot terminal path gọi recorder bằng Boolean `status == WON_X`, nên DRAW trở thành `false`.

**Expected/actual:** hòa phải giữ outcome hòa; recorder chuyển mọi `false` thành `Defeat`, `-10 Elo` và `losses + 1` ở dòng 173/181/183. Đây là mất thông tin outcome, không phải thiếu test đơn thuần.

**Bằng chứng/BASE:** harness thực thi chuỗi cho `TTT terminal=DRAW, result-recorder isWin=false`; hậu quả persistence/rating được xác lập bằng source-read recorder, chưa chạy Room DB. Recorder/Kotlin game là source mới, không có ở BASE/develop.

**Tích hợp nhỏ nhất:** không import rating/history giả từ standalone app. UI History trong CMP cần trình bày trung thực unavailable state cho đến khi có port/authority và acceptance được cho phép.

### 5. DEFECT / P2 / HIGH — debug signing phụ thuộc keystore không được cung cấp

**Vị trí:** `app/build.gradle.kts:23`; `:32`.

**Trigger:** assemble debug trên checkout sạch của root Android project. Build thay signing mặc định bằng `debugConfig`, trỏ tới `${rootDir}/debug.keystore`; file không được track, không có generator/task tạo nó, và patch thêm ignore chính file đó.

**Expected/actual:** debug build phải tự dùng signing debug chuẩn hoặc có input được khai báo. Cấu hình hiện tại yêu cầu file không tồn tại nên signing/assembly không thể hoàn tất trên checkout sạch. Không có Gradle/Android execution trong original review, do đó đây là source-established build defect, không phải báo cáo observed Gradle failure.

**Bằng chứng/BASE:** `git ls-tree` xác nhận không có `debug.keystore` hay root wrapper trong HEAD; đọc toàn signing configuration và paths. develop dùng project `apps/mobile` hiện có, không có custom signing này.

**Tích hợp nhỏ nhất:** giữ Android host/signing configuration hiện có của CMP; không import custom root signing hoặc keystore credentials.

### 6. DEFECT / P2 / HIGH — xóa root Cargo.lock chặn gate online-match chạy --locked

**Vị trí:** `Cargo.lock:1` (deletion trong commit `55d2354`); consumer `.github/workflows/online-match.yml:87`.

**Trigger:** checkout mới không có root Cargo.lock, chạy job online-match. Lệnh root đầu tiên là `cargo test -p tabula-storage --features online-match-postgres --locked ...`. Các Cargo commands trước đó dùng workspace độc lập `tests/online-match` (manifest có `[workspace]` riêng), nên không tạo root lockfile.

**Expected/actual:** gate phải chạy non-empty real-Postgres selection với dependency inputs cố định. Cargo phải tạo root lockfile nhưng bị `--locked` từ chối; acceptance chưa thể được chọn/thực thi.

**Bằng chứng/BASE:** causal source-read toàn workflow và hai workspace contexts; không chạy Rust repro. Original BASE và develop đều giữ root lock; develop lock 4.460 dòng cùng workflow này. Xóa lock là thay đổi mới.

**Tích hợp nhỏ nhất:** giữ nguyên Cargo.lock của develop, loại hunk xóa file.

## Duplicate inventory và quyết định tích hợp

Net range có 107 changed paths, 15 binary paths; đối chiếu HEAD với develop có 56 path cùng blob/mode và 51 path khác hoặc thiếu. Các nhóm trùng không cần được import lại:

- Sáu workflows `accounts-social`, `dashboard-design01`, `kanidm-oidc`, `match-postgres`, `online-match`, `session-postgres`: cùng exact blob/mode develop.
- `.cargo/mutants.toml`, `.sqlx/README.md` và 14 query JSON được thêm trong original range: cùng exact blob/mode develop. Giữ hai query caches develop-only `e8608c...` và `f6b9ba...`; không tái sinh hoặc xóa metadata trong review.
- Ba canonical Tabula skills, README, references, UI metadata và helper contents: cùng nội dung develop. Hai ai-doc helper/test files chỉ khác mode main `100644` / develop `100755`; giữ mode develop. Không khôi phục legacy `rust-*` paths.
- `ci.yml` và `nightly.yml` không bị thay trong original range; develop có cập nhật mới hơn snapshot main. Giữ develop.
- Phần asset relocation, mobile ignore và artifact ignore cũ của `.gitignore` đã có trên develop. Chỉ sáu generic patterns mới được lấy từ input: `.gradle/`, `*.hprof`, `debug.keystore`, `debug.keystore.base64`, `app/build/`, `.build-outputs/`. Đã apply sáu patterns với một comment giải thích; `git check-ignore -v` và `git diff --check -- .gitignore` PASS. Không có defect actionable trong delta này.

**Lưu ý policy gốc:** `.agents/skills/README.md:32`/`:39` tuyên bố legacy definitions đã được dọn, nhưng `6170ddd` còn bốn `rust-*/SKILL.md` và `.claude/skills` là tree thường. New checker tại dòng 232 sẽ từ chối extra definitions và dòng 208 từ chối bridge không phải symlink. Đây là source-established drift của snapshot gốc; develop đã sửa. Giữ skill tree/bridge develop giải quyết mâu thuẫn mà không cần thay enforcement.

## Kiểm tra bảo toàn patch

Evidence chi tiết ở `integrity-review.json`; phần này xác nhận serialization/application, không xác nhận tính đúng game hay native app:

- Embedded patch: `git apply --check` và apply `--index` trên scratch develop PASS; cả 45 paths tái tạo cùng exact bytes HEAD gốc, gồm 39 `app/` paths và 15 binaries. Reverse application trở lại sạch.
- Full net patch: reconstructed tree bằng chính HEAD tree `00c3584a1803780f2ed541745e691ac02722e214`; reverse sạch. Đây là content identity, không phải source có thể merge nguyên vào develop.
- Mbox: `git am` với hooks disabled tái tạo đúng hai original commit trees; checkout sạch.
- Mbox còn 9 intermediate `.gradle` cache files, tổng 797.482 bytes. Net patch bỏ toàn bộ caches đó. Cache artifacts không được lấy làm code hay build evidence.

## Evidence thực thi và giới hạn

Pure Kotlin harness dùng K2 compiler 2.4.0 có sẵn trong cached Gradle 9.7.0, JDK 17; source models được trích từ HEAD gốc. Compile command:

```bash
java -cp '/Users/manh.pd1/.gradle/wrapper/dists/gradle-9.7.0-bin/d4tj7w02tcgubx9zk9hbippn6/gradle-9.7.0/lib/*' org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect -classpath /Users/manh.pd1/.gradle/wrapper/dists/gradle-9.7.0-bin/d4tj7w02tcgubx9zk9hbippn6/gradle-9.7.0/lib/kotlin-stdlib-2.4.0.jar -jvm-target 17 -d /private/tmp/tabula-main-android-review/classes /private/tmp/tabula-main-android-review/ChessEngine.kt /private/tmp/tabula-main-android-review/TicTacToeEngine.kt /private/tmp/tabula-main-android-review/Repro.kt
```

Runtime command, thực thi riêng:

```bash
java -cp '/private/tmp/tabula-main-android-review/classes:/Users/manh.pd1/.gradle/wrapper/dists/gradle-9.7.0-bin/d4tj7w02tcgubx9zk9hbippn6/gradle-9.7.0/lib/kotlin-stdlib-2.4.0.jar' ReproKt
```

Output gốc:

```text
FoolsMate: isGameOver=false, isCheck=false
Move while checkmated accepted=true
TTT terminal=DRAW, result-recorder isWin=false
Identical initial states: distinct bot moves=10
```

Đây là example-tested pure models và reproduction của defects, không phải Android/CMP compilation, ViewModel integration, Room persistence, pixel inspection hay device execution.

Tooling checks ban đầu:

| Check | Trạng thái/phạm vi |
|---|---|
| Exact Git blob/mode inventory ở ba pins | PASS |
| `PYTHONDONTWRITEBYTECODE=1 python3 .agents/skills/tabula-engineering/scripts/test_ai_doc_contracts.py` | PASS, 6 tests, 0 failures; helper/harness contents trùng original HEAD và develop |
| `.../check_skills.py` | BLOCKED: host Python thiếu `yaml`; không cài dependency |
| `.../test_check_skills.py` | BLOCKED cùng nguyên nhân; 0 tests thực thi |
| Rust/core/mobile/provider/PostgreSQL/browser acceptance của original patch | NOT_RUN trong các subreviews này |
| CI dispatch và merge enforcement | Không dispatch; enforcement chưa kiểm tra |

Đã đọc helper/runner trước khi chạy: ai-doc đọc source/rustdoc và xuất stdout; unit fixtures ghi trong TemporaryDirectory; skill checker safe-parse YAML rồi kiểm tra metadata/link/symlink, không sửa source. Bất kỳ check trên phiên bản CMP được tích hợp sau review cần được ghi riêng theo source/config hiện tại, không dùng kết quả này làm PASS cho patch mới.

## Prerequisites local cho gate mobile được tích hợp

Đã discover read-only, chưa launch Gradle trong subtask này để tránh tranh Gradle locks:

- JDK mặc định: OpenJDK Microsoft 17.0.19; home `/Users/manh.pd1/Library/Java/JavaVirtualMachines/ms-17.0.19/Contents/Home`.
- Android SDK: `/Users/manh.pd1/Library/Android/sdk`; platform package `platforms;android-37.0` (api-level 37.0), build-tools 37.0.0, license files hiện diện. Folder `platforms/android-37` không có; tên package thực tế là `android-37.0`.
- Gradle 9.7.0 distribution và `.zip.ok` đã cache tại `/Users/manh.pd1/.gradle/wrapper/dists/gradle-9.7.0-bin/d4tj7w02tcgubx9zk9hbippn6/gradle-9.7.0`.
- Cache directories cho AGP 9.3.1, Kotlin Gradle plugin 2.4.20 và Compose Gradle plugin 1.12.0 hiện diện. Chỉ directory presence không chứng minh mọi dependency đầy đủ hoặc build PASS.
- `apps/mobile/gradlew` executable và wrapper JAR hiện diện. `JAVA_HOME`, `ANDROID_HOME`, `ANDROID_SDK_ROOT` chưa được đặt; `apps/mobile/local.properties` không có. Đặt task env trỏ JDK/SDK hiện có khi root chạy gate; không cần tạo/copy root Android project.

Gate bắt buộc cho thay đổi dưới `apps/mobile`: từ directory đó chạy `./gradlew :shared:testAndroidHostTest :android:assembleDebug`. Source/build success, shared layout pixels và native Android/iOS gameplay/device acceptance vẫn là các claim khác nhau.

## Disposition và review bản tích hợp cuối

Người dùng đã yêu cầu sửa code thực tế, dùng patch làm đầu vào. BASE tích hợp là
`80d9fdb96f18cd59fa85c9401b533d66bf04b5d7`; containing commit ghim toàn bộ bản thay đổi.
Inventory mobile cuối gồm 19 changed paths, tính cả source, test, build configuration và JPEG;
SHA256 của sorted source manifest là
`d22cc4609098133fd74a19e428daa8788eccac720e1620e667d70492da207cd6`.

| Input finding | Disposition trên develop |
|---|---|
| 1: Android project thứ hai/Kotlin rules | Tích hợp vào CMP hiện có; giữ Rust/GameHost ownership và native-unavailable state |
| 2–4: Chess, bot reset, draw/rating | Không import các engine, coroutine và result store lỗi; không tuyên bố đã sửa chúng trong snapshot gốc |
| 5: debug signing phụ thuộc keystore riêng | Không import signing configuration gốc; APK của cây Android hiện có build PASS |
| 6: xóa Cargo.lock | Giữ nguyên Cargo.lock của develop; `.agents` và `.claude` cũng giữ nguyên |

Review read-only độc lập trên bản source cuối không còn actionable finding trong phạm vi này.
Đã trace các caller/navigation/save/restore paths, enum allow-list, account cancellation/current
identity fences, generated catalog consumers, palette/copy và thời điểm tạo GameLaunch. Đã kiểm
tra complete changed functions và Android KMP resource packaging; APK được đối chiếu exact
bytes với resource source. JPEG được kiểm bằng blob/hash và pixels, không review như source code.

| Claim / owner | Barrier và consumer đã kiểm | Evidence / disposition |
|---|---|---|
| Một CMP tree; Rust sở hữu gameplay / doc 00, ADR-0043 | Không thêm root project/Kotlin engines; existing GameHost seam giữ nguyên | Source-reviewed, policy/APK checks PASS; native gameplay chưa mở |
| Local presentation choices / ShellPreferences, TabulaApp | Ba enum hữu hạn, malformed save reset, OS reduced-motion request, fresh launch snapshot | Example-tested: model và Compose UI PASS |
| Rooms/History không tạo authority / BackStack, AccountScreens, ShellToolsScreens | Public task routes, Account section selection, Library recovery, không sample results | Source-reviewed và UI navigation/restoration tests PASS |
| Home image dùng resource thật / DiscoveryArt, Android KMP build | Generated Res, adaptive dimensions, `androidResources.enable`, final APK entry | Build/packaging PASS và inspected pixels ở wide layout |
| Catalog/filter behavior / DiscoveryScreens | Registry categories, AND với filters hiện có, saved query | Existing và updated Compose tests PASS |

Kết quả cuối: portable core gate PASS; 163 Android host tests và 59 desktop Compose tests PASS;
APK/policy checks PASS; hai iOS Kotlin targets compile PASS. Commands, toolchain, ảnh đã inspect
và giới hạn được ghi tại [evidence của implementation](README.md#evidence). Source không đổi
sau các gate này ngoài báo cáo evidence.

Residual scope: Rooms/History vẫn cần native service adapters; native GameHost, provider/social,
linked iOS app và device acceptance không được chứng minh bởi patch này. Đây là review delta và
evidence của shell adaptation, không phải full-game audit, phase-exit hay whole-repository approval.
