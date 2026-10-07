# ADR-0045: Registry-backed discovery in the CMP shell

- **Status:** accepted bounded implementation requested by [issue #102](https://github.com/loveoverflowcom/tabula/issues/102).
- **Date:** 2026-10-07
- **Extends:** ADR-0032's shell/navigation foundation and ADR-0028's public discovery slice. ADR-0043's unavailable native gameplay boundary remains.
- **Invariants:** I-1, I-5/I-6, I-9, I-10, I-13 and I-15 unchanged.

## Decision

The existing Android/iOS CMP app may implement Home, searchable/filterable Library,
game detail and a read-only setup review using the public registry discovery
inventory. This is the owner's second mobile parity slice, following #101. It
does not open Phase 6, native accounts, remote discovery, online play or resume.

The registry remains the authority for identity, translated name/description,
categories, tags, seats, estimates, complexity, rules resources and setup
descriptors. A checked-in Kotlin adapter is generated from the existing read-only
discovery interface. The portable gate checks its freshness. CMP filters those
public facts; it neither branches on a game identifier nor imports rules or
canonical state. Cover data is lightweight discovery artwork, separate from
runtime packs. Browsing, searching and opening detail/setup mount no GameHost and
load no gameplay asset pack.

The public catalog and the host's packaged launch inventory are separate inputs.
A catalog entry is never evidence that native gameplay is available. Production
entrypoints retain ADR-0043's unavailable host and empty runtime inventory. They
can browse real registry entries and inspect setup, while local start stays
disabled with a visible explanation. Online/account-dependent modes likewise
show their unsupported state. There is no fake population, rating, bot readiness,
resumable match or successful launch.

The existing packaged host binding and launch preferences remain unchanged at the
explicit setup handoff. Setup descriptors are read-only until a native configuration adapter can
honor edited values. Desktop tests supply named catalog/host doubles to exercise
that handoff and host-first Back. Such tests establish shell interaction only.
Restoration retains public navigation/filter preferences, never a live match.

## Design and evidence boundary

The shell uses the generated semantic tokens and the same purple, warm paper,
tonal grouping, card artwork, neutral avatar and vi/en terminology as the web
discovery surface. Layout adapts to phone widths, scrolling and large fonts.
Loading, empty, unavailable, error/retry and no-results are distinct. Retry is
offered only when an actual caller supplies recovery; an absent service cannot
silently return fixture data.

The [issue ledger](../verification/issue-102-mobile-discovery/README.md) records
catalog/query checks, measured layout and actual shared CMP screenshot cases,
plus exact toolchain limitations. Desktop captures do not establish Android/iOS
touch, screen-reader completion, native rendering or device lifecycle/performance.
Native adapter/assets acceptance remains issue #81 under ADR-0043. No wire type,
rule, server authority or unsafe-code policy changes here.
