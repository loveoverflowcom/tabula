# Unify existing Tabula identity

## Outcome

Implement [issue #91](https://github.com/loveoverflowcom/tabula/issues/91) as a
new draft PR into develop, from freshly fetched7716b2e after PR93 normal merge.
One approved T Portal source and semantic brand palette serve existing surfaces.

## Why

The active web shell used a T + dot, native setup used a different live-font
wordmark, mobile used a text heading and launcher targets had no Tabula icon.
The approved source is pinned ae40d8ef; immutable source provenance and
reproducible target exports prevent another independent logo per app.

## Review boundary

Only branding, its existing shared token/export pipeline and current consumers.
Keep the original Design01 dashboard hierarchy, games' artwork/palettes,
account avatars, rules, navigation and phase gates. Primary/on-primary/selected
remain unchanged; unmerged PR90 continues to own action-palette intent.

## Evidence and dependencies

[Asset source/export contract](../../assets/brand/README.md) and
[scoped verification ledger](../verification/brand-identity/README.md) distinguish
source/pixels/builds from actual device and browser rendering. No GitHub CI
status checks/waiting; requested validation is local. A separate authorized
runtime screenshot pass is QA, not a merge gate.

Android build/UI execution needs the pinned Gradle distribution, JDK17 and an
Android SDK. iOS needs macOS/Xcode. These are target evidence prerequisites,
not authorization to create a desktop launcher, About screen or new host.

## Completion

Implementation and target exports are complete. PR94's skip-link correction
is integrated from5e66202c. Finish exact-source local gates, publish the final
commit, verify its remote identity and leave the PR draft.
Do not merge this new logo PR without subsequent authorization.
