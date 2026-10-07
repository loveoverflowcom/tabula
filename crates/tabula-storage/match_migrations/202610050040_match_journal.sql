-- ADR-0040: additive isolated schema only; no general gameplay/session schema.
-- A claim creates only an uninitialized fenced head. Genesis initializes it
-- atomically with the immutable creation and first canonical journal record.
CREATE TABLE match_journal_heads (
    match_id UUID PRIMARY KEY CHECK (match_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    fence BIGINT NOT NULL CHECK (fence > 0),
    format SMALLINT NOT NULL CHECK (format = 1),
    version BIGINT,
    input_index BIGINT,
    observed_ms BIGINT NOT NULL DEFAULT 0 CHECK (observed_ms >= 0),
    record_count BIGINT NOT NULL DEFAULT 0 CHECK (record_count BETWEEN 0 AND 10001),
    record_bytes BIGINT NOT NULL DEFAULT 0 CHECK (record_bytes BETWEEN 0 AND 67108864),
    creation BYTEA,
    creation_hash BYTEA,
    ledger BYTEA,
    ledger_hash BYTEA,
    latest_hash BYTEA,
    CHECK ((version IS NULL AND input_index IS NULL AND record_count = 0
            AND record_bytes = 0 AND observed_ms = 0 AND creation IS NULL AND creation_hash IS NULL
            AND ledger IS NULL AND ledger_hash IS NULL AND latest_hash IS NULL)
        OR (version IS NOT NULL AND input_index IS NOT NULL
            AND version >= 0 AND input_index = version AND record_count = input_index + 1
            AND record_bytes > 0 AND creation IS NOT NULL AND creation_hash IS NOT NULL
            AND ledger IS NOT NULL AND ledger_hash IS NOT NULL AND latest_hash IS NOT NULL)),
    CHECK (creation IS NULL OR octet_length(creation) BETWEEN 2 AND 1048576),
    CHECK (ledger IS NULL OR octet_length(ledger) BETWEEN 2 AND 4194304),
    CHECK (creation_hash IS NULL OR octet_length(creation_hash) = 32),
    CHECK (ledger_hash IS NULL OR octet_length(ledger_hash) = 32),
    CHECK (latest_hash IS NULL OR octet_length(latest_hash) = 32)
);

-- Payload is canonical JournalRecord with ledger cleared. The complete bounded
-- operation ledger lives in the same atomic head, avoiding quadratic history.
CREATE TABLE match_journal_records (
    match_id UUID NOT NULL REFERENCES match_journal_heads(match_id),
    input_index BIGINT NOT NULL CHECK (input_index BETWEEN 0 AND 10000),
    state_version BIGINT NOT NULL CHECK (state_version = input_index),
    logical_ms BIGINT NOT NULL CHECK (logical_ms >= 0),
    state_hash BYTEA NOT NULL CHECK (octet_length(state_hash) = 32),
    payload BYTEA NOT NULL CHECK (octet_length(payload) BETWEEN 2 AND 67108864),
    payload_bytes BIGINT NOT NULL CHECK (payload_bytes = octet_length(payload)),
    payload_hash BYTEA NOT NULL CHECK (octet_length(payload_hash) = 32),
    PRIMARY KEY (match_id, input_index)
);
