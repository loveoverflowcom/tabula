# Nghiên cứu chiến lược nền tảng Board Game Rust/WASM cho Web và Mobile

## Kết luận điều hành

Sau khi mở rộng khảo sát từ câu hỏi “engine nào hợp Rust 2D/WASM?” sang **toàn bộ bài toán sản phẩm, thị trường, multiplayer, vận hành, mobile, bản quyền và pháp lý**, mình sẽ điều chỉnh kết luận ban đầu một chút:

> **Không nên xây một “game engine” tổng quát ở giai đoạn đầu. Hãy xây một `board-game platform/runtime` bằng Rust, trong đó rules/domain là lõi độc lập; Macroquad chỉ là một renderer/client adapter.**

Macroquad vẫn là lựa chọn game renderer mình đánh giá cao nhất cho yêu cầu **WASM-first + Android/iOS native graphics + 2D nhẹ**: dự án chính thức hỗ trợ Windows/Linux/macOS, HTML5, Android và iOS; cùng codebase giữa các nền tảng, có geometry batching cho 2D, dependency tree nhỏ, immediate-mode UI và workflow deploy cho WASM/Android. Miniquad bên dưới còn nhẹ hơn và hỗ trợ Windows/Linux/macOS/iOS/WASM/Android, nhưng đổi lại bạn phải tự sở hữu nhiều lớp engine hơn. citeturn18search0turn18search1

Tuy nhiên, với **ma sói, lobby, room, chat, profile, shop, tournament, history, admin dashboard**, phần khó lại không phải sprite rendering mà là **form, text input, responsive layout, accessibility, localization, chat và UI business application**. Dioxus 0.7 hiện hỗ trợ Web/Desktop/Mobile, Android/iOS tooling, WASM code splitting, server functions và một bộ primitive có keyboard/ARIA accessibility; do đó Dioxus đáng được dùng cho **portal/dashboard/social shell**, thay vì cố vẽ mọi button và input bằng Macroquad. citeturn18search3turn18search5

Kiến trúc mình đề xuất cuối cùng là:

```text
                       ┌──────────────────────────┐
                       │      Rust Workspace      │
                       └────────────┬─────────────┘
                                    │
                 ┌──────────────────▼──────────────────┐
                 │         boardgame-core              │
                 │                                     │
                 │ Rules / State / Action / RNG        │
                 │ Turn / Replay / Validation          │
                 │ Visibility / PlayerView             │
                 │ NO Macroquad / NO HTTP / NO DB      │
                 └──────────┬────────────────┬─────────┘
                            │                │
              WASM/native   │                │ native
                            │                │
          ┌─────────────────▼───┐       ┌────▼──────────────────┐
          │ Client              │       │ Backend               │
          │                     │       │                       │
          │ Macroquad           │       │ Tokio + Axum          │
          │ 2D gameplay         │       │ WebSocket + HTTP      │
          │                     │       │ SQLx + PostgreSQL     │
          └──────────┬──────────┘       └────────┬──────────────┘
                     │                           │
        Web/WASM ────┤                           │
        Android ─────┤                           │
        iOS ─────────┘                           │
                                                 │
          ┌─────────────────────┐                │
          │ Dioxus              │◄───────────────┘
          │                     │
          │ Landing / Lobby     │
          │ Account / Chat      │
          │ Shop / Dashboard    │
          │ Admin / CMS         │
          └─────────────────────┘
```

Với **cờ, bài đơn giản, ma sói**, thậm chí không nhất thiết phải dùng Macroquad ngay. Dioxus + HTML/CSS/SVG đủ khả năng làm rất nhiều board UI. Nhưng nếu mục tiêu ngay từ đầu là cảm giác “game client” với drag/drop mượt, zoom board, card animation, particles, shaders và native GPU trên mobile, **Macroquad vẫn là lựa chọn mặc định hợp lý hơn Dioxus cho phần bàn chơi**. Đây là quyết định kiến trúc, không phải vì Dioxus không chạy mobile: Dioxus 0.7 thực sự hỗ trợ mobile và có cả hướng native renderer dựa trên WGPU, nhưng framework này vẫn thiên về application UI hơn một 2D game runtime. citeturn18search3turn18search9

**Stack mình sẽ chọn nếu bắt đầu dự án hôm nay, ngày 28/08/2026:**

| Layer | Khuyến nghị |
|---|---|
| Game rules | **Pure Rust crate** |
| Shared protocol | Rust + Serde |
| 2D gameplay | **Macroquad** |
| Low-level renderer | Miniquad gián tiếp |
| Web portal/dashboard | **Dioxus 0.7** |
| Backend | **Tokio + Axum** |
| Realtime | **WebSocket** |
| Database | **PostgreSQL + SQLx** |
| Cache/presence | Chưa cần Redis ở MVP |
| Asset delivery | Object storage + CDN |
| Web game | WASM |
| Android/iOS | Macroquad native target |
| Admin | Dioxus web |
| Deployment | Container + managed PostgreSQL |
| Orchestration | **Không Kubernetes lúc đầu** |
| Architecture | Modular monolith |
| Multiplayer authority | Server authoritative |
| Persistence | Event log + snapshots |
| Observability | Logs + metrics + traces |
| CI | Linux + WASM + Android + macOS/iOS matrix |

Mình **không khuyến nghị viết trực tiếp Miniquad ở MVP**. Miniquad phù hợp khi sau 2–3 game bạn đã chứng minh được abstraction chung của mình và thực sự muốn biến nó thành một board-game SDK/framework riêng. Miniquad tự mô tả là lớp graphics abstraction cực nhẹ, tập trung portability và low-end hardware; chính vì thế, nhiều thứ Macroquad đã làm sẵn sẽ quay lại thành trách nhiệm của bạn. citeturn18search1


## Thị trường, đối thủ và loại sản phẩm nên xây

