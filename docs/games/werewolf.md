# Werewolf (The Social and Scale Benchmark)

> **Status: PHASE 3 (rules, headless) → PHASE 7 (presentation + online) → PHASE 8 (voice).**
> Architectural role: [doc 08 §5](../architecture/08-first-games-validation-plan.md).
> Implementation sequence: [`todos/werewolf/`](../../todos/werewolf/).

---

## 1. Architectural role

Werewolf is Tabula's primary **hidden-information, phased-rules, and social-scale benchmark**.
It validates three platform capabilities that no other reference game requires:

1. **Event non-existence:** unauthorized viewers must observe `view_event -> None` rather than
   `Some(Redacted)`. Even knowing that an event occurred leaks who acted during private phases.
2. **Game-driven communication scoping:** the rules emit absolute `ChatScopes` and `VoiceScopes`
   as pure data (`Effect::SetChatScopes`, `Effect::SetVoiceScopes`). The imperative shell enforces
   them at the socket and SFU without the platform ever learning what "Werewolf" is.
3. **Many seats with simultaneous action:** 6–20 seats acting in parallel during fixed-duration
   phases (`TurnModel::Phased`) rather than taking sequential turns.

---

## 2. Phase boundary and roadmap

| Phase | Werewolf deliverables | Gate / dependencies |
|---|---|---|
| **Phase 3** | Pure deterministic reducer, fixed timer windows, RNG role assignment, projections, event non-existence, scope values, SecretModel, tests, simulation, canonical replay, terminal projection viewer. **No UI, no networking, no audio.** | Phase 2 exit; doc 07 Phase 3 |
| **Phase 4** | Generic network envelope/routing/security decisions required before protocol freeze (state version/ack metadata ADR). No Werewolf-specific server branches. | Phase 3 exit; doc 07 Phase 4 |
| **Phase 7** | Werewolf presentation, online social UX, chat enforcement, moderation, 12-human playtest. | Phases 4 and 5; doc 07 Phase 7 |
| **Phase 8** | Voice provider, publish/listen enforcement, SFU/TURN integration. | Phase 7 exit; ADR-016 |
| **Phase 9+** | Spectator replay product, advanced roles (Cupid/lovers, Jester, Alpha), custom rulesets, moderator mode. | Separate product scope |

A headless text projection viewer is Phase-3 verification tooling, not `GamePresentation` or a local social client.

---

## 3. Rules summary (Phase 3)

Werewolf is played with 6 to 20 seats divided into two secret factions: the **Village** and the **Werewolves**.

The match progresses through five fixed-duration phases per round:
```text
Night  →  Dawn  →  Day discussion  →  Vote  →  Dusk
  ↑                                              │
  └──────────────── Next round ──────────────────┘
```

- **Night:** living players with active roles submit private actions or pass.
- **Dawn:** simultaneous night resolution; Seer receives alignment report; public deaths are announced (seat and role revealed, cause hidden).
- **Day discussion:** living players discuss publicly.
- **Vote:** living players cast public, replaceable ballots or abstain.
- **Dusk:** vote resolution; tied maximum yields no elimination; elimination deaths announced; Hunter retaliates if killed; victory check. If no team has won and `max_rounds` is reached, the match ends in a stalemate draw; otherwise round increments to Night.

---

## 4. ClassicV1 preset family (W-D1, W-D2)

Phase 3 adopts a single named preset family: `ClassicV1`. The role multiset is strictly determined
by the number of occupied seats `n` in `6..=20`:

$$\text{Villagers} = n - \text{Werewolves} - \text{Seer} - \text{Doctor} - \text{Hunter} - \text{Witch}$$

### The authoritative ClassicV1 distribution table

| Seats ($n$) | Werewolves ($W$) | Seer ($S$) | Doctor ($D$) | Hunter ($H$) | Witch ($T$) | Villagers ($V$) |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **6** | 1 | 1 | 1 | 0 | 0 | 3 |
| **7** | 1 | 1 | 1 | 0 | 0 | 4 |
| **8** | 2 | 1 | 1 | 1 | 0 | 3 |
| **9** | 2 | 1 | 1 | 1 | 0 | 4 |
| **10** | 2 | 1 | 1 | 1 | 1 | 4 |
| **11** | 2 | 1 | 1 | 1 | 1 | 5 |
| **12** | 3 | 1 | 1 | 1 | 1 | 5 |
| **13** | 3 | 1 | 1 | 1 | 1 | 6 |
| **14** | 3 | 1 | 1 | 1 | 1 | 7 |
| **15** | 3 | 1 | 1 | 1 | 1 | 8 |
| **16** | 4 | 1 | 1 | 1 | 1 | 8 |
| **17** | 4 | 1 | 1 | 1 | 1 | 9 |
| **18** | 4 | 1 | 1 | 1 | 1 | 10 |
| **19** | 4 | 1 | 1 | 1 | 1 | 11 |
| **20** | 5 | 1 | 1 | 1 | 1 | 11 |

