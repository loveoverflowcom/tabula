-- Explicit isolated account/social migration; never run by production startup.
CREATE TABLE tabula_friend_requests (
    request_id uuid PRIMARY KEY,
    sender_id uuid NOT NULL REFERENCES session_accounts(user_id),
    recipient_id uuid NOT NULL REFERENCES session_accounts(user_id),
    pair_low uuid GENERATED ALWAYS AS (LEAST(sender_id, recipient_id)) STORED,
    pair_high uuid GENERATED ALWAYS AS (GREATEST(sender_id, recipient_id)) STORED,
    status text NOT NULL CHECK (status IN ('pending','accepted','declined','expired','cancelled')),
    revision bigint NOT NULL CHECK (revision > 0),
    created_at_ms bigint NOT NULL CHECK (created_at_ms >= 0),
    expires_at_ms bigint NOT NULL CHECK (expires_at_ms > created_at_ms),
    updated_at_ms bigint NOT NULL CHECK (updated_at_ms >= created_at_ms),
    CHECK (sender_id <> recipient_id)
);
CREATE UNIQUE INDEX tabula_friend_requests_active_pair
    ON tabula_friend_requests(pair_low, pair_high)
    WHERE status IN ('pending','accepted');
CREATE INDEX tabula_friend_requests_sender ON tabula_friend_requests(sender_id, updated_at_ms DESC);
CREATE INDEX tabula_friend_requests_recipient ON tabula_friend_requests(recipient_id, updated_at_ms DESC);

CREATE TABLE tabula_social_operations (
    actor_id uuid NOT NULL REFERENCES session_accounts(user_id),
    operation_id uuid NOT NULL,
    fingerprint text NOT NULL CHECK (octet_length(fingerprint) <= 512),
    request_id uuid NOT NULL REFERENCES tabula_friend_requests(request_id),
    PRIMARY KEY (actor_id, operation_id)
);

-- Last-seen transition metadata is never proof that an account is currently online.
CREATE TABLE tabula_social_last_seen (
    user_id uuid PRIMARY KEY REFERENCES session_accounts(user_id),
    last_seen_ms bigint NOT NULL CHECK (last_seen_ms >= 0)
);
