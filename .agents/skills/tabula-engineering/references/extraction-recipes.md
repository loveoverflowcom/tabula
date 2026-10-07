# Extraction recipes

Supporting reference for [Tabula engineering](../SKILL.md) and
[functional core](functional-core.md). Use these recipes while moving code, not for routine glue.

## Contents

1. Handler to core and shell
2. Fallible mutation to decide and commit
3. Event decision and evolution
4. Safe extraction sequence
5. Pure tests and deterministic context

## 1. Handler to core and shell

Mixed handlers hide rules between I/O calls:

```rust
async fn transfer(db: &Pool, request: Request) -> Result<Response, ApiError> {
    let from = load_account(db, request.from).await?;
    let to = load_account(db, request.to).await?;
    if request.amount <= 0 || from.frozen || from.balance < request.amount {
        return Err(ApiError::Invalid);
    }
    write_transfer(db, from.id, to.id, request.amount).await?;
    Ok(Response::accepted())
}
```

Extract the facts and decision:

```rust
pub struct TransferFacts {
    pub from: Account,
    pub to: Account,
}

pub enum TransferError {
    NonPositiveAmount,
    Frozen(AccountId),
    InsufficientFunds,
}

pub struct Transfer {
    pub debit: LedgerEntry,
    pub credit: LedgerEntry,
}

pub fn decide_transfer(
    facts: &TransferFacts,
    amount: PositiveMoney,
) -> Result<Transfer, TransferError> {
    if facts.from.frozen {
        return Err(TransferError::Frozen(facts.from.id));
    }
    if facts.to.frozen {
        return Err(TransferError::Frozen(facts.to.id));
    }
    if facts.from.balance < amount.get() {
        return Err(TransferError::InsufficientFunds);
    }

    Ok(Transfer {
        debit: LedgerEntry::debit(facts.from.id, amount),
        credit: LedgerEntry::credit(facts.to.id, amount),
    })
}
```

The shell now resolves facts, invokes the rule once, and interprets the result:

```rust
async fn transfer(db: &Pool, request: Request) -> Result<Response, ApiError> {
    let amount = PositiveMoney::try_from(request.amount)?;
    let facts = load_transfer_facts(db, request.from, request.to).await?;
    let transfer = decide_transfer(&facts, amount)?;
    persist_transfer(db, &transfer).await?;
    Ok(Response::accepted())
}
```

Keep transport validation that reports malformed fields at the transport boundary. Put semantic
rules such as frozen accounts and balance sufficiency in the core.

Map the domain error to an application/transport error in one exhaustive match at the edge.
Adding a new error variant then forces an explicit transport decision. The domain test can assert
`Err(TransferError::InsufficientFunds)` without HTTP status strings or a database.

## 2. Fallible mutation to decide and commit

Avoid partial mutation:

```rust
fn play(state: &mut Game, command: Command) -> Result<Event, RuleError> {
    state.board[command.index] = Some(state.turn);
    if !is_legal(state, &command) {
        return Err(RuleError::Illegal);
    }
    // ...
}
```

Make validation produce evidence:

```rust
pub struct LegalMove(Command);

pub fn decide(state: &Game, command: Command) -> Result<LegalMove, RuleError> {
    let square = state.board.get(command.index).ok_or(RuleError::OutOfBounds)?;
    if state.finished() {
        return Err(RuleError::GameOver);
    }
    if square.is_some() {
        return Err(RuleError::Occupied);
    }
    Ok(LegalMove(command))
}

pub fn commit(state: &mut Game, legal: LegalMove) -> Event {
    let command = legal.0;
    state.board[command.index] = Some(state.turn);
    state.turn = state.turn.next();
    Event::Moved(command)
}
```

Keep `LegalMove` construction private to the deciding module. Test every `RuleError` class and a
single property: any rejected command leaves the canonical state encoding unchanged.

The example's decide/commit pair must stay within one `apply` call on the same state without
intervening mutation. A public `commit` accepting a stale `LegalMove` can violate the premise;
scope it privately, tie it to the checked borrow/version, or revalidate. A private field alone
does not establish freshness.

## 3. Event decision and evolution

Use this shape when replay or multiple consumers matter:

```rust
pub fn decide(state: &State, command: Command) -> Result<Vec<Event>, RuleError>;
pub fn evolve(state: &mut State, event: &Event);
```

`decide` owns rules and may fail. `evolve` consumes facts that already happened and should be
total for events valid in the current rules version. If `evolve` needs normal rule validation,
the decision/evolution boundary is leaking.

For Tabula, this is an internal implementation option behind `GameRules::apply`; it does not
create a second mutation path. The authoritative replay contract remains the ordered `Input`
stream, including logical times, seed, and rules version (doc 00 §3.1).

Test:

- replaying emitted events produces the same state as the live path;
- applying the same ordered input stream twice from the same seed produces identical bytes;
- an invalid input emits no event and changes no state;
- migrations make old valid event sequences explicit rather than silently reinterpret them.

## 4. Safe extraction sequence

1. Freeze observable behavior with characterization tests if coverage is weak.
2. Identify plain facts consumed by the rule.
3. Introduce domain input/output/error types without moving behavior.
4. Extract a pure function and keep the existing shell as its caller.
5. Add exact semantic and property tests around the pure function.
6. Remove mocks that no longer assert an adapter contract.
7. Move types/modules only after behavior is stable.
8. Re-run dependency and architecture checks.

Avoid combining extraction, public API redesign, dependency upgrades, and unrelated cleanup in one
change. Each independent axis makes a regression harder to localize.

## 5. Pure tests and deterministic context

Replace rule tests whose mocks assert clock reads or repository call counts with plain facts and
an exact result. Keep adapter tests for actual storage/transport contracts. With effects returned
as data, a rule can assert that an audit effect is requested; integration checks establish that
the shell actually persists/interprets it.

```rust
#[test]
fn expired_subscription_is_rejected() {
    let facts = subscription_expiring_at(LogicalTime::from_millis(10));
    assert_eq!(renew(&facts, LogicalTime::from_millis(11)), Err(RenewError::Expired));
}
```

Randomness enters Tabula rules through the deterministic `Ctx` / `DetRng` contract, never a
generic clock/RNG service or OS entropy. A rule can consume an already-known random fact where
the contract allows it, or use its documented domain stream from `Ctx`. The shell owns seed
creation. Introducing a resumable request for external randomness would change the ordered-input
contract and needs an explicit architecture decision; it is not a default extraction technique.
