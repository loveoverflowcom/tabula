# Mobile account UI and native integration acceptance

**Status (2026-10-07):** [issue #103](https://github.com/loveoverflowcom/tabula/issues/103)
has a bounded UI draft under [ADR-0046](../../adr/0046-mobile-account-surfaces.md).
Its [ledger](../../verification/issue-103-mobile-account/README.md) records
focused JVM state/controller tests separately from the blocked pinned mobile
build and unexecuted Compose/device acceptance. The issue remains open.

## Outcome / why

First establish the existing account screens on the pinned CMP/native targets.
Then, when separately authorized, connect the retained Kanidm/session/profile
contracts through a real native adapter without replacing the screen model.
Current isolated web login/enrollment/profile/friends exist under ADR-0038/0044;
they do not establish native provider, secure-store or social delivery. Fake
sessions or more screenshot fixtures cannot close that boundary.

## Dependencies and small review boundaries

1. **Execute current UI acceptance.** Restore access to the pinned Gradle
   distribution/dependencies using an approved environment. Run the repository
   portable gate and affected shared tests, `:previewApp:test` and Android build.
   Record exact nonempty selections/toolchains. Capture and inspect actual
   320/390 dp vi/en, light/dark/high-contrast, large-text and long-profile cases,
   including neutral/managed-avatar owner mismatch, route/Back, focus/keyboard,
   duplicate actions, cancellation, late completions and unresolved sign-out.
   The standalone coroutines-1.8.0 JVM run does not validate Gradle's pinned 1.11.0.
2. **Prove native presentation/lifecycle.** Build Android and iOS in suitable
   environments; execute touch, safe-area/reflow, foreground/background,
   screen-reader and disposal/restoration checks. Saved routes must resolve
   current state without private restoration. Desktop Compose is shared-shell
   evidence, not TalkBack/VoiceOver or actual device lifecycle. Native gameplay
   remains the independent #81 task.
3. **Review the actual native auth contract before implementation.** Keep Kanidm
   and ADR-0031; define exact trusted origins, system-browser OIDC/callback
   binding, typed current-subject/version validation, credential lifetime and
   platform secure storage. UI accepts no raw redirect/code/token. Establish
   durable current-device logout, expiry/revocation and safe account-switch
   behavior. The current in-memory unresolved-logout quarantine cannot prove
   secure-store deletion or process-restart suppression; the adapter must
   reconcile unresolved revocation before later bootstrap reveals identity.
4. **Deliver and test a bounded real native account adapter.** With the approved
   contract and authorized disposable provider/server/device prerequisites,
   exercise actual login/callback/deep-link, secure-store behavior, permitted
   self profile, explicit sign-out, interruption/cancellation, stale/duplicate
   callback, expiry, port replacement and process restart. Future enabled forms
   need native labels/help/error association, IME, autofill/password-manager and
   accessibility acceptance. No provider onboarding, account/client/grant
   provisioning or production activation is implied by this backlog.
5. **Review native social separately.** After current native account authority
   exists and native social scope is explicitly opened, reuse ADR-0044's v2
   permitted profile/search/request/snapshot contracts through a narrow client
   owner. Establish current-viewer/target disclosure, participant-only decisions,
   revision/idempotent unknown-result recovery and timestamped Unknown/Online/
   Offline/Stale observations across replacement/resync/lifecycle. Real
   permission/transport acceptance is required; a signed-in badge grants no
   friendship, presence, room, match or voice rights. No new backend is needed
   merely because native delivery is absent.

## Acceptance / risks / non-goals

Keep source, standalone JVM, pinned build, Compose interactions/pixels, actual
provider/session and device evidence separate in the existing ledger. Issue
#103's current UI tests must pass before completion; future integration tests
remain conditional on the actual native adapter. Do not expand the UI draft to
erase a blocker or treat missing/private profile/social data as a successful
empty result. A generic error string alone does not establish anti-enumeration.

This deferred item does not reorder the active queue or authorize the later
implementation. No new provider, credential policy, reset/email-verification
flow, server migration/activation, fabricated profile/history/presence, avatar
URL/upload, native GameHost change, matchmaking/rooms, deployment, store release
or phase exit is included.