### Thị trường có thật, nhưng “có thị trường” không đồng nghĩa “còn chỗ cho một BGA clone”

Các chỉ dấu thị trường đều khá mạnh. Kickstarter công bố riêng năm 2024 có **6.646 dự án tabletop được launch, 5.314 dự án gọi vốn thành công, tỷ lệ thành công 80% và khoảng 220 triệu USD được pledge cho các chiến dịch tabletop thành công**; tabletop chiếm 83% pledge của toàn category Games trên Kickstarter năm đó. Đây không phải TAM của game online, nhưng là bằng chứng rất rõ rằng board game vẫn có một creator/fandom economy lớn. citeturn22search0

Một ước lượng thương mại cập nhật tháng 8/2026 của Mordor Intelligence đặt thị trường board game vật lý toàn cầu khoảng **18,95 tỷ USD năm 2026**, dự báo **30,06 tỷ USD năm 2031**, CAGR 9,67%; cùng hãng ước lượng riêng online board games khoảng **2,72 tỷ USD năm 2026**. Những số này nên được coi là **ước lượng directional**, không phải số liệu kiểm toán, vì định nghĩa “board game” và phương pháp của các hãng market research khác nhau. citeturn22search2turn22search4

Điều đáng chú ý hơn TAM là các sản phẩm đang hoạt động thực tế:

| Sản phẩm | Tín hiệu quy mô/đặc điểm | Bài học đáng lấy |
|---|---|---|
| **Board Game Arena** | Danh mục hiện khoảng **1.347 game**, chơi trực tiếp browser | Catalog + browser-first + nhiều game dùng chung platform |
| **Tabletopia** | **2.500+ board game**, sandbox, không có AI enforce luật | Có thị trường cho “virtual tabletop”, không nhất thiết automate mọi luật |
| **Wolvesville** | Google Play quảng bá **10 triệu người chơi**, tối đa 16 người, 100+ role, cross-platform web/iOS/Android | Ma sói có thể trở thành live-service riêng, không chỉ mini-game |
| **Plato** | Công ty công bố **115 triệu users, 10 triệu chơi mỗi tháng**, 1 tỷ phút/tháng | Chat/social graph có thể quan trọng ngang game |
| **Chess.com** | Công bố vượt **250 triệu member** trong 2026 | Một game cổ điển vẫn đủ lớn để thành platform |
| **Lichess** | Chính trang của Lichess cho biết hơn **5 triệu ván/ngày** | Có chỗ cho mô hình community/open-source, nhưng quy mô vận hành không nhỏ |

BGA hiện hiển thị 1.347 game và tập trung vào chơi trực tiếp từ browser; Tabletopia quảng bá hơn 2.500 game nhưng chủ ý là sandbox “không AI enforce luật”, trong khi Wolvesville chứng minh một phiên bản social-deduction chuyên sâu có thể hỗ trợ 16 người, hơn 100 role và cross-platform. citeturn16search0turn16search6turn17search0

Plato là đối chứng đặc biệt quan trọng cho hướng của bạn: công ty công bố hơn 115 triệu user, khoảng 10 triệu chơi hàng tháng và hơn một tỷ phút sử dụng mỗi tháng. Điều này gợi ý một thesis sản phẩm mạnh hơn “website chứa nhiều board game”: **một social room nơi game là hoạt động để bạn bè gặp nhau**. citeturn16search2

Chess cũng chứng minh một vertical duy nhất có thể cực lớn: Chess.com công bố hơn 250 triệu member, còn Lichess cho biết cộng đồng của họ chơi hơn năm triệu ván mỗi ngày. Đây là member/game-count do chính nền tảng công bố, không phải số MAU độc lập, nhưng vẫn là bằng chứng mạnh về nhu cầu chơi board game online lặp lại với tần suất cao. citeturn17search1turn17search5

### Bốn archetype sản phẩm

Từ các đối thủ trên, thị trường thực tế tách thành bốn mô hình:

**Rules-enforced platform**, gần BGA:

```text
Game definition
     ↓
platform knows rules
     ↓
legal moves
turn sequencing
scoring
game over
replay
ranking
```

Đây là hướng hợp nhất với Rust vì rules engine deterministic có thể dùng lại ở client, server, test và simulation. Điểm yếu là mỗi game mới cần implementation và QA sâu.

**Virtual tabletop/sandbox**, gần Tabletopia/Tabletop Simulator:

```text
virtual table
cards
dice
pieces
deck
zones
physics / drag
voice/chat

Rules mostly handled by players
```

Tabletopia xác nhận chính họ là sandbox không dùng AI để enforce rules nhưng tự động hóa setup, camera, shuffle/deal, turn tracking và zones. Hướng này mở rộng catalog nhanh hơn, nhưng UX trên smartphone và cheat prevention yếu hơn rules-enforced games. citeturn16search6

**Single-game live service**, như Wolvesville hoặc Chess.com:

```text
One core game
   +
rank
seasons
roles
skins
friends
clans
events
tournaments
spectator
replay
```

Wolvesville đã phát triển mô hình Werewolf thành hơn 100 role, private/public matches và cross-platform, chứng minh rằng “ma sói” không nhất thiết chỉ là một feature nhỏ trong portal. citeturn17search0turn17search3

**Social-games platform**, gần Plato:

```text
Chat / friends / room
          ↓
     pick activity
          ↓
 chess / cards / party / pool / ...
```

Theo mình đây là **mô hình có tiềm năng phù hợp nhất cho một startup/indie team Việt Nam**, nhưng chỉ khi có distribution/community rõ ràng. Plato cho thấy social layer có thể là phần lõi chứ không phải phụ kiện của game. citeturn16search2

### Chỗ trống mình thấy đáng thử

Mình sẽ **không** pitch sản phẩm là:

> “Board Game Arena nhưng viết bằng Rust.”

Technology không phải moat, và cạnh tranh catalog với 1.347–2.500+ game ngay từ đầu là bài toán licensing/content acquisition khổng lồ. citeturn16search0turn16search1

