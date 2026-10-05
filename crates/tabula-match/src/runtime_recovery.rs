//! Fail-closed approved-module replay of the bounded durable prefix (ADR-0040).
use crate::durable::{
    JournalRecord, LoadedMatch, MatchCreation, OperationScope, JOURNAL_FORMAT, MAX_RECOVERY_BYTES,
};
use crate::runtime::{ledger_limits, Limits, RecoveryError};
use std::collections::BTreeMap;
use tabula_core::{canonical_encode, DetRng, MatchId, MatchSeed, StateVersion};
use tabula_game_api::Effect;
use tabula_registry::runtime::{CreatedMatch, ErasedInput, ErasedMatch, RuntimeIdentity};
fn same_encoding<T: serde::Serialize + ?Sized>(left: &T, right: &T) -> Result<bool, RecoveryError> {
    Ok(canonical_encode(left).map_err(|_| RecoveryError::Corrupt)?
        == canonical_encode(right).map_err(|_| RecoveryError::Corrupt)?)
}

pub(crate) fn validate(
    id: MatchId,
    game: &dyn tabula_registry::ErasedGame,
    loaded: &crate::durable::LoadedMatch,
    limits: Limits,
) -> Result<Box<dyn ErasedMatch>, RecoveryError> {
    use crate::durable::{MAX_LEDGER_BYTES, MAX_RECOVERY_RECORDS};
    let fail = || RecoveryError::Corrupt;
    let creation = &loaded.creation;
    if creation.format != JOURNAL_FORMAT
        || creation.identity.rules_hash == [0; 32]
        || creation.limits != ledger_limits(limits)
        || loaded.records.is_empty()
        || loaded.records.len() > MAX_RECOVERY_RECORDS
        || loaded.ledger.len() > limits.operation_scopes
        || canonical_encode(&loaded.ledger).map_err(|_| fail())?.len() > MAX_LEDGER_BYTES
    {
        return Err(fail());
    }
    let approved = tabula_registry::runtime::RuntimeIdentity {
        game: creation.identity.game.clone(),
        game_version: creation.identity.game_version.clone(),
        rules_version: creation.identity.rules_version,
        rules_hash: creation.identity.rules_hash,
    };
    let created = game
        .create_match(
            &creation.config,
            &creation.roster,
            MatchSeed::from_bytes(creation.seed),
        )
        .map_err(|_| fail())?;
    if created.runtime().identity() != &approved {
        return Err(fail());
    }
    let (state, accepted_scopes) = validate_records(id, game, loaded, &approved, created)?;
    validate_ledger(creation, loaded, limits, &accepted_scopes)?;
    Ok(state)
}

fn validate_ledger(
    creation: &MatchCreation,
    loaded: &LoadedMatch,
    limits: Limits,
    accepted_scopes: &BTreeMap<OperationScope, u64>,
) -> Result<(), RecoveryError> {
    let fail = || RecoveryError::Corrupt;
    let mut previous_scope = None;
    for scope in &loaded.ledger {
        if previous_scope.is_some_and(|prior| prior >= scope.scope)
            || scope.recent.len() > limits.receipts_per_scope
            || creation.roster.get(scope.scope.seat).is_none()
            || accepted_scopes
                .get(&scope.scope)
                .is_some_and(|seq| *seq > scope.highest)
        {
            return Err(fail());
        }
        previous_scope = Some(scope.scope);
        let mut seq = 0;
        for receipt in &scope.recent {
            if receipt.seq == 0
                || receipt.seq <= seq
                || receipt.seq > scope.highest
                || receipt.at > loaded.observed_ms
            {
                return Err(fail());
            }
            seq = receipt.seq;
            match (receipt.result, receipt.committed_index) {
                (Ok(()), Some(index)) => {
                    let record = loaded
                        .records
                        .get(usize::try_from(index.0).map_err(|_| fail())?)
                        .ok_or_else(fail)?;
                    let operation = record.operation.as_ref().ok_or_else(fail)?;
                    if operation.scope != scope.scope
                        || operation.seq != receipt.seq
                        || operation.command != receipt.command
                        || record.now.0 != receipt.at
                    {
                        return Err(fail());
                    }
                }
                (Err(_), None) => {}
                _ => return Err(fail()),
            }
        }
    }
    if accepted_scopes
        .keys()
        .any(|scope| !loaded.ledger.iter().any(|entry| &entry.scope == scope))
    {
        return Err(fail());
    }
    Ok(())
}

type ReplayedPrefix = (Box<dyn ErasedMatch>, BTreeMap<OperationScope, u64>);