### Assignment semantics (W-D2)
At match creation:
1. The roster must contain 6–20 unique, occupied seats (`Occupant != Empty`), with no pre-assigned teams (`team == None`).
2. Seats are sorted ascending by `SeatId`.
3. The exact multiset from the table is shuffled using `ctx.rng.stream(DOMAIN_ROLES)`.
4. Shuffled roles are paired 1-to-1 with the sorted seats.

---

## 5. Role mechanics locked in W1

Phase 3 defines exactly six base roles:

```text
Villager   Werewolf   Seer   Doctor   Hunter   Witch
```

### Werewolf (`Alignment::Wolf`)
- Living werewolves know the identities of their teammates.
- Each living werewolf submits one living non-wolf target or passes during Night.
- Wolves' night submissions are private to each wolf until resolution (a wolf does not see teammates' live target choices before phase closure).
- **Consensus:** the unique positive plurality of wolf choices attacks that target. Ties or unanimous passes result in no wolf attack.

### Seer (`Alignment::Village`)
- Investigates one other living seat during Night.
- Receives a private alignment report (`Wolf` or `NonWolf`/`Village`) at Dawn.
- The investigation report is delivered even if the target or the Seer dies in that resolution batch.

### Doctor (`Alignment::Village`, W-D3)
- Submits one protection target or passes each Night.
- **Self-save allowed:** the Doctor may target their own seat.
- **No consecutive repeat saves:** the Doctor cannot protect the same seat on consecutive nights. Passing clears the previous-night target.
- **Wolf attack only:** protection cancels a werewolf attack against the protected seat; it provides no protection against Witch poison.