Mình sẽ pitch gần hơn:

> **“Discord/Plato cho game night, nhưng browser-first, Vietnamese/SEA-first và tập trung party/strategy board game.”**

User journey:

```text
Bạn A mở URL
      ↓
Create Room
      ↓
copy link / QR
      ↓
Messenger / Discord / Zalo
      ↓
friends join
      ↓
chọn game
      ↓
play
      ↓
rematch / switch game
```

Killer feature ban đầu không nên là renderer đẹp nhất mà là:

```text
Create room < 5 sec
Share link
Join nhanh
Reconnect tốt
Chat tốt
Game start nhanh
Mobile browser dùng được
Không bắt download trước khi biết game có vui hay không
```

BGA đã chứng minh browser/no-download là một mô hình khả thi ở quy mô catalog lớn, còn Wolvesville chứng minh cross-platform là thuộc tính quan trọng đối với social deduction. citeturn16search3turn17search0

### Game nào nên làm trước

Mình sẽ không launch mười game. Ba game đầu nên cố tình kiểm thử ba loại primitive khác nhau:

| Game | Mục đích kỹ thuật | Mục đích sản phẩm |
|---|---|---|
| **Chess/Checkers-like** | Grid, piece movement, timer, deterministic rules, replay | Benchmark engine |
| **Original card game** | Hidden state, deck, hand, animation, RNG | Kiểm thử card runtime + monetizable IP |
| **Werewolf/social deduction** | 8–16 player, secret roles, chat, timers, voting, reconnect | Kiểm thử social/network effect |

Sau ba game này, bạn gần như đã đụng tới toàn bộ primitive quan trọng:

```text
Board
Piece
Grid
Card
Deck
Hand
Zone
Hidden information
Public information
Turn
Phase
Timer
Vote
Randomness
Animation
Chat
Reconnect
Spectator
Replay
```

Chỉ khi những abstraction này lặp lại qua nhiều game thì mới nên tách chúng thành `boardgame-runtime`. **Đừng thiết kế framework hoàn chỉnh dựa trên tưởng tượng trước khi ship game thứ nhất.**


## Game engine và frontend: Macroquad thắng ở renderer, Dioxus thắng ở application UI

### So sánh lại engine theo đúng workload

| Công nghệ | WASM | Android/iOS | UI business | 2D game | Footprint/complexity | Vai trò đề xuất |
|---|---:|---:|---:|---:|---:|---|
| **Macroquad** | ★★★★★ | ★★★★★ | ★★ | ★★★★★ | Thấp | **Gameplay mặc định** |
| **Miniquad** | ★★★★★ | ★★★★★ | ★ | ★★★★ | Rất thấp nhưng phải tự build | Foundation sau này |
| **Dioxus 0.7** | ★★★★★ | ★★★★☆ | ★★★★★ | ★★☆ | Trung bình | **Portal/lobby/dashboard** |
| **Bevy** | ★★★★☆ | ★★★★☆ | ★★★ | ★★★★★ | Cao hơn nhu cầu | Khi game phức tạp hơn |
| egui/eframe | ★★★★ | Mobile kém “consumer app” hơn | ★★★★ | ★★ | Thấp-trung | Internal tools/editor |
| Leptos | ★★★★★ | Không phải native-mobile game stack | ★★★★★ | ★★ | Trung bình | Web admin/website |

Macroquad chính thức mô tả same-code cross-platform, automatic geometry batching cho 2D, immediate-mode UI, HTML5/Android/iOS support. Miniquad đi thấp hơn và hỗ trợ iOS GLES/Metal, WASM WebGL, Android GLES. Bevy hiện cũng hỗ trợ Web/iOS/Android và có 2D/3D renderer, custom ECS UI, scenes, animation và render graph. citeturn18search0turn18search1turn18search2

Vì vậy việc mình không chọn Bevy ở đây **không phải vì Bevy thiếu mobile/WASM**. Ngược lại, Bevy hiện có hỗ trợ những target đó. Lý do là workload của bạn chủ yếu là vài chục đến vài trăm card/piece/UI object, trong khi Bevy đưa vào tư duy ECS, scene, plugin, render graph, animation system và một general-purpose engine rộng hơn nhiều. Với board game, đó thường là complexity mà sản phẩm không cần. citeturn18search2

### Điểm mà nghiên cứu sâu làm thay đổi recommendation ban đầu

Ban đầu mình nghiêng về:

```text
Macroquad cho tất cả mọi thứ
```

Bây giờ mình sẽ chọn:

```text
                 PRODUCT UI
                    │
              Dioxus / DOM
                    │
       ┌────────────┴────────────┐
       │                         │
Lobby / Profile / Shop      Admin / CMS
Chat / History / Clubs      Analytics UI

                    +
                    │
               GAME CLIENT
                    │
                Macroquad
                    │
           Board / Card / FX
```

Dioxus 0.7 đáng chú ý vì nó không chỉ là một web framework Rust: release 0.7 hỗ trợ web/desktop/mobile, Android/iOS device tooling, mobile project customization, WASM code splitting và first-party primitives với keyboard/ARIA support. Với UI material-like, form, responsive layout, accessibility và chat, đây là abstraction tự nhiên hơn immediate-mode game UI. citeturn18search3

Điều mình **không** khuyên ở V1 là cố nhét Dioxus và Macroquad sâu vào cùng một screen trên cả ba platform. Hai framework nên chia theo boundary rõ ràng. Ví dụ portal web ở `app.example.com`, game ở route/client riêng nhưng chia sẻ auth và protocol. Native app có thể chỉ là Macroquad client với UI riêng lúc đầu; admin/dashboard vẫn là Dioxus web.

### Một khả năng còn nhẹ hơn: không dùng game engine cho game đầu

Cờ:

```html
CSS Grid / SVG
```

Thẻ bài:

