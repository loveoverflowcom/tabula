SELECT user_id, authorization_epoch, enabled, last_observed_at_ms
FROM session_accounts
WHERE user_id = (SELECT user_id FROM session_provider_identities WHERE issuer = $1 AND subject = $2)
