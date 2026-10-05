UPDATE session_auth_sessions SET
    authorization_epoch = $3, channel = $4, credential_generation = $5,
    credential_digest = $6, context_id = $7, created_at_ms = $8,
    last_activity_at_ms = $9, last_observed_at_ms = $10,
    idle_deadline_ms = $11, absolute_deadline_ms = $12,
    revoked_at_ms = $13, expired_at_ms = $14
WHERE id = $1 AND user_id = $2
