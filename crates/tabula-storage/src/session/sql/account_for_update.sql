SELECT user_id, authorization_epoch, enabled, last_observed_at_ms FROM session_accounts WHERE user_id = $1 FOR UPDATE