```html
DOM + transform + transition
```

Ma sói:

```html
Cards + avatars + buttons + dialogs + chat
```

Các game này về bản chất có thể là application UI chứ không nhất thiết là continuous-rendering games. Dioxus web compile Rust thành WASM và Dioxus 0.7 nhắm tới cùng codebase cho web/mobile; do đó **Dioxus-only là một MVP hoàn toàn hợp lý nếu animation không phải USP**. citeturn18search3turn18search5

Nhưng với ưu tiên bạn đã nêu là WASM + native graphics + muốn sau này có nhiều loại board game, mình vẫn chọn:

> **Macroquad cho bàn chơi, Dioxus cho phần “SaaS xung quanh game”.**

### Vì sao chưa đi thẳng Miniquad

Miniquad có đúng những platform bạn cần và triết lý rất hấp dẫn: abstraction GPU càng nhẹ càng tốt, ít dependency, chạy trên dải hardware rộng; WASM/WebGL được project nêu là đã test trên iOS Safari, Firefox và Chrome, cùng Android GLES và iOS Metal/GLES. citeturn18search1

Nhưng đi thẳng Miniquad đồng nghĩa bạn bắt đầu sở hữu thêm:

```text
sprite abstraction
atlas
font/text
camera
batching conventions
asset lifecycle
UI widgets
animation
layout
input normalization
scene traversal
```

Đó là khoản “engine tax” không nên trả trước khi có bằng chứng Macroquad thực sự cản trở bạn.

**Điểm chuyển hợp lý** là khi:

```text
Game #1 shipped
Game #2 shipped
Game #3 shipped

↓ bạn phát hiện 70–80% rendering runtime giống nhau

boardgame-render-api
      ↓
custom implementation
      ↓
Miniquad
```

Lúc đó Miniquad trở thành một dependency tuyệt vời. Trước đó, nó dễ biến bạn từ “người làm board game” thành “người làm engine”.


## Kiến trúc Rust full-stack cho multiplayer board game

### Domain core phải độc lập tuyệt đối

Đây là quyết định quan trọng nhất của toàn project:

```rust
pub trait Game {
    type State;
    type Action;
    type PlayerView;
    type Error;

    fn apply(
        state: &Self::State,
        actor: PlayerId,
        action: Self::Action,
        rng: &mut impl GameRng,
    ) -> Result<Self::State, Self::Error>;

    fn view_for(
        state: &Self::State,
        player: PlayerId,
    ) -> Self::PlayerView;
}
```

Không nên có bất kỳ thứ gì như:

```rust
macroquad::Texture2D
axum::WebSocket
sqlx::PgPool
dioxus::prelude::*
```

trong `boardgame-core`.

Workspace nên bắt đầu gần như:

```text
workspace/
├── crates/
│   ├── protocol/
│   ├── boardgame-core/
│   ├── boardgame-testkit/
│   │
│   ├── games/
│   │   ├── chess/
│   │   ├── cards-demo/
│   │   └── werewolf/
│   │
│   ├── game-client/
│   │   ├── scene/
│   │   └── macroquad/
│   │
│   ├── server/
│   ├── persistence/
│   └── web-ui/
│
└── apps/
    ├── game-client/
    ├── server/
    └── portal/
```

Lợi ích chiến lược là cùng một rules implementation có thể chạy:

```text
Browser WASM
Android
iOS
Server
CLI tests
Replay validator
Simulation / bot
```

### Server phải authoritative

Đừng để client gửi:

```json
{
  "new_state": "..."
}
```

Hãy để nó gửi:

```json
{
  "game_id": "...",
  "expected_seq": 148,
  "action": {
    "type": "play_card",
    "card_id": 17,
    "target": 3
  },
  "idempotency_key": "..."
}
```

Server:

```text
Receive Action
     ↓
authenticate player
     ↓
check game/version
     ↓
boardgame-core.validate()
     ↓
server RNG if needed
     ↓
apply action
     ↓
append event
     ↓
snapshot periodically
     ↓
derive PlayerView
     ↓
broadcast
```

Đối với turn-based game, đây vừa giải quyết cheat, vừa tạo replay/audit trail và reconnect rất tự nhiên.

### Hidden information phải được model hóa ngay từ core

Sai:

```rust
struct ClientGameState {
    hands: Vec<Vec<Card>>,
}
```

rồi hy vọng UI không render bài đối thủ.

Đúng:

```rust
enum HandView {
    Mine(Vec<Card>),
    Opponent { count: usize },
}
```

Server phải tạo:

```rust
state.view_for(player_id)
```

trước khi serialize.

Đối với Ma Sói:

```text
Server State
├── Alice: Seer
├── Bob: Werewolf
├── Carol: Villager
└── ...

AliceView
├── Alice knows Alice role
└── no forbidden information

WerewolfView
├── own role
└── fellow wolves according to rules
```

**Secret không được gửi xuống client rồi “hide bằng CSS”.**

### RNG cũng phải authoritative và replayable

Các game thẻ bài cần:

```text
seed
+
ordered action stream
=
replayable match
```

Không nhất thiết public seed trong trận, vì có thể làm lộ deck order. Server giữ seed/state RNG, event log lưu đủ dữ liệu để audit hoặc reconstruct sau đó.

Một event stream đơn giản:

```text
GameCreated
PlayerJoined
GameStarted
DeckShuffled
CardPlayed
VoteSubmitted
PhaseAdvanced
PlayerEliminated
GameEnded
```

Không cần áp dụng event sourcing giáo điều cho toàn product. Chỉ cần coi **game match là event log + snapshots**, còn account/shop/catalog vẫn là relational CRUD thông thường.

### Backend stack

Mình chọn:

```text
Tokio
  ↓
Axum
  ├── HTTP
  └── WebSocket

SQLx
  ↓
PostgreSQL
```

