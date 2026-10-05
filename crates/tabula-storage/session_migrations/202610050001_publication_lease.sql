-- ADR-0036 isolated first-frame exclusion survives loss of a publishing backend.
-- A lease is bounded by both a 2-second ceiling and the current session deadline.
-- All authority operations honor this committed exclusion before observing time.
ALTER TABLE session_accounts
    ADD COLUMN publication_lease_started_at_ms BIGINT,
    ADD COLUMN publication_lease_until_ms BIGINT,
    ADD CONSTRAINT session_accounts_publication_lease_bounds CHECK (
        (publication_lease_started_at_ms IS NULL AND publication_lease_until_ms IS NULL)
        OR
        (publication_lease_started_at_ms IS NOT NULL AND publication_lease_until_ms IS NOT NULL
         AND publication_lease_started_at_ms >= 0
         AND publication_lease_until_ms > publication_lease_started_at_ms
         AND publication_lease_until_ms - publication_lease_started_at_ms <= 2000)
    );
