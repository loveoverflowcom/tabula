# Extraction recipes

Worked transformations. Read this when actually moving code, not when reviewing.

## Contents

1. [Pulling a core out of an HTTP handler](#1-pulling-a-core-out-of-an-http-handler)
2. [Mutating method → decide / apply](#2-mutating-method--decide--apply)
3. [Killing a mock-heavy test](#3-killing-a-mock-heavy-test)
4. [Deterministic randomness](#4-deterministic-randomness)
5. [Order of operations for a safe extraction](#5-order-of-operations-for-a-safe-extraction)

---

## 1. Pulling a core out of an HTTP handler

### Before

Rules, I/O, and transport concerns are one function. To test "you cannot transfer more than the
balance" you need a database, a runtime, and an HTTP request.

```rust
async fn transfer(
    State(db): State<PgPool>,
    Json(req): Json<TransferRequest>,
) -> Result<Json<TransferResponse>, StatusCode> {
    let from = sqlx::query_as!(Account, "SELECT ... WHERE id = $1", req.from)
        .fetch_one(&db).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let to = sqlx::query_as!(Account, "SELECT ... WHERE id = $1", req.to)
        .fetch_one(&db).await.map_err(|_| StatusCode::NOT_FOUND)?;

    if from.frozen || to.frozen {
        return Err(StatusCode::FORBIDDEN);
    }
    if from.balance < req.amount {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if req.amount <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = db.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query!("UPDATE ... SET balance = balance - $1 WHERE id = $2", req.amount, req.from)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query!("UPDATE ... SET balance = balance + $1 WHERE id = $2", req.amount, req.to)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(TransferResponse { ok: true }))
}
```

### After — core

No `async`, no `sqlx`, no `axum`. Every rule is one exhaustive read.

```rust
// domain/src/transfer.rs

pub struct Accounts { pub from: Account, pub to: Account }

#[derive(Debug, PartialEq)]
pub enum Ledger { Debit { account: AccountId, amount: Money }, Credit { account: AccountId, amount: Money } }

#[derive(Debug, PartialEq)]
pub struct Transfer { pub entries: Vec<Ledger> }

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TransferError {
    #[error("account {0} is frozen")] Frozen(AccountId),
    #[error("insufficient funds")]    InsufficientFunds,
    #[error("amount must be positive")] NonPositiveAmount,
}

pub fn decide(accounts: &Accounts, amount: Money) -> Result<Transfer, TransferError> {
    if amount <= Money::ZERO {
        return Err(TransferError::NonPositiveAmount);
    }
    if accounts.from.frozen {
        return Err(TransferError::Frozen(accounts.from.id));
    }
    if accounts.to.frozen {
        return Err(TransferError::Frozen(accounts.to.id));
    }
    if accounts.from.balance < amount {
        return Err(TransferError::InsufficientFunds);
    }

    Ok(Transfer {
        entries: vec![
            Ledger::Debit  { account: accounts.from.id, amount },
            Ledger::Credit { account: accounts.to.id,   amount },
        ],
    })
}
```

### After — shell

The handler now has one job: resolve facts, call `decide`, interpret the result. Notice the
validation ordering became *visible* — in the "before" version, a non-positive amount was checked
after two database round-trips.

```rust
async fn transfer(
    State(db): State<PgPool>,
    Json(req): Json<TransferRequest>,
) -> Result<Json<TransferResponse>, ApiError> {
    let accounts = load_accounts(&db, req.from, req.to).await?;   // resolve facts

    let transfer = decide(&accounts, req.amount)?;                 // pure rule

    apply_ledger(&db, &transfer).await?;                           // interpret
    Ok(Json(TransferResponse { ok: true }))
}

impl From<TransferError> for ApiError {
    fn from(e: TransferError) -> Self {
        match e {
            TransferError::Frozen(_)          => ApiError::Forbidden(e.to_string()),
            TransferError::InsufficientFunds  => ApiError::Unprocessable(e.to_string()),
            TransferError::NonPositiveAmount  => ApiError::BadRequest(e.to_string()),
        }
    }
}
```

Status-code mapping lives in one exhaustive `match`, so adding a `TransferError` variant produces
a compile error at the exact place a decision is required, instead of a silent 500.

### The test that is now possible

```rust
#[test]
fn cannot_transfer_more_than_balance() {
    let accounts = Accounts { from: account(100), to: account(0) };
    assert_eq!(decide(&accounts, Money::from(101)), Err(TransferError::InsufficientFunds));
}
```

---

## 2. Mutating method → decide / apply

### Before

The failure path has already mutated `self`. The caller cannot tell what changed, and a partially
applied error leaves a corrupt game.

```rust
impl Game {
    fn play(&mut self, m: Move) -> Result<(), Error> {
        self.board[m.idx] = self.turn;      // mutation before validation
        if !self.is_legal(m) {
            return Err(Error::Illegal);     // board is now corrupt
        }
        self.turn = self.turn.next();
        self.history.push(m);
        Ok(())
    }
}
```

### After

Validate fully, then mutate. `apply` is infallible because `LegalMove` can only be produced by
`decide` — the type carries the proof that validation ran.

```rust
pub struct LegalMove(Move);   // private field: only `decide` can build one

pub fn decide(state: &Game, m: Move) -> Result<LegalMove, Error> {
    if state.finished()          { return Err(Error::GameOver); }
    if state.board[m.idx].is_some() { return Err(Error::Occupied); }
    Ok(LegalMove(m))
}

pub fn apply(state: &mut Game, m: LegalMove) {
    state.board[m.0.idx] = Some(state.turn);
    state.turn = state.turn.next();
    state.history.push(m.0);
}
```

This is the cheapest possible version of "make illegal states unrepresentable" — one newtype and
a split function. Two properties fall out for free, both worth a test: a rejected input leaves
state byte-identical, and `apply` has no error path to forget.

### Event-sourced variant

When you need replay, persistence of intent, or multiple listeners:

```rust
pub fn decide(state: &Game, cmd: Command) -> Result<Vec<Event>, Error>;
pub fn evolve(state: Game, event: &Event) -> Game;
```

`decide` holds the rules and can fail; `evolve` is total and never fails, because events are
facts that already happened. Keep that asymmetry — an `evolve` that returns `Result` means a rule
leaked into the wrong half, and replay of a valid log can now fail.

---

## 3. Killing a mock-heavy test

### Before

Six mocks, and the assertions are about call counts rather than about the rule.

```rust
#[tokio::test]
async fn expired_subscription_is_rejected() {
    let mut clock = MockClock::new();
    clock.expect_now().times(1).returning(|| parse("2024-06-01T00:00:00Z"));
    let mut repo = MockRepo::new();
    repo.expect_load().times(1).returning(|_| Ok(subscription()));
    let mut audit = MockAudit::new();
    audit.expect_record().times(1).returning(|_| Ok(()));
    // ...three more mocks

    let svc = Service::new(clock, repo, audit, mailer, billing, flags);
    assert!(svc.renew(user_id()).await.is_err());
}
```

`expect_now().times(1)` is the tell: the test now fails if you refactor the code to read the
clock twice, even though behaviour is unchanged. It is asserting on implementation.

### After

```rust
#[test]
fn expired_subscription_is_rejected() {
    let sub = Subscription { expires: t("2024-05-01T00:00:00Z"), ..active() };
    let now = t("2024-06-01T00:00:00Z");

    assert_eq!(renew(&sub, now), Err(RenewError::Expired));
}

#[test]
fn renewal_emits_an_audit_record() {
    let decision = renew(&active(), t("2024-06-01T00:00:00Z")).unwrap();
    assert!(decision.effects.contains(&Effect::Audit(AuditKind::Renewed)));
}
```

The second test shows the payoff of effects-as-data: "an audit record is written" is now a value
you can assert on, instead of a mock expectation coupled to call order.

Mocks remain correct for adapter/integration tests — verifying your Postgres implementation
honours the `DocumentStore` contract. The smell is specifically mocks in a *rule* test.

---

## 4. Deterministic randomness

`rand::thread_rng()` inside a rule makes the rule unreplayable and untestable. Three options,
in increasing order of power:

**Pass the values.** Best when the rule needs a fixed, known number of random values.

```rust
pub fn deal(deck: &Deck, shuffle: Permutation) -> Hands
```

**Pass a seeded generator.** The rule stays deterministic given the seed; the shell owns seeding.

```rust
pub fn resolve(state: &State, rng: &mut ChaCha8Rng) -> Outcome
```

Use a reproducible, version-stable PRNG (`rand_chacha`), not `ThreadRng` — the point is that the
same seed replays identically across processes, machines, and architectures.

**Return a request for randomness.** Best when the amount needed depends on the decision itself.

```rust
pub enum Step {
    Done(State),
    NeedsRandom { request: RandomRequest, resume: Continuation },
}
```

The third is powerful but adds a state machine; do not start there. Same reasoning applies to
`Instant::now()` and UUID generation — both are effects wearing a value's clothes.

---

## 5. Order of operations for a safe extraction

Extractions go wrong when behaviour changes silently mid-move. This sequence keeps each step
verifiable:

1. **Pin current behaviour.** Write characterization tests against the existing entry point,
   even ugly ones with mocks. They are scaffolding and get deleted at step 7.
2. **Name the decision.** Find the exact point where "what should happen" is determined. That is
   the seam.
3. **Extract the pure function in place** — same file, `fn decide(...)`, taking already-resolved
   values. Do not move files yet.
4. **Rewrite the caller** to resolve facts, call `decide`, interpret. Run the step-1 tests.
5. **Write real tests** against `decide` directly. This is where the actual value lands, and
   where you usually discover a rule nobody had ever tested.
6. **Move it** to the domain module or crate, once it has no framework imports. If it will not
   move, something is still entangled — go back to step 3.
7. **Delete the characterization tests** that duplicate step 5.

If step 6 is impossible, the honest outcome may be that this code has no core worth separating.
That is a valid result — report it rather than forcing the structure.