fn validate_records(
    id: MatchId,
    game: &dyn tabula_registry::ErasedGame,
    loaded: &LoadedMatch,
    approved: &RuntimeIdentity,
    created: CreatedMatch,
) -> Result<ReplayedPrefix, RecoveryError> {
    let fail = || RecoveryError::Corrupt;
    let creation = &loaded.creation;
    let (mut state, seed, initial_events, initial_effects) = created.into_parts();
    let mut bytes = 0usize;
    let mut time = 0u64;
    let mut terminal = false;
    let mut accepted_scopes = BTreeMap::<OperationScope, u64>::new();
    for (ordinal, record) in loaded.records.iter().enumerate() {
        bytes = bytes
            .checked_add(canonical_encode(record).map_err(|_| fail())?.len())
            .ok_or_else(fail)?;
        let ordinal = u64::try_from(ordinal).map_err(|_| fail())?;
        if bytes > MAX_RECOVERY_BYTES
            || record.match_id != id
            || record.index.0 != ordinal
            || record.version.0 != ordinal
            || record.now.0 < time
            || record.now.0 > loaded.observed_ms
            || terminal
        {
            return Err(fail());
        }
        let expected = if ordinal == 0 {
            None
        } else {
            Some(StateVersion(ordinal - 1))
        };
        if record.expected_version != expected {
            return Err(fail());
        }
        let effects = replay_input(
            id,
            record,
            ordinal,
            creation,
            approved,
            &initial_events,
            &initial_effects,
            state.as_mut(),
            &seed,
            &mut accepted_scopes,
        )?;
        terminal |= effects
            .iter()
            .any(|effect| matches!(effect, Effect::EndMatch { .. }));
        if record.terminal != terminal
            || record.hash != state.state_hash()
            || record.snapshot.is_some() != (ordinal == 0 || ordinal % 20 == 0 || terminal)
        {
            return Err(fail());
        }
        if let Some(snapshot) = &record.snapshot {
            if state.snapshot().map_err(|_| fail())? != *snapshot {
                return Err(fail());
            }
            let restored = game
                .restore_match(approved, &creation.config, &creation.roster, snapshot)
                .map_err(|_| fail())?;
            if restored.state_hash() != record.hash
                || restored.snapshot().map_err(|_| fail())? != *snapshot
            {
                return Err(fail());
            }
            state = restored;
        }
        time = record.now.0;
    }
    let last = loaded.records.last().ok_or_else(fail)?;
    if last.index != loaded.index || last.version != loaded.version {
        return Err(fail());
    }
    Ok((state, accepted_scopes))
}

// The replay context explicitly binds every external persisted fact; it never
// carries a database handle or emits effects/output while verifying the prefix.
#[allow(clippy::too_many_arguments)]
fn replay_input(
    id: MatchId,
    record: &JournalRecord,
    ordinal: u64,
    creation: &MatchCreation,
    approved: &RuntimeIdentity,
    initial_events: &[Vec<u8>],
    initial_effects: &[Effect],
    state: &mut dyn ErasedMatch,
    seed: &MatchSeed,
    accepted_scopes: &mut BTreeMap<OperationScope, u64>,
) -> Result<Vec<Effect>, RecoveryError> {
    let fail = || RecoveryError::Corrupt;
    let effects = if ordinal == 0 {
        if !record.input.is_empty()
            || record.now.0 != 0
            || record.operation.is_some()
            || !same_encoding(record.creation.as_ref().ok_or_else(fail)?, creation)?
            || record.events.as_slice() != initial_events
            || !same_encoding(record.effects.as_slice(), initial_effects)?
        {
            return Err(fail());
        }
        initial_effects.to_vec()
    } else {
        if record.creation.is_some() || record.input.is_empty() {
            return Err(fail());
        }
        let mut rng = DetRng::for_input(seed, record.index);
        let transition = if let Some(operation) = &record.operation {
            if operation.seq == 0
                || operation.command.match_id() != id
                || operation.command.game() != &approved.game
                || operation.command.game_version() != &approved.game_version
                || accepted_scopes
                    .get(&operation.scope)
                    .is_some_and(|highest| operation.seq <= *highest)
            {
                return Err(fail());
            }
            accepted_scopes.insert(operation.scope, operation.seq);
            state.apply(
                ErasedInput::Player {
                    seat: operation.scope.seat,
                    payload: operation.command.payload().to_vec(),
                },
                record.now,
                record.index,
                &mut rng,
            )
        } else {
            state.replay(&record.input, record.now, record.index, &mut rng)
        }
        .map_err(|_| fail())?;
        if transition.canonical_input != record.input
            || transition.events != record.events
            || !same_encoding(&transition.effects, &record.effects)?
        {
            return Err(fail());
        }
        transition.effects
    };
    Ok(effects)
}
