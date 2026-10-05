UPDATE session_accounts SET publication_lease_started_at_ms = $2, publication_lease_until_ms = $3
WHERE user_id = $1
