# Isolated account-state and self-profile delta

Issue #54 PR3, authorized by [ADR-0036](../../adr/0036-isolated-durable-session-validation.md),
stacked on PR2 `3e539bfe`. This bounded slice does not open production authentication,
provider registration, social services or the wider Phase 4/5 gates.

## Design before implementation

The static reference is `030da25d0098e240ab2cf36dacf9892e8b320a89`,
`06-accounts/screens/{15-login,16-register,20-profile,21-friends,mobile-states}.svg`.
The supplied Login/Profile/mobile PNG pixels were inspected. Register and Friends
SVGs were rasterized with the already available Sharp module and those pixels
were inspected too. These are reference-art inspections, not product screenshots.
The [foundation](foundation.md), accounts/social contract, four screen specs and
shared material-component map governed this adaptation.

Use the existing toolbar, generated four-scheme tokens, sans task heading,
tonal status/error/contained rows and native labelled buttons/links. Keep one
bounded task column rather than the reference's large promotional hero. Remove
prototype game art, sample names/handles, avatar initials, editing, privacy
choices and unsupported metric rows. An immutable ID is the only returned
self-profile fact. IDs wrap in a readable mono field; no ID is announced through
an error/status live region. Compact actions stack and grow with translated copy.

## Implemented route and interaction boundary

- `/account` and `/me` check current context then a matching authorized self
  profile through the isolated adapter. Pending, signed out, previous session
  no longer confirmed, unavailable, disconnected, generic error, cancellation
  and uncertain logout are separate presentations. A signed-out response after
  prior authentication does not identify whether expiry or revocation caused it.
  Ready copy describes the latest successful check, not continuous authority
- `/login`, `/register`, `/friends` explain their unavailable capabilities.
  They collect no credentials, create no accounts and show no mock friend data
- `/u/:handle` never falls back to self, resolves no handle and confirms no other
  account's existence. No other-profile adapter exists in PR2
- Escapes are fixed `/games`, `/account` or `/` links. Incoming return/redirect
  query parameters are deliberately ignored. No history-back, encoded return
  parser, arbitrary URL, implicit mutation or automatic game launch is enabled
- Account/profile output is selected only for idle authenticated authority and
  keyed by adapter presentation generation plus immutable ID. The adapter's
  synchronous lifecycle mask targets `data-account-private` output; the key
  creates new nodes after a successful recheck instead of unmasking old nodes
- Logout asks in a nonmodal in-document fieldset. Focus moves to Cancel;
  Cancel/Escape dismiss the local prompt and restore its invoker. Confirmation
  moves focus to the persistent heading before retiring the private task.
  Explicit check/refresh/retry also use that persistent successor before their
  initiating controls disappear or become disabled; async errors do not steal focus.
  Cancellation of a dispatched operation only retires local completion effects
- Uncertain logout keeps account data hidden and offers explicit session recheck
  and sign-out retry. It never claims durable revocation from local cleanup or
  transport failure. A changed or unmatchable context keeps account data hidden
  and withholds sign-out retry, so the earlier action cannot revoke a replacement
  session. Other tabs sharing the cookie may lose account access

Locale and theme remain shared shell concerns. Ordinary arrival focuses the
heading; async generic errors use a persistent alert and pending/current state a
separate polite status. Private profile data is outside those live regions.

## Evidence limits

Pure presentation selection/localization tests, static component HTML assertions,
compilation and source inspection are distinct from browser execution. Real
320/390 dp reflow, zoom, native keyboard activation/focus, BFCache first-restored
pixels, DOM accessibility snapshots, assistive technology, IME and password-manager
behavior require target-specific receipts. Previously denied cloud-loopback
browser QA and unavailable native display routes remain unrun, not bypassed.
PR2 provides neither session deadlines nor a live revocation stream. Immediate
autonomous expiry clearing and missed-signal account-change detection while the
page remains visible still require a production lifecycle contract and evidence;
this UI does not claim continuously verified authority.
There are no credential inputs in this slice, so it proves no provider login,
password/autofill interaction or signup acceptance.
