//! # `tabula-game-tiles` — the state-and-camera benchmark
//!
//! > ## PHASE 3 (rules + presentation) → PHASE 9 (async polish)
//!
//! A Carcassonne-like tile-placement game: draw a tile, place it legally
//! adjacent, optionally place a follower, score completed features. It is here
//! to stress **large dynamic spatial state**, **secret deterministic RNG**,
//! **incremental graph scoring**, and **camera**, which chess, Caro, and
//! Werewolf all leave untested. (doc 08 §4)
//!
//! The tile distribution is Tabula's own, in the Carcassonne family. It is not
//! a reproduction of any published set, and nothing here depends on matching
//! one.
//!
//! ## Scope, as implemented (doc 08 §4.1)
//!
//! ```text
//! IN:  a 72-tile bag with a fixed distribution, four rotations, adjacency
//!      legality, a feature graph over roads / cities / monasteries, follower
//!      placement and return, incremental scoring on completion, end-of-game
//!      partial scoring, 2-5 seats, an optional per-turn deadline whose rules
//!      are identical for live and async play
//! OUT: farms/fields as a SCORABLE feature. `Terrain::Field` remains an edge
//!      terrain so adjacency matching is complete, but a field is not a
//!      feature and is never scored. Scoring farms needs sub-edge granularity
//!      (two field corners per tile side); it multiplies the graph's
//!      representation without exercising a contract that roads, cities, and
//!      monasteries do not already exercise.
//! OUT: expansions, rivers, custom boards, trading.
//! ```
//!
//! ## Module map
//!
//! ```text
//! rules::coord      Coord, Side, Rotation           — the coordinate system
//! rules::tile       Terrain, FeatureKind, TileKind, PlacedTile, Board, TILE_SET
//! rules::placement  pure placement legality over a Board
//! rules::feature    FeatureGraph — the incremental component structure, plus
//!                   the whole-board recomputation it is checked against
//! rules::scoring    the single authority for every point in the game
//! rules::secret     the information model, test/testkit builds only
//! rules::state      State, Command, Event, View, Config, and the validator
//! rules             impl GameRules for TilesRules
//! bot               a shallow policy over the projection; the fuzz driver
//! ```
//!
//! Dependencies point one way only: `coord → tile → placement → feature →
//! scoring → state → rules → (bot, presentation)`. Canonical rules never reach
//! back into `bot` or `presentation`.
//!
//! ## The contract lessons this game actually produced
//!
//! 1. **`state_size` is set from a measurement, not from the design estimate.**
//!    Doc 02 §12.4 expected `Medium` (30–120 KB). `tests/state_size.rs` plays
//!    complete matches at every seat count and measures the canonical
//!    encoding: **a full board is about 1.7 KB**, so the declared class is
//!    `Small`. The test asserts the declared class *is* the measured one, in
//!    both this file and `game.toml`, so the two cannot drift. The honest
//!    consequence — no game in the portfolio occupies `Medium` — is recorded
//!    in doc 03 §9.2 and `docs/games/tiles.md`.
//!
//! 2. **`legal_commands` returns `Hints` in the placement phase.** One hint per
//!    legal coordinate, carrying that coordinate's legal rotations — enough for
//!    the client to highlight without enumerating every `(position, rotation)`
//!    command. Writing it found that `CommandHint` was `#[non_exhaustive]` with
//!    public fields and therefore unconstructible outside `tabula-game-api`
//!    (E0639), which made `LegalCommands::Hints` returnable by no game at all.
//!    That is the one generic contract change this game required. In the meeple
//!    phase the same function returns `Enumerated` — the choice is per state,
//!    not per game.
//!
//! 3. **The feature graph is an incremental structure inside `State`**, which
//!    is why `apply` takes `&mut State` (doc 02 §3.3). It participates in the
//!    state hash by construction (it is a `State` field and `state_hash` is the
//!    default postcard hash), so a divergence in it is caught rather than
//!    silently accumulating. It is deliberately **not** a union-find — the one
//!    word of doc 02 §12.4's sketch that implementation overturned, because
//!    path compression mutates on read and `project` is a read path. See
//!    [`rules::feature`] for the four representations compared and why
//!    canonical-serialization safety decided it, and `tests/features.rs` for
//!    the whole-board recomputation kept as its differential oracle.
//!
//! 4. **The bag order is secret; the count is public.** Tiles declares
//!    `hidden_information = true`, implements
//!    [`tabula_testkit::projection::SecretModel`] marking the remaining order
//!    as authorised to **nobody**, and expands `projection_security!` alongside
//!    `conformance!`. Being the *secondary* hidden-information benchmark
//!    (Werewolf owns per-seat knowledge and event non-existence) buys Tiles no
//!    exemption from either obligation. Containment scanning alone is not
//!    adequate evidence for an *ordered* secret — a short remaining bag encodes
//!    to a token too small to be a leak detector — so the scan is paired with a
//!    noninterference property over bag permutations, which is the oracle that
//!    covers the whole range. (`tests/projection.rs`)
//!
//! 5. **The one evidence class that survives a code change** is a committed
//!    replay. The conformance suite, the replay property, and self-play all
//!    compare the current code against itself, so all three stay green through
//!    a rules change that silently alters historical behaviour.
//!    `tests/replays/tiles-golden.tbr` is a complete match — the shuffle, all
//!    71 draws, every merge, completion scoring, follower returns, end-of-game
//!    partial scoring, and the standings — with its final state hash committed
//!    as a literal in `tests/replay.rs` (doc 02 §11.4).
//!
//! 6. **Live and async are the same rules.** The per-turn deadline is a
//!    `Config` value in milliseconds; `0` disables it. Whether that value is
//!    60 s or 24 h changes nothing in `apply`, because `apply` only ever reads
//!    `ctx.now`. That is the payoff of `LogicalTime`, and it is why the async
//!    capability can be declared from Phase 3 rules rather than promised.
//!
//! Camera pan and zoom live entirely in `P::Local`, never in canonical state
//! (I-10). The property test that proves it drives one command sequence from
//! several camera positions and compares state hashes
//! (`tests/presentation.rs`). Camera *rotation* is not implemented and is not
//! representable: `tabula_presentation::Camera2D` carries an origin and a zoom
//! and nothing else (doc 08 §4.2).
//!
//! ## Deferred, and honestly so
//!
//! - Farms/fields as a scorable feature (above).
//! - `client_preview` is declared **false**. Doc 02 §7.2 defines it as "folding
//!   the `ViewEvent` stream onto a previous `View` lands on the same `View`
//!   that `project` returns", and the testkit has no oracle for that property
//!   yet. Tiles' `View` also carries feature-derived affordances (the legal
//!   meeple slots) that a client would have to re-derive rather than fold.
//!   Declaring `true` would be an unverified claim.
//! - **Wheel and pinch zoom.** `tabula_presentation::InputEvent` carries
//!   pointer, key, and focus events only. Zoom is on-screen instead. Adding a
//!   wheel variant would be a Phase-2 contract change with no second consumer
//!   asking for one, and the game is fully playable without it (doc 08 §4.3).
//! - **`apply` and `state_hash` wall-clock budgets.** Doc 08 §4.5 asks for
//!   them; `std::time::Instant` is a `disallowed-type` in a rules crate (I-3)
//!   and a timing assertion in the per-PR tier is a flake generator. They
//!   belong to the Phase-4 load test (doc 06 §2). `tests/state_size.rs`
//!   measures size, which is what can be measured here honestly.
//! - Board Reader (`describe`) beyond a status line: Phase 9, and it is what
//!   will shape `A11yRegion` (doc 04 §10.4).
//! - Async-turn *operations* — hibernation, push, surviving deploys — are Phase
//!   4+ platform work. Phase 3 owns only the rules half of that claim.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

