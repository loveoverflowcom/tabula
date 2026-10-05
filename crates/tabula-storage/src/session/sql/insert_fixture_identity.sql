INSERT INTO session_provider_identities (issuer, subject, user_id)
VALUES ($1, $2, $3) ON CONFLICT (issuer, subject) DO NOTHING
