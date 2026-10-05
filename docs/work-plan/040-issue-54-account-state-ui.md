# Issue #54 PR3 — isolated account-state and self-profile UI

**Status:** waits for PR2's verified completion; fresh work session required.

**Outcome:** compact M3 Expressive shell account-state/read-only self-profile
flows use the isolated current-authority HTTP adapter and shared foundation.

**Why:** the account specification is not an implemented UI; state/lifecycle
behavior must use authority, not saved mock login or CSS permission flags.

**Dependencies:** PR2 exact verified head; explicit stacked base if unmerged;
ADR-0031/0036, screens 15/16/20/21 and existing foundation/design references.
Login, registration and friends remain unavailable until their actual backend
contracts are ready. This does not close every #54 acceptance criterion.

**Review boundary:** design before changes; shared form/list/error components,
en/vi keys, pending/unavailable/expired states, safe return/local escape,
operation generations, stale responses and account-switch cleanup. Use real
HTTP/database integration where available and distinguish event/headless tests
from browser/IME/password-manager/AT/device receipts.

**Risks / unknowns:** real BFCache first-restored-frame and accessibility privacy
cannot be established by mocked events or no-store alone. Previously denied
runtime QA must remain unrun until an authorized supported route exists.

**Non-goals:** fake successful auth/registration/friendship, invented stats,
profile edits, invitation/room work under #55, new auth storage/providers,
production activation, voice, merging and broad phase exits.