// `rules` is the complete canonical module tree. Package, bot, and
// presentation code may depend on it, but canonical rules never reach back
// into those noncanonical sources.
pub mod rules;

// `cfg(test)` alongside the feature so `cargo test -p tabula-game-tiles` runs
// the self-play fuzzer: an integration test cannot enable its own crate's
// features, and self-play belongs with the rules rather than in the client.
#[cfg(any(test, feature = "bots"))]
pub mod bot;

#[cfg(feature = "presentation")]
pub mod presentation;

use std::sync::LazyLock;

use tabula_core::{BotLevel, Millis, SeatRoster};
use tabula_game_api::{
    capabilities::ChatKind, AssetRef, AsyncTurnPolicy, BotLevels, Budget, Category,
    ChatChannelSpec, ChatPolicy, Complexity, ConfigError, ContentRating, Durability, DurationRange,
    GameCapabilities, GameCapabilitiesSpec, GameId, GameMetadata, GameMetadataSpec, GameModule,
    GameRules, GameVersion, I18nKey, RankedSupport, RatingKind, ReconnectPolicy, SeatCounts,
    SeatSpec, SpectatorPolicy, StateSizeClass, SubstitutionPolicy, TurnModel, VoiceRequirement,
};

pub use rules::{
    Board, Command, Config, Coord, Event, Feature, FeatureGraph, FeatureId, FeatureKind,
    PlacedTile, Rotation, SegmentRef, Side, State, Status, Terrain, TileKind, TilesRules,
    TurnPhase, View, ViewEvent,
};

