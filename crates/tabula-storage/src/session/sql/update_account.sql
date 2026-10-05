UPDATE session_accounts SET authorization_epoch = $2, enabled = $3, last_observed_at_ms = $4
WHERE user_id = $1