### Witch (`Alignment::Village`, W-D4)
- Holds exactly **one heal potion** and **one poison potion** for the entire match.
- May use **at most one potion per Night**, or pass.
- **Blind heal:** the Witch may heal any living seat (including self) against wolf attack, but receives **no private preview** of who the wolves targeted. (A victim-informed preview would leak wolf action existence through the Witch's client).
- **Poison:** targets any living seat and kills them at Dawn independently of Doctor protection or heal potions.
- Potions are consumed upon submission, even if ineffective (e.g. healing a seat that was not attacked).

### Hunter (`Alignment::Village`, W-D5)
- **Precommitted retaliation:** the Hunter selects an optional precommitted target during Night, which remains active through that round's Vote.
- If the Hunter dies (by wolves, poison, or elimination vote), the Hunter's precommitted target is killed if still living.
- Triggers once per match before the final victory check.
- **Why precommit instead of reactive shot:** dead players in Tabula immediately acquire full vision (`Viewer::Seat` sees all roles). A reactive shot after death would allow dead players to act with omniscient knowledge, functioning as a cheating oracle. Precommitting before death completely eliminates this vector.

### Villager (`Alignment::Village`)
- Has no private night actions. Participates in day discussions and voting.

---

## 6. Voting, ties, and abstention (W-D6, W-D7)

- **Ballots:** living seats may submit `Vote(target)` or `Vote(Abstain)` or `Unvote`. Ballots are public and replaceable until the Vote phase deadline.
- **Vote mode:** configurable as `VoteMode::Plurality` (default) or `VoteMode::AbsoluteMajority`.
- **Threshold:**
  - `Plurality`: the unique highest positive ballot count is eliminated.
  - `AbsoluteMajority`: requires `count > living_count / 2`, where `living_count` includes all living seats (even those abstaining).
- **Tied maximum:** a tie for highest vote count results in **no elimination** in both modes.
- **Abstention:** uncast ballots at timer expiry default to abstain. Abstain is not a candidate for elimination.
- **Fixed timer closure:** the vote phase never closes early (even if all players have voted or an absolute majority is mathematically unreachable). Early closure would leak participation timing.

---

## 7. Disconnect and seat lifecycle (W-D8)

- Phase timers and transitions continue uninterrupted regardless of player connection state.
- Disconnected or idle seats retain previously submitted choices. Missing choices default to pass or abstain at deadline expiry.
- A reconnecting player may act if the phase deadline has not yet expired.
- Permanent absence (`Abandoned` or `Vacated`) marks the seat permanently absent; missing actions default to pass/abstain.
- **No substitution:** `SubstitutionPolicy::Forbidden`. No human or bot may take over an abandoned seat.
- **No automatic death:** disconnected seats are not killed automatically at dawn; they remain living participants with defaulting actions.

---

## 8. Death reveals and victory resolution (W-D9, W-D10, W-D11)

### Death reveal (W-D9)
- Upon elimination or night kill, the deceased player's seat and role are published publicly at Dawn or Dusk.
- Night deaths conceal the specific cause (wolf attack vs. poison) and actor identity from public clients.
- At match conclusion (`Ended`), all secret roles are revealed publicly. Match seed and private action logs are never published to outsiders.

### Resolution order (W-D11)
At phase closure:
1. Freeze all submitted choices; pending choices default to pass/abstain.
2. Calculate werewolf consensus target.
3. Apply Doctor protection and Witch heal against the werewolf attack.
4. Apply Witch poison independently.
5. Union initial deaths (duplicate kills merge into a single death). Valid choices submitted by actors killed in this batch remain effective.
6. If the Hunter is among the dead, trigger retaliation against the precommitted mark.
7. Publish sorted death events (`DeathRevealed { seat, role }`).
8. Check victory conditions.

### Victory and parity semantics (W-D10)
Checked once after the complete resolution batch:
1. **Draw:** if 0 players remain alive.
2. **Village win:** if 0 werewolves remain alive.
3. **Werewolf win:** if living werewolves $\ge$ living non-werewolves.
4. **Continue:** otherwise.

Deceased team members share their faction's victory or defeat in final match standings.

---

## 9. Information model summary

### Knowledge matrix by viewer role (W-D12, W-D13)

| Fact | Living Villager | Living Werewolf | Living Seer | Living Doctor | Living Witch | Living Hunter | Dead Seat (`Viewer::Seat`) | Outside Spectator (`Viewer::Spectator`) | Server / Audit (`Viewer::Audit`) |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **Own role** | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Hidden (Revealed at End) | Visible |
| **Other roles** | Hidden (Revealed on death/end) | Visible wolves; Hidden non-wolves | Hidden (Revealed on death/end) | Hidden (Revealed on death/end) | Hidden (Revealed on death/end) | Hidden (Revealed on death/end) | **Visible (All current roles)** | Hidden (Revealed on death/end) | Visible (All) |
| **Night action submission** | **None** | Visible own; **None** others | Visible own; **None** others | Visible own; **None** others | Visible own; **None** others | Visible own; **None** others | Visible | **None** | Visible |
| **Night targets & choices** | Hidden | Visible own; Hidden others | Visible own; Hidden others | Visible own; Hidden others | Visible own; Hidden others | Visible own; Hidden others | Visible | Hidden | Visible |
| **Seer investigation result** | Hidden | Hidden | Visible own at Dawn | Hidden | Hidden | Hidden | Visible | Hidden | Visible |
| **Doctor save success** | Hidden | Hidden (cannot distinguish save from pass) | Hidden | Hidden (only knows own target, not whether it was attacked) | Hidden | Hidden | Visible | Hidden | Visible |
| **Witch potion inventory** | Hidden | Hidden | Hidden | Hidden | Visible own | Hidden | Visible | Hidden | Visible |
| **Day ballots** | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Visible |
| **Phase & public timers** | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Visible |
| **Alive / dead roster** | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Visible | Visible |

### Dead-player knowledge (W-D12)
Dead players retain `Viewer::Seat(seat)`. From the completed transition of their death onward, they
acquire full vision of all current roles, pending choices, and retained history. They cannot issue
gameplay commands.

### Outside spectators (W-D13)
Outside spectators (`Viewer::Spectator(Live | Delayed)`) receive public-only information. Role
reveals occur only upon death or match end.

### Event non-existence (W-D15, doc 08 §5.2)
For private night actions (e.g. `NightActionSubmitted`), `view_event` returns `None` for every
unauthorized viewer. Returning `Some(RedactedEvent)` is strictly forbidden because the presence
of an event frame leaks that a private action was executed.

### Intentionally derivable vs. deliberately NOT derivable
- **Derivable:** deductive reasoning based on the public `ClassicV1` preset table, revealed dead roles,
  known wolf teammates, own role, and voting history is part of gameplay and not a leak.
- **NOT derivable:** zero deaths at Dawn does not certify a Doctor save (wolves may have passed or tied);
  no UI indicator or event distinguishes a passed action from a pending action for other players.

---

## 10. Phase timing and scope policies (W-D14, W-D16, W-D17)

### Phase durations (W-D16)
Phases have fixed durations, enforced by `LogicalTime` timers:
- **Night:** 30 seconds (default)
- **Dawn:** 2 seconds (default)
- **Day discussion:** 120 seconds (default)
- **Vote:** 30 seconds (default)
- **Dusk:** 2 seconds (default)

Valid configurable range for any phase duration: **1,000 ms to 600,000 ms**.

### Round cap (W-D17)
`max_rounds` is bounded in `1..=100` (default 100). If round 100 resolves at Dusk with no team meeting
victory conditions, the match terminates in a stalemate draw.

### Scopes (W-D14)
The rules emit absolute `ChatScopes` values:
- **Night:** Table muted; Wolves speak/listen among living wolves; Dead speak/listen among dead.
- **Dawn / Dusk:** Table listen-only; Wolves muted; Dead speak/listen among dead.
- **Day:** Table speak/listen among living; Wolves muted; Dead speak/listen among dead.
- **Vote:** Table muted; Wolves muted; Dead speak/listen among dead.
- **Ended:** Table speak/listen for all seats.

---

## 11. Bot substitution and verification simulation (W-D18)

- **Runtime substitution is Forbidden:** `SubstitutionPolicy::Forbidden`. `WerewolfModule::bot` returns
  `None` under all feature configurations, including `--features bots`.
- **Verification simulation is test-only:** a dedicated `SimulationModule` under `cfg(test)` or
  `feature = "testkit"` provides simulation heuristics for headless fuzzing and verification without
  ever exposing a bot implementation to the production runtime.

---

## 12. Generic architecture gaps recorded

These findings are architectural issues for platform crates or ADRs, not Werewolf-specific bugs:

1. **State version / ack metadata leakage:**
   `StateVersion` increments on every accepted canonical input (I-7). An external observer or client
   could infer that a hidden night action occurred by inspecting version counters or reconnect cursors.
   *Disposition:* Requires a platform ADR before the Phase 4 protocol freeze to decouple client-visible
   sync cursors from internal canonical event sequences.
2. **Voice permission asymmetry:**
   `VoiceRoom` currently defines only symmetric group membership. In Werewolf, dead players need to
   listen to living voice channels while being strictly forbidden from speaking into them.
   *Disposition:* Requires an updated generic voice authorization model before Phase-8 implementation.
3. **Audience on canonical events:**
   The `GameRules::Event` trait does not carry an `Audience` envelope (`Audience` exists on `Notify`).
   *Disposition:* Enforced in Werewolf via exhaustive `view_event -> None` handling; a platform-level
   event audience wrapper is deferred to Phase 4.

---

## 13. W1 verification ledger

| Claim | Invariant | Evidence level | Test location |
|---|---|---|---|
| ClassicV1 table exactness | `classic-v1-table-exact` | example-tested | `games/werewolf/tests/config.rs::classic_v1_table_matches_pinned_specification_for_all_seat_counts` |
| Role & Alignment closedness | `base-roles-closed`, `alignment-mapping-total` | example-tested | `games/werewolf/tests/config.rs::role_alignment_mapping_is_correct` |
| SeatCount boundaries (6..=20) | `seat-count-bounds` | example-tested | `games/werewolf/tests/config.rs::seat_count_boundaries` |
| Duration boundaries (1s..=10m) | `phase-duration-bounds` | example-tested | `games/werewolf/tests/config.rs::phase_duration_boundaries` |
| Max rounds boundaries (1..=100) | `max-rounds-bounds` | example-tested | `games/werewolf/tests/config.rs::max_rounds_boundaries` |
| Deserialization barrier | `validated-deserialization-barrier` | example-tested | `games/werewolf/tests/config.rs::deserialization_rejects_*` |
| Canonical round trip | `canonical-round-trip` | example-tested | `games/werewolf/tests/config.rs::config_canonical_round_trip` |
| Roster validation | `roster-validation-rules` | example-tested | `games/werewolf/tests/config.rs::roster_validation_enforces_boundaries_and_rules` |
