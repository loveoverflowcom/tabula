SELECT pg_advisory_xact_lock(hashtextextended('session_accounts'::regclass::oid::text || ':' || $1::uuid::text, 541))