Tokio runtime cung cấp async I/O driver, scheduler, timer và blocking pool; Axum có WebSocket extractor và hỗ trợ split read/write stream để xử lý song song; SQLx có `query!` macros kiểm tra query/schema/type ở build time nếu CI cung cấp database metadata hoặc offline metadata. citeturn19search2turn19search0turn19search1

HTTP phù hợp với:

```text
/login
/me
/games
/match-history
/shop
/leaderboard
/admin
```

WebSocket phù hợp với:

```text
room events
game actions
chat
presence
timer synchronization
reconnect stream
spectator stream
```

Không cần GraphQL ở giai đoạn này.

### Một Tokio task cho một active room là pattern rất tự nhiên

```text
Room 123
   ↓
mpsc<Action>
   ↓
single owner task
   ↓
GameState
```

Thay vì nhiều request cùng mutex vào state:

```text
WS 1 ─┐
WS 2 ─┼──► room channel ─► RoomTask ─► State
WS 3 ─┘
```

Tokio tasks là lightweight asynchronous units do runtime schedule, nên mô hình một task/actor-like owner cho active match phù hợp về mặt kỹ thuật. citeturn19search11

Điểm hay là game action trở thành **serialized by design**:

```text
seq 101
seq 102
seq 103
```

giảm hẳn race condition kiểu hai người cùng end-turn hoặc timeout trùng với action.

### Protocol cần sequence và reconnect ngay từ V1

Client lưu:

```text
last_seen_seq = 421
```

Reconnect:

```text
Client:
"I have 421"

Server:
events 422..430
```

Nếu gap quá lớn:

```text
snapshot at 400
+
events 401..430
```

Với board game, cách này đáng tin cậy hơn cố sync arbitrary mutable object graph.

### PostgreSQL đủ lâu hơn bạn nghĩ

Schema tối thiểu:

```text
users
profiles

rooms
room_members

matches
match_players
match_events
match_snapshots

friendships
clubs

chat_channels
chat_messages
reports
moderation_actions

products
entitlements
transactions
```

SQLx hỗ trợ PostgreSQL và compile-time query checking cho `query!`, vì vậy PostgreSQL + SQLx là pairing rất tự nhiên cho backend Rust này. citeturn19search1turn19search7

Mình **không thêm Redis vào ngày đầu**. Chỉ thêm khi có nhu cầu rõ:

```text
multi-instance presence
distributed room routing
very hot ephemeral cache
rate limiting at scale
pub/sub across processes
```

Và cũng không tách:

```text
auth-service
room-service
game-service
chat-service
leaderboard-service
...
```

ở V1.

Hãy deploy **modular monolith** trước.


## DevOps, vận hành và economics thực tế

### Hạ tầng ban đầu nên cực kỳ boring

Mình sẽ deploy:

```text
                 CDN
                  │
        ┌─────────▼──────────┐
        │ WASM + static asset│
        └────────────────────┘

Browser / Mobile
       │
   HTTPS/WSS
       │
┌──────▼─────────────┐
│ Rust application   │
│ Axum + Tokio       │
└──────┬─────────────┘
       │
┌──────▼──────┐
│ PostgreSQL  │
│ managed     │
└─────────────┘

Object Storage
    │
    ├── card art
    ├── avatars
    ├── downloadable packs
    └── replay/archive
```

Board game không cần 30 hoặc 60 server ticks/second như action game. Hầu hết network traffic là discrete actions, timer events, chat và presence. Vì vậy, **ở giai đoạn đầu bottleneck nhiều khả năng là acquisition, moderation, database correctness và support hơn CPU simulation**. Đây là suy luận kiến trúc từ đặc tính turn-based/social games, không phải benchmark capacity cụ thể.

### CI matrix

Mỗi PR:

```text
cargo fmt --check
cargo clippy
cargo test
core deterministic tests
protocol compatibility tests
SQLx checks

wasm32 build
Linux build
Android build smoke-test
```

Release branch:

```text
+ browser e2e
+ Android packaging
+ macOS runner → iOS build
+ migration tests
+ replay compatibility tests
```

Đừng chỉ test rằng renderer compile. Critical regression của platform board game thường nằm ở:

```text
rule edge case
secret information leak
double action
timeout race
reconnect
old replay incompatible
migration corruption
```

### Test quan trọng hơn graphics benchmark

`boardgame-core` nên có property/simulation tests kiểu:

```rust
for seed in 0..100_000 {
    let mut game = Game::new(seed);

    while !game.is_finished() {
        let actions = game.legal_actions();
        let action = choose(actions, seed);

        game.apply(action)?;

        assert!(game.invariants_hold());
    }
}
```

Cho card game:

```text
cards in deck
+ cards in hands
+ cards discarded
+ cards in play
=
total card count
```

Cho chess-like:

```text
illegal action cannot mutate state
```

Cho Werewolf:

```text
dead player cannot vote
phase transitions preserve roles
unauthorized PlayerView never contains hidden roles
```

Đây là một trong các lợi thế lớn nhất của việc tách rules thành pure Rust.

### Observability nên xoay quanh match, không chỉ HTTP

Mỗi event/log nên có:

```text
trace_id
match_id
room_id
player_id
connection_id
action_seq
server_instance
rules_version
```

Metrics nên theo dõi:

```text
active_rooms
active_ws
room_create_rate
match_start_rate
match_complete_rate

action_latency
db_latency
ws_disconnect_rate
reconnect_success

invalid_action_rate
duplicate_action_rate
timeout_rate

reports_per_1000_sessions
mute/block rate
```

Product metrics:

```text
room_created → match_started
invite_sent → friend_joined
match_started → match_finished
first game → second game
D1 / D7 / D30 retention
games per active group
```

Đối với social board game, **“bao nhiêu room có đủ người để bắt đầu?” có thể quan trọng hơn raw MAU**.

### Kubernetes chưa giải quyết vấn đề nào ở đây

MVP:

```text
1–3 Rust app instances
managed Postgres
CDN/object storage
```

