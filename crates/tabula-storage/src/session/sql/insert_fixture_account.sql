INSERT INTO session_accounts (user_id, authorization_epoch, enabled, last_observed_at_ms)
VALUES ($1, $2, $3, $4) ON CONFLICT (user_id) DO NOTHING
