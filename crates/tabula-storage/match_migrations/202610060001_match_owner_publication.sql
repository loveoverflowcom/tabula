-- PR3 isolated owner-output exclusion. This is neither placement nor an owner
-- lease: native ownership is a retained physical advisory-lock connection.
-- A committed short publication window survives that connection's loss until
-- every corresponding local monotonic first-frame guard is unusable.
CREATE TABLE match_journal_publications (
    match_id UUID PRIMARY KEY REFERENCES match_journal_heads(match_id),
    fence BIGINT NOT NULL CHECK (fence > 0),
    started_at_ms BIGINT NOT NULL CHECK (started_at_ms >= 0),
    until_ms BIGINT NOT NULL,
    CHECK (until_ms > started_at_ms AND until_ms - started_at_ms <= 2000)
);
