SELECT id, user_id, authorization_epoch, channel, credential_generation,
       credential_digest, context_id, created_at_ms, last_activity_at_ms,
       last_observed_at_ms, idle_deadline_ms, absolute_deadline_ms,
       revoked_at_ms, expired_at_ms FROM session_auth_sessions WHERE id = $1 AND user_id = $2 FOR UPDATE
