INSERT INTO session_auth_sessions (id, user_id, authorization_epoch, channel, credential_generation,
       credential_digest, context_id, created_at_ms, last_activity_at_ms,
       last_observed_at_ms, idle_deadline_ms, absolute_deadline_ms,
       revoked_at_ms, expired_at_ms)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