/// The game package around [`TilesRules`].
#[derive(Debug)]
pub struct TilesModule;

impl GameModule for TilesModule {
    type Rules = TilesRules;

    fn metadata() -> &'static GameMetadata {
        &METADATA
    }

    fn capabilities() -> &'static GameCapabilities {
        &CAPABILITIES
    }

    #[cfg(any(test, feature = "bots"))]
    fn bot(level: BotLevel) -> Option<Box<dyn tabula_game_api::GameBot<TilesRules>>> {
        match level {
            BotLevel::Trivial | BotLevel::Easy => Some(Box::new(bot::Greedy::new(level))),
            BotLevel::Medium | BotLevel::Hard => None,
        }
    }

    fn validate_config(cfg: &Config, roster: &SeatRoster) -> Result<(), ConfigError> {
        // Fail here, at match creation, rather than mid-match.
        let seats = u8::try_from(roster.len()).map_err(|_| ConfigError::SeatCount)?;
        if !(rules::MIN_SEATS..=rules::MAX_SEATS).contains(&seats) {
            return Err(ConfigError::SeatCount);
        }
        if rules::turn_deadline(cfg).is_err() {
            return Err(ConfigError::field("turn_deadline_ms"));
        }
        Ok(())
    }
}

static METADATA: LazyLock<GameMetadata> = LazyLock::new(|| {
    GameMetadata::from(GameMetadataSpec {
        id: GameId::new("com.tabula.tiles").expect("literal is a valid game id"),
        version: GameVersion::new("0.1.0").expect("literal is valid SemVer"),
        rules_version: TilesRules::RULES_VERSION,
        name_key: I18nKey::new("game.tiles.name").expect("literal is a valid i18n key"),
        tagline_key: I18nKey::new("game.tiles.tagline").expect("literal is a valid i18n key"),
        description_key: I18nKey::new("game.tiles.description")
            .expect("literal is a valid i18n key"),
        categories: vec![Category::TilePlacement],
        tags: vec!["placement".to_owned(), "family".to_owned()],
        estimated_minutes: DurationRange::new(30, 60).expect("literal range is ordered"),
        complexity: Complexity::Medium,
        content_rating: ContentRating::Everyone,
        icon: AssetRef::new("tiles/icon").expect("literal is a valid asset reference"),
        hero: AssetRef::new("tiles/hero").expect("literal is a valid asset reference"),
        rules_url_key: None,
    })
});

static CAPABILITIES: LazyLock<GameCapabilities> = LazyLock::new(|| {
    GameCapabilities::try_from(GameCapabilitiesSpec {
        seats: SeatSpec::new(
            SeatCounts::range(rules::MIN_SEATS, rules::MAX_SEATS).expect("literal range is valid"),
            None,
            true,
            true,
        ),
        turn_model: TurnModel::StrictSequential,
        // The remaining bag order determines every future draw (doc 02 §12.4).
        hidden_information: true,
        spectators: SpectatorPolicy::Live,
        chat: ChatPolicy::new(
            vec![ChatChannelSpec::new("table", ChatKind::Table)
                .expect("literal is a valid chat channel")],
            false,
        )
        .expect("tiles chat channels are unique"),
        voice: VoiceRequirement::Optional,
        ranked: RankedSupport::Yes {
            rating: RatingKind::Placement,
        },
        // Declarable from Phase 3 because the rules read only `ctx.now`: the
        // per-turn deadline is a Config value and 60 s and 24 h take the same
        // code path. The async *operations* (hibernation, push) are Phase 4+.
        async_turns: AsyncTurnPolicy::Enabled {
            turn_deadline: Some(Millis(rules::DEFAULT_ASYNC_TURN_DEADLINE_MS)),
            match_ttl: None,
        },
        reconnect: ReconnectPolicy {
            grace: Millis(60_000),
            notify_rules: false,
        },
        substitution: SubstitutionPolicy::BotOnly {
            levels: BotLevels::new(vec![BotLevel::Trivial, BotLevel::Easy])
                .expect("literal levels are non-empty and unique"),
        },
        pausable: true,
        // `ranked` implies this (GameCapabilitiesError::RankedNeedsDurability).
        durability: Durability::AckAfterPersist,
        // False deliberately — see the "Deferred, and honestly so" note above.
        client_preview: false,
        // Set from `tests/state_size.rs`, not from the design estimate.
        state_size: StateSizeClass::Small,
        apply_budget: Budget {
            max_apply_micros: 2_000,
            max_events_per_input: 16,
        },
        max_match_duration: None,
    })
    .expect("tiles capabilities are coherent")
});