là đủ architecture-wise.

Khi scale:

```text
Load Balancer
     │
 ┌───┼────┐
 │   │    │
 A   B    C
 │   │    │
 └── Postgres
```

Sau này mới thêm room ownership/distributed presence:

```text
room 100 → instance B
room 101 → instance A
```

Redis/NATS có thể xuất hiện ở đây, nhưng không nên thiết kế product quanh chúng trước khi có traffic.

### Mobile release chính là một DevOps stream riêng

Có một chi tiết rất sát thời điểm hiện tại: **từ ngày 31/08/2026**, tức chỉ ba ngày sau ngày hiện tại 28/08/2026, Google Play yêu cầu app mới và app update mobile phải target **Android 16 / API level 36 trở lên**, với một số ngoại lệ cho form factor khác. Vì vậy pipeline Android bạn thiết kế bây giờ nên target API 36 ngay từ đầu thay vì API 35. citeturn23search0

Đây cũng là lý do mình không coi “engine hỗ trợ Android” là xong việc. Release engineering còn phải duy trì:

```text
target SDK
NDK
signing
AAB
store metadata
privacy declaration
billing SDK
push notification
crash symbol
mobile QA
```

### Chat/moderation có thể tốn công hơn game engine

Điều này đặc biệt đúng cho Ma Sói. Apple đã cập nhật App Review Guidelines ngày **06/02/2026** để làm rõ rằng random/anonymous chat thuộc Guideline 1.2 về User-Generated Content. Guideline hiện tại yêu cầu social/UGC apps có cơ chế bảo vệ chống abuse. citeturn20search1turn20search4

Google Play cũng vừa có policy hiệu lực **26/08/2026**, chỉ hai ngày trước thời điểm nghiên cứu này, mở rộng Child Safety Standards và age-restricted rules cho anonymous/random-chat apps. Chính sách hiện yêu cầu các ứng dụng thuộc nhóm social/random/anonymous chat có published safety standards, in-app reporting và quy trình child-safety compliance. citeturn23search1turn23search7

Do đó nếu làm public matchmaking cho Werewolf, moderation phải được coi là **core backend domain**:

```text
block
mute
report player
report message
kick
room host moderation
rate limit
spam detection
ban
appeal
moderator audit trail
age controls
```

Không phải feature “để làm sau khi có user”.

Mình thậm chí sẽ trì hoãn public voice chat. Text chat đã có đủ moderation surface; voice đưa thêm recording/privacy/harassment/moderation complexity mà game chưa chắc cần.


## Bản quyền, licensing, monetization và pháp lý Việt Nam

### “Luật chơi không có copyright” không đồng nghĩa “clone game nào cũng được”

U.S. Copyright Office nói khá rõ: **ý tưởng về game, tên/title và phương thức chơi không được copyright bảo hộ theo luật copyright Hoa Kỳ**, nhưng text diễn giải luật, graphic art, board artwork và các biểu đạt sáng tạo khác có thể được bảo hộ. citeturn20search0turn20search3

Điều này có nghĩa một mechanic kiểu:

```text
move pieces on grid
draft a card
hidden roles
majority vote
```

không đồng nghĩa với quyền sử dụng:

```text
commercial game name
logo
character names
card wording
illustrations
board artwork
iconography
rulebook text
music
publisher assets
```

Hơn nữa còn có trademark, trade dress, contract, patent hoặc luật cạnh tranh không lành mạnh tùy jurisdiction. Vì thế đừng dùng câu “game rules không copyright được” làm cơ sở pháp lý duy nhất để clone một sản phẩm thương mại. Đây là phần nên có counsel chuyên IP trước khi launch.

### Ba mức rủi ro IP

**Thấp nhất: original game**

```text
your mechanics
your name
your art
your characters
your copy
your code
```

Đây là con đường mình khuyến nghị cho card game đầu tiên.

**Khá thấp: classic/public-domain-inspired**

```text
chess
checkers
go
traditional card rules
```

Nhưng vẫn dùng brand, UI, artwork và rule text của chính bạn; kiểm tra trademark trước khi đặt tên commercial product.

**Cao: adaptation của board game thương mại đang bán**

```text
Carcassonne
Azul
Ticket to Ride
Catan
...
```

Ở đây hãy nghĩ “digital publishing deal”, không phải “implementation of rules”.

BGA là một chỉ dấu tốt cho practice của ngành: tài liệu developer của họ yêu cầu publisher/designer approval trước khi một implementation có thể đi tới production/alpha đối với game có quyền liên quan, và lưu ý artwork của publisher vẫn thuộc copyright của chủ sở hữu chứ không tự động trở thành open-source asset chỉ vì code implementation có thể được chia sẻ. citeturn7search2turn7search10turn7search14

### Một hợp đồng licensing digital board game cần định nghĩa ít nhất

```text
IP/game title
digital adaptation right
territory
languages
platforms:
    web
    Android
    iOS
    desktop

term
exclusivity / non-exclusivity
publisher approval process

art asset rights
music
fonts
translations

price authority
subscription treatment
premium/free positioning

gross revenue definition
store fees
taxes
refunds
revenue share

DLC / expansions
cross-play
online multiplayer
tournaments
streaming/esports

analytics/data
marketing assets
termination
post-termination player access
```

Đặc biệt đừng ký “20% revenue share” mà không định nghĩa 20% của:

```text
gross customer spend?

hay

customer spend
- VAT
- Apple/Google
- refunds
- chargebacks
?
```

### Monetization phù hợp hơn pay-to-win

Với social/board games, mình sẽ thử:

```text
Free:
  public/basic rooms
  base games
  normal avatar

Premium host:
  custom room
  room themes
  advanced settings
  statistics
  tournament tools

Cosmetic:
  card backs
  table themes
  avatars
  emotes
  animations

Subscription:
  no ads
  premium catalog
  club tools
  advanced history
  analysis

Licensed game:
  premium entitlement
  or subscription revenue share
```

