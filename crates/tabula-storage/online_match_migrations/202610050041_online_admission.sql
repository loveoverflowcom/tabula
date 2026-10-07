-- Isolated PR2 composition only: authority, admissions and journal in one schema.
CREATE TABLE online_match_rooms (
 match_id UUID PRIMARY KEY CHECK(match_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 creator_user_id UUID NOT NULL REFERENCES session_accounts(user_id),
 code_digest BYTEA NOT NULL UNIQUE CHECK(octet_length(code_digest)=32),
 game_id TEXT NOT NULL CHECK(octet_length(game_id) BETWEEN 3 AND 256),
 game_version TEXT NOT NULL CHECK(octet_length(game_version) BETWEEN 5 AND 128),
 config BYTEA NOT NULL CHECK(octet_length(config) BETWEEN 1 AND 65536),
 seats SMALLINT NOT NULL CHECK(seats BETWEEN 2 AND 8),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms>=0),
 code_deadline_ms BIGINT NOT NULL CHECK(code_deadline_ms>created_at_ms AND code_deadline_ms-created_at_ms<=600000),
 started BOOLEAN NOT NULL DEFAULT FALSE,
 completed BOOLEAN NOT NULL DEFAULT FALSE,
 CHECK(NOT completed OR started)
);
CREATE TABLE online_match_memberships (
 match_id UUID NOT NULL REFERENCES online_match_rooms(match_id),
 user_id UUID NOT NULL REFERENCES session_accounts(user_id),
 seat SMALLINT NOT NULL CHECK(seat BETWEEN 0 AND 7),
 generation BIGINT NOT NULL CHECK(generation=1),
 PRIMARY KEY(match_id,user_id), UNIQUE(match_id,seat), UNIQUE(match_id,user_id,seat,generation)
);
CREATE TABLE online_match_admissions (
 match_id UUID NOT NULL,
 session_id UUID NOT NULL REFERENCES session_auth_sessions(id),
 user_id UUID NOT NULL,
 authorization_epoch BIGINT NOT NULL CHECK(authorization_epoch>=0),
 seat SMALLINT NOT NULL,
 generation BIGINT NOT NULL,
 PRIMARY KEY(match_id,session_id),
 FOREIGN KEY(match_id,user_id,seat,generation) REFERENCES online_match_memberships(match_id,user_id,seat,generation)
);
CREATE TABLE online_match_join_attempts (
 session_id UUID PRIMARY KEY REFERENCES session_auth_sessions(id),
 window_started_at_ms BIGINT NOT NULL CHECK(window_started_at_ms>=0),
 attempts SMALLINT NOT NULL CHECK(attempts BETWEEN 1 AND 8)
);
-- This storage-only transaction witness is not a transferable permit. Its
-- original session deadline prevents accepted activity reviving a command that
-- expires while awaited SQL or COMMIT is pending. Its extra budget is two seconds.
CREATE TABLE online_session_commit_guards (
 session_id UUID PRIMARY KEY REFERENCES session_auth_sessions(id),
 transaction_id BIGINT NOT NULL,
 user_id UUID NOT NULL REFERENCES session_accounts(user_id),
 authorization_epoch BIGINT NOT NULL,
 credential_digest BYTEA NOT NULL CHECK(octet_length(credential_digest)=32),
 channel TEXT NOT NULL CHECK(channel IN ('browser_cookie','native_bearer')),
 context_id UUID NOT NULL,
 deadline_ms BIGINT NOT NULL CHECK(deadline_ms>0)
);
CREATE TABLE online_match_commit_guards (
 match_id UUID PRIMARY KEY REFERENCES online_match_rooms(match_id),
 transaction_id BIGINT NOT NULL,
 session_id UUID NOT NULL REFERENCES online_session_commit_guards(session_id),
 user_id UUID NOT NULL,
 authorization_epoch BIGINT NOT NULL,
 seat SMALLINT NOT NULL,
 generation BIGINT NOT NULL
);
CREATE FUNCTION online_validate_session_commit() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE g online_session_commit_guards%ROWTYPE; s session_auth_sessions%ROWTYPE;
 a session_accounts%ROWTYPE; now_ms BIGINT;
BEGIN
 SELECT * INTO STRICT g FROM online_session_commit_guards WHERE session_id=NEW.session_id;
 SELECT * INTO STRICT s FROM session_auth_sessions WHERE id=g.session_id;
 SELECT * INTO STRICT a FROM session_accounts WHERE user_id=g.user_id;
 now_ms:=floor(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint;
 IF g.transaction_id<>txid_current() OR now_ms>=g.deadline_ms
 OR now_ms<s.last_observed_at_ms OR now_ms<a.last_observed_at_ms
 OR s.user_id<>g.user_id OR NOT a.enabled
 OR s.authorization_epoch<>g.authorization_epoch OR a.authorization_epoch<>g.authorization_epoch
 OR s.credential_digest<>g.credential_digest OR s.channel<>g.channel OR s.context_id<>g.context_id
 OR s.revoked_at_ms IS NOT NULL OR s.expired_at_ms IS NOT NULL
 OR now_ms>=s.idle_deadline_ms OR now_ms>=s.absolute_deadline_ms THEN
 RAISE EXCEPTION 'online authority expired' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER online_session_commit_fence AFTER INSERT OR UPDATE ON online_session_commit_guards
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION online_validate_session_commit();
CREATE FUNCTION online_validate_match_commit() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE g online_match_commit_guards%ROWTYPE; room online_match_rooms%ROWTYPE;
 admission online_match_admissions%ROWTYPE; session_guard online_session_commit_guards%ROWTYPE;
BEGIN
 -- Pure owner claims and offline matches preserve ADR-0040 compatibility.
 IF NEW.version IS NULL OR (TG_OP='UPDATE' AND NEW.version IS NOT DISTINCT FROM OLD.version
 AND NEW.ledger IS NOT DISTINCT FROM OLD.ledger AND NEW.observed_ms IS NOT DISTINCT FROM OLD.observed_ms) THEN RETURN NULL; END IF;
 SELECT * INTO room FROM online_match_rooms WHERE match_id=NEW.match_id;
 IF NOT FOUND THEN RETURN NULL; END IF;
 SELECT * INTO STRICT g FROM online_match_commit_guards WHERE match_id=NEW.match_id;
 SELECT * INTO STRICT session_guard FROM online_session_commit_guards WHERE session_id=g.session_id;
 SELECT * INTO STRICT admission FROM online_match_admissions WHERE match_id=NEW.match_id AND session_id=g.session_id;
 IF g.transaction_id<>txid_current() OR session_guard.transaction_id<>txid_current()
 OR admission.user_id<>g.user_id OR admission.authorization_epoch<>g.authorization_epoch
 OR admission.seat<>g.seat OR admission.generation<>g.generation
 OR session_guard.user_id<>g.user_id OR session_guard.authorization_epoch<>g.authorization_epoch
 OR (SELECT count(*) FROM online_match_memberships WHERE match_id=NEW.match_id)<>room.seats THEN
 RAISE EXCEPTION 'online admission changed' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER online_match_commit_fence AFTER INSERT OR UPDATE ON match_journal_heads
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION online_validate_match_commit();
