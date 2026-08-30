//! # `tabula-lobby` — rooms, matchmaking, presence
//!
//! > ## PHASE 5
//! >
//! > Doc 01 §3 recommends this starts life as a **module inside `tabula-match`**
//! > in Phase 4 and is extracted here in Phase 5. Creating it early invites
//! > premature interface design for product logic that has not been written yet.
//!
//! Different change cadence and different scaling profile from the match runtime.
//! It is also where most product and business logic will accrete — which is
//! exactly why it must not be able to touch game state.
//!
//! **Forbidden: `tabula-game-api`.** The lobby reasons about `GameCapabilities`,
//! never about rules (ADR-023). That restriction is what keeps matchmaking
//! generic across every game we will ever ship.
//!
//! ## Room vs Match — the distinction to get right once (doc 03 §14.1)
//!
//! ```text
//! Room   a SOCIAL container. Persists across matches. Owns invites, settings,
//!        chat history, and a seat list. "Play again" reuses the room and
//!        creates a new match.
//! Match  ONE instance of play. Owns state, event log, outcome, replay.
//! ```
//!
//! ```rust,ignore
//! pub struct Room {
//!     pub id: RoomId,
//!     pub game: GameId,
//!     pub config: RawValue,              // validated by the module at creation
//!     pub visibility: Visibility,        // Public | Unlisted | FriendsOnly | Private(code)
//!     pub seats: Vec<RoomSeat>,          // reservations, ready flags, teams
//!     pub owner: UserId,
//!     pub current_match: Option<MatchId>,
//!     pub settings: RoomSettings,        // rematch policy, spectators, voice
//! }
//! ```
//!
//! ## Matchmaking reads capabilities and nothing else (doc 03 §15)
//!
//! ```text
//! from GameCapabilities: seats.min/max/allowed, seats.teams, seats.symmetric,
//!                        seats.fill_with_bots, ranked.rating, async_turns
//! from the queue entry:  user, game_id, config_key (hash of normalized config),
//!                        rating, region, latency hint, party members, enqueued_at
//! ```
//!
//! ```text
//! Buckets keyed by (game_id, config_key, region).
//! Within a bucket, entries sorted by enqueued_at.
//! Every 500 ms, for each bucket:
//!    widen = f(waited)                    # rating window grows with wait time
//!    greedily form the largest legal seat count from compatible entries
//!    if a group forms:  reserve seats, create match, notify
//!    if waited > fill_after and seats.fill_with_bots: fill with bots
//!    if waited > give_up: notify "no match found", offer a bot game or room browse
//! ```
//!
//! `widen`, `fill_after`, and `give_up` are **per-game configuration rows, not
//! code**. The moment they become code, tuning matchmaking needs a deploy and
//! I-9 starts to erode.
//!
//! **The matchmaker never reads game state and never links a game crate.**
//!
//! ## Presence: in-process truth, debounced durability (doc 03 §14.2)
//!
//! `DashMap<UserId, PresenceState>`, updated by session attach/detach.
//! Write-through to Postgres **on transitions only**, 5 s debounce. Fan-out is
//! coalesced by a single broadcaster task flushing diffs every **500 ms**.
//!
//! Write the coalescing on day one. Presence is the classic O(friends × events)
//! fan-out disaster, and adding coalescing after it melts is a rewrite.
//!
//! Presence is not durable truth — it rebuilds from live sessions on restart.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/ports.rs      RoomRepo, QueueStore, PresenceStore, RatingRepo, MatchLauncher
//! src/room.rs       Room, RoomSeat, RoomSettings, Visibility   (doc 03 §14.1)
//! src/invite.rs     invitations, seat reservation
//! src/queue.rs      queue entries, bucketing                   (doc 03 §15.1)
//! src/matchmaker.rs the 500 ms round: widen / fill / give_up
//! src/presence.rs   presence map + the coalescing broadcaster  (doc 03 §14.2)
//! src/topics.rs     LobbyTopic, delta publishing (room list capped at 200)
//! src/launcher.rs   match creation orchestration
//! ```

#![forbid(unsafe_code)]