Plato công khai positioning của mình theo hướng mua style chứ không mua sức mạnh; dù đó là chiến lược riêng của Plato chứ không phải quy luật thị trường, nó là một reference tốt cho social board platform muốn tránh phá game balance. citeturn16search2

Mình sẽ tránh hoàn toàn:

```text
cash-out
tradable currency
player-to-player real-value items
wagering
real-money poker
```

Không chỉ vì product risk mà còn vì regulatory burden tăng đột biến.

### App-store economics phải được đưa vào P&L

Apple Developer Program hiện có mức enrollment **99 USD mỗi membership year**. Google Play không còn có một con số commission duy nhất cho mọi trường hợp: trang phí hiện tại nói 99% số developer phải chịu service fee đủ điều kiện cho mức 15% hoặc thấp hơn thông qua các chương trình khác nhau, và cơ chế phí đã thay đổi theo region/install/billing model trong năm 2026. Do đó financial model nên coi store fees là một **policy-driven variable**, không hard-code “Apple/Google đều lấy 30%”. citeturn23search2turn23search3

Nghĩa là backend billing nên lưu:

```text
gross_amount
tax
store
store_fee
refund
net_receipt
publisher_share
our_share
```

thay vì chỉ:

```text
price = 4.99
```

### Việt Nam là phần phải nghiên cứu trước khi public launch, không phải sau

Nghị định 147/2024/NĐ-CP có hiệu lực từ **25/12/2024** và điều chỉnh dịch vụ trò chơi điện tử trên mạng tại Việt Nam. Thông tin chính thức của Chính phủ nêu các cơ chế phát hành G1/G2/G3/G4; quy định về virtual item/currency/reward points; và yêu cầu thông tin đăng ký người chơi gồm họ tên, ngày sinh và số điện thoại di động Việt Nam. Với người dưới 16 tuổi, quy định nêu việc cha/mẹ hoặc người giám hộ đăng ký và quản lý. citeturn21search0

Nguồn Chính phủ cũng nêu virtual items, virtual currency và reward points chỉ được dùng trong game theo phạm vi đã khai báo, **không được đổi ngược thành tiền, gift card hoặc tài sản có giá trị giao dịch bên ngoài game, và không được mua bán giữa người chơi**. Đây là lý do rất mạnh để mô hình economy ban đầu chỉ có non-transferable cosmetics/entitlements. citeturn21search0

Cũng theo phần hướng dẫn chính thức về Nghị định 147, doanh nghiệp cung cấp dịch vụ phải lưu thông tin người chơi trong thời gian sử dụng dịch vụ và sáu tháng sau khi người chơi ngừng sử dụng; tuy nhiên **việc chính xác sản phẩm/công ty của bạn rơi vào loại giấy phép/thông báo và nghĩa vụ nào cần được luật sư Việt Nam xác định từ mô hình vận hành cụ thể**, nhất là nếu pháp nhân/server ở nước ngoài. citeturn21search0

Privacy landscape cũng mới thay đổi mạnh. Nghị định **356/2025/NĐ-CP**, quy định chi tiết một số điều và biện pháp thi hành Luật Bảo vệ dữ liệu cá nhân, có hiệu lực từ **01/01/2026**. Ngoài ra, chỉ ngày **19/08/2026**, Chính phủ đã ban hành thêm một loạt nghị định mới liên quan an ninh mạng và xử phạt về an ninh mạng/bảo vệ dữ liệu cá nhân, cho thấy đây là một regulatory surface đang thay đổi nhanh. citeturn21search1turn21search2

Vì vậy, trước beta commercial tại Việt Nam, checklist pháp lý nên bao gồm:

```text
game service classification
licensing / notification requirements
user identity requirements
under-16 flow

privacy notice
consent
retention
data processors
cross-border data

chat / moderation
abuse reporting

virtual item rules
payments
refunds

publisher IP licenses
terms of service
community guidelines
```

Đây không phải khu vực nên dựa hoàn toàn vào interpretation kỹ thuật; cần counsel Việt Nam kiểm tra bản pháp luật đang có hiệu lực tại ngày launch.


## Chiến lược sản phẩm và lộ trình mình đề xuất

### Đừng bắt đầu bằng “platform”

Sai lầm dễ mắc nhất là bắt đầu xây:

```text
Board Game Engine
Plugin SDK
Game DSL
Mod system
Marketplace
Editor
Dedicated asset pipeline
Custom UI framework
Matchmaking framework
Tournament framework
...
```

rồi 12 tháng sau chưa có game nào đủ vui.

Mình sẽ đi ngược lại:

```text
game
→ second game
→ third game
→ extract platform
```

### Giai đoạn đầu: chứng minh game core

Sản phẩm:

```text
1 chess/checkers-like
1 original card game
```

Tech:

```text
Rust workspace
boardgame-core
Macroquad
WASM
Axum
Postgres
WebSocket
```

Mục tiêu không phải traffic lớn mà chứng minh:

```text
join room
play
disconnect
reconnect
finish
replay
```

và kiểm chứng một game core chạy giống hệt trên server/client.

**Chưa cần**:

```text
Redis
Kubernetes
microservices
voice
UGC
plugin API
publisher SDK
```

### Giai đoạn tiếp: social deduction

Thêm Werewolf-like:

```text
8–16 players
roles
private state
phases
timers
vote
private/public chat
moderator
reports
reconnect
spectator restrictions
```

Wolvesville hiện quảng bá tối đa 16 người, hơn 100 role và web/iOS/Android cross-platform, nên đây là một benchmark thực tế tốt để xác định feature ceiling, dù MVP của bạn chỉ nên có số role nhỏ hơn nhiều. citeturn17search0

Đây chính là game sẽ stress-test platform thật sự.

### Giai đoạn sau: mới extract board-game runtime

Khi ba game chạy:

