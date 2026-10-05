# Complete Werewolf local standalone

Status: implementation and feasible core/headless/build checks complete; real platform acceptance blocked. Priority numbers are sequence markers, not permanent IDs.

## Outcome / why

One genuinely playable ClassicV1 local round and terminal result, exercising isolated
per-seat private views, public view, approved card graphics and the existing renderer.
The current W1/W2-only skeleton cannot accept actions or resolve a match.

## Review boundaries

1. Pure referee and complete projection contract with semantic/privacy tests
2. Game-owned card/action presenter and reproducible standard pack
3. Opt-in native/WASM local host, interrupted/repeated flows, build and real target evidence

## Dependencies and risks

Use maintained W-D decisions and ADR-0035. Do not import the offline unpublished Mac branch
as if available. Do not promote screenshot/build evidence into online/privacy/platform proof.
Rollout remains disabled pending required gates. Next work after this slice is actual
human/social, online and voice acceptance under their phase prerequisites.
