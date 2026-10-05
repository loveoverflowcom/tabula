# Offline PostgreSQL query descriptions

These 13 query descriptions were generated unchanged by SQLx CLI 0.9.0 against
real PostgreSQL 16 after the isolated session migrations, at PR71 head
`ec4027aa95b1f438e8307e61121e6e944cad7546`.

Source receipt: [session-postgres run 37247778233](https://github.com/loveoverflowcom/tabula/actions/runs/37247778233),
artifact `session-sqlx-metadata` 11319648452. Its downloaded ZIP SHA-256 is
`f42ce26d50fe6f67816c27bb7d76d6bea60fa226cf6f394d5c7f67f4e490ccb5`.
All filenames, internal query hashes and current SQL source bytes were verified.
The run generated these descriptions and passed all 19 real-DB cases; its final
metadata-committed check failed as expected before this import. That historical
run is not reported as a full CI success.

Regenerate using `just sqlx-prepare-session` with DATABASE_URL pointing only to an
explicitly migrated disposable database. The dedicated CI job regenerates and
compares committed files, executes a non-empty ignored DB selection and proves
offline compilation. Never hand-author metadata or substitute fixture outcomes
for provider/browser/native security evidence. Both service startups stay closed.

Actions checked synthetic PR merge
`8fe36757aaadb1b5ca562841a0abdd11e4f5c28c`; its tree is
`daf3ac5a10e09c853ea59b5a276f11f5e2032dbe`, exactly the ec4027a head tree
against unchanged develop 729417f. This is a head-associated identical-tree CI
receipt, not a claim that checkout used the raw head commit.