```text
chess
cards
werewolf
```

hãy quan sát phần lặp lại và mới tách:

```text
boardgame-runtime/
├── board/
├── cards/
├── zones/
├── drag/
├── animation/
├── timer/
├── hidden-info/
├── replay/
├── room/
└── multiplayer/
```

Lúc đó mới cân nhắc:

```text
Macroquad
   ↓
custom boardgame renderer
   ↓
Miniquad
```

Việc Miniquad hỗ trợ desktop, iOS, Android và WASM/WebGL với abstraction rất nhẹ khiến nó là foundation tốt cho **giai đoạn framework hóa**, nhưng không có lý do phải trả chi phí đó trước khi abstraction được chứng minh bằng game thật. citeturn18search1

### Giai đoạn thương mại: original IP trước, licensed catalog sau

Mình sẽ đi theo thứ tự:

```text
traditional/classic
       +
original IP
       ↓
prove retention
       ↓
prove room/community
       ↓
publisher conversations
       ↓
one licensed title
       ↓
measure economics
       ↓
catalog expansion
```

Không nên đi xin 20 license khi chưa biết:

```text
CAC
retention
ARPU
premium conversion
match completion
friend invites
```

Một publisher sẽ quan tâm hơn nhiều nếu bạn nói:

```text
"We have X monthly active groups,
Y% D30 retention,
Z matches/month,
and this is the revenue model"
```

thay vì:

```text
"Our engine is written in Rust."
```

### Định vị sản phẩm mình thấy có xác suất tốt nhất

Không phải:

> **“Nền tảng board game online tổng quát.”**

Mà là:

> **“Nơi một nhóm bạn có thể mở link và bắt đầu game night ngay trên web, sau đó tiếp tục trên mobile.”**

Tập trung:

```text
Vietnamese-first
SEA-friendly
browser-first
mobile-good
short sessions
private groups
social deduction
cards
light strategy
async + realtime
```

Sau đó mới mở rộng catalog.

BGA cho thấy cả realtime lẫn browser play là mô hình đã được người dùng chấp nhận ở quy mô lớn; Plato cho thấy social gaming có thể đạt hàng chục triệu monthly users; Wolvesville cho thấy một game social-deduction có thể tồn tại độc lập trên web và mobile. citeturn16search0turn16search2turn17search0

### Quyết định cuối cùng về stack

Nếu mình là tech lead của dự án này, ADR ban đầu sẽ viết:

```text
ADR-001: Game Rules
Decision:
Pure Rust deterministic domain core.
No renderer/network/database dependencies.

ADR-002: Game Renderer
Decision:
Macroquad.

Reason:
WASM + Android + iOS.
Lightweight.
2D-oriented.
Minimal abstraction.
Easy escape path to Miniquad.

ADR-003: Product Web UI
Decision:
Dioxus 0.7.

Reason:
Rust.
WASM.
HTML/CSS-style UI.
Responsive/application UI.
Accessibility.
Dashboard/lobby/account easier than canvas UI.

ADR-004: Backend
Decision:
Tokio + Axum + SQLx + PostgreSQL.

ADR-005: Multiplayer
Decision:
Server-authoritative Action protocol.
WebSocket realtime.
HTTP for ordinary API.

ADR-006: Persistence
Decision:
Event log + snapshots for matches.
Relational CRUD everywhere else.

ADR-007: Architecture
Decision:
Modular monolith.
No Kubernetes.
No microservices.
Redis optional later.

ADR-008: Mobile
Decision:
Macroquad native client for actual gameplay.
Android/iOS store release only after web product proves retention.

ADR-009: IP
Decision:
Original/public-domain-inspired titles first.
Commercial board game only after written digital license.

ADR-010: Social Safety
Decision:
Reporting/blocking/moderation/age policy
is MVP scope for public chat.
```

Macroquad đáp ứng chính xác portability requirement về HTML5/Android/iOS và 2D; Miniquad giữ cho bạn một đường đi xuống abstraction thấp hơn; Dioxus 0.7 giải quyết phần application UI mà game engine không làm tốt bằng; Tokio/Axum/SQLx là một stack Rust async/WebSocket/PostgreSQL trực tiếp và ít tầng trung gian. citeturn18search0turn18search1turn18search3turn19search0turn19search1turn19search2

**Kết luận quan trọng nhất:** lợi thế kiến trúc không nằm ở Macroquad hay Miniquad. Nó nằm ở chỗ:

```text
                        ┌───────────────┐
                        │ ONE RULE CORE │
                        │   Pure Rust   │
                        └───────┬───────┘
                                │
            ┌───────────────────┼───────────────────┐
            │                   │                   │
            ▼                   ▼                   ▼
        Web WASM            Native Mobile        Rust Server
            │                   │                   │
            └───────────────────┼───────────────────┘
                                │
                         exact same rules
```

Từ đó bạn có được **server-authoritative validation, deterministic testing, replay, bot simulation, web/mobile parity và khả năng thay renderer mà không phải viết lại game**.

Và xét cả thị trường lẫn engineering, mình sẽ ưu tiên ngân sách theo thứ tự:

```text
Game design
    >
social/join-room UX
    >
rules correctness
    >
reconnect
    >
moderation
    >
mobile usability
    >
content/IP
    >
renderer effects
    >
custom engine technology
```

Thị trường hiện tại cho thấy người dùng sẵn sàng chơi board game ở quy mô rất lớn trên browser, social gaming và app chuyên biệt; nhưng BGA, Tabletopia, Plato, Chess.com, Lichess và Wolvesville cũng cho thấy cạnh tranh đã đủ trưởng thành để **“có nhiều game” hoặc “dùng Rust” không phải một differentiation**. Distribution, social loop, IP/content và trải nghiệm “mời bạn vào bàn trong vài giây” mới có khả năng trở thành lợi thế sản phẩm. citeturn16search0turn16search1turn16search2turn17search0turn17search1turn17search5