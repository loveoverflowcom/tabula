-- ADR-0044: separate opt-in account enrollment/profile schema. Production remains gated.
CREATE TABLE account_enrollment_policy (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 enabled BOOLEAN NOT NULL DEFAULT FALSE,
 policy_epoch BIGINT NOT NULL CHECK(policy_epoch>=0),
 last_observed_at_ms BIGINT NOT NULL CHECK(last_observed_at_ms>=0)
);
INSERT INTO account_enrollment_policy(singleton,enabled,policy_epoch,last_observed_at_ms) VALUES(TRUE,FALSE,0,0);
CREATE TABLE account_profiles (
 user_id UUID PRIMARY KEY REFERENCES session_accounts(user_id),
 handle TEXT COLLATE "C" NOT NULL UNIQUE CHECK(handle ~ '^[a-z0-9_]{3,32}$'),
 display_name TEXT NOT NULL CHECK(char_length(display_name) BETWEEN 1 AND 64 AND octet_length(display_name)<=256),
 visibility TEXT NOT NULL CHECK(visibility IN ('public','friends','private')),
 revision BIGINT NOT NULL CHECK(revision>=1)
);
CREATE TABLE account_enrollment_grants (
 digest BYTEA PRIMARY KEY CHECK(octet_length(digest)=32),
 operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 issuer TEXT COLLATE "C" NOT NULL CHECK(octet_length(issuer) BETWEEN 1 AND 1024),
 subject TEXT COLLATE "C" NOT NULL CHECK(octet_length(subject) BETWEEN 1 AND 1024),
 policy_epoch BIGINT NOT NULL CHECK(policy_epoch>=0),
 expected_account_epoch BIGINT CHECK(expected_account_epoch>=0),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms>=0),
 expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>created_at_ms AND expires_at_ms-created_at_ms=300000),
 disposition TEXT NOT NULL CHECK(disposition IN ('ready','accepted','rejected')),
 requested_handle TEXT COLLATE "C",
 requested_display_name TEXT,
 CHECK((disposition='ready' AND requested_handle IS NULL AND requested_display_name IS NULL) OR (disposition<>'ready' AND requested_handle IS NOT NULL AND requested_display_name IS NOT NULL))
);
CREATE INDEX account_enrollment_grants_expiry ON account_enrollment_grants(expires_at_ms);
CREATE TABLE account_profile_receipts (
 user_id UUID NOT NULL REFERENCES account_profiles(user_id),
 operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 expected_revision BIGINT NOT NULL CHECK(expected_revision>=1),
 display_name TEXT NOT NULL,
 visibility TEXT NOT NULL CHECK(visibility IN ('public','friends','private')),
 resulting_revision BIGINT NOT NULL CHECK(resulting_revision=expected_revision+1),
 PRIMARY KEY(user_id,operation_id)
);
