-- ADR-0035 isolated validation slice. Additive only. Kanidm owns credentials;
-- no password, provider token, e-mail matching or plaintext session is stored.
CREATE TABLE session_accounts (
    user_id UUID PRIMARY KEY,
    authorization_epoch BIGINT NOT NULL CHECK (authorization_epoch >= 0),
    enabled BOOLEAN NOT NULL,
    last_observed_at_ms BIGINT NOT NULL CHECK (last_observed_at_ms >= 0),
    CHECK (user_id <> '00000000-0000-0000-0000-000000000000'::uuid)
);

CREATE TABLE session_provider_identities (
    -- Each exact UTF-8 component is bounded to 1024 bytes. The composite key
    -- stays below PostgreSQL's B-tree tuple limit without digest collisions,
    -- truncation, normalization, or an additional issuer registry.
    issuer TEXT COLLATE "C" NOT NULL CHECK (octet_length(issuer) BETWEEN 1 AND 1024),
    subject TEXT COLLATE "C" NOT NULL CHECK (octet_length(subject) BETWEEN 1 AND 1024),
    user_id UUID NOT NULL REFERENCES session_accounts(user_id),
    PRIMARY KEY (issuer, subject)
);

CREATE TABLE session_auth_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES session_accounts(user_id),
    authorization_epoch BIGINT NOT NULL CHECK (authorization_epoch >= 0),
    channel TEXT NOT NULL CHECK (channel IN ('browser_cookie', 'native_bearer')),
    credential_generation BIGINT NOT NULL CHECK (credential_generation >= 0),
    credential_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(credential_digest) = 32),
    context_id UUID NOT NULL,
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    last_activity_at_ms BIGINT NOT NULL,
    last_observed_at_ms BIGINT NOT NULL,
    idle_deadline_ms BIGINT NOT NULL,
    absolute_deadline_ms BIGINT NOT NULL,
    revoked_at_ms BIGINT,
    expired_at_ms BIGINT,
    CHECK (id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CHECK (context_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CHECK (created_at_ms <= 9223372036768375807),
    CHECK (absolute_deadline_ms = created_at_ms + 86400000),
    CHECK (last_activity_at_ms >= created_at_ms),
    CHECK (last_activity_at_ms < absolute_deadline_ms),
    CHECK (last_observed_at_ms >= last_activity_at_ms),
    CHECK (idle_deadline_ms >= last_activity_at_ms),
    CHECK (idle_deadline_ms <= absolute_deadline_ms),
    CHECK (idle_deadline_ms - last_activity_at_ms <= 1800000),
    CHECK (idle_deadline_ms = absolute_deadline_ms
        OR idle_deadline_ms - last_activity_at_ms = 1800000),
    CHECK (revoked_at_ms IS NULL OR (revoked_at_ms >= created_at_ms
        AND revoked_at_ms <= last_observed_at_ms)),
    CHECK (expired_at_ms IS NULL OR (expired_at_ms >= LEAST(idle_deadline_ms, absolute_deadline_ms)
        AND expired_at_ms <= last_observed_at_ms))
);
