# Source and evidence retention

Keep the source tree reproducible and lightweight. `docs/verification/` and `docs/ui/`
retain human-written Markdown summaries, acceptance/requirements, commands, prompts
and provenance. `docs/ui/tokens.json` is the deliberate generated exception: it is
an adapter of `tokens.toml` checked for freshness by CI. Do not hand-edit it.

Do not track screenshots, JPG/PNG/SVG previews, HTML/CSS/MJS design bundles, ZIP
exports, test logs, coverage, browser captures or generated JSON/XML/GZ receipts in
those documentation directories. Use ignored `verification/` output, existing
ignored build targets or GitHub Actions Artifacts. Keep source/build/harness identity,
commands, selected cases and limitations in the lightweight summary. Link verified
workflow artifacts when available; do not invent links or equate captured pixels,
compilation, mockups or historical PASS with current runtime/device acceptance.

## Preserve actual inputs

Runtime and authoring files under `games/<game>/assets/`, used brand icons/logos,
Android/iOS resources and the Gradle wrapper JAR remain tracked. Never globally ignore
`*.png` or `*.svg`. Test fixtures, contracts and generators stay with their source
owner outside the documentation evidence namespaces. In particular, Werewolf's
`games/werewolf/assets/budgets.json` and `source/design-provenance.json` preserve the
existing byte/hash budget oracle and artwork attribution; they are not run receipts.
Generated design tokens remain required CI inputs.

## Historical archive

The cleanup is a normal recoverable Git change. It does not rewrite history, purge
other evidence branches or remove runtime assets. Raw evidence and design exports
remain available in the pinned
[pre-cleanup source tree](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7).
Archived records retain their original source/build scope and limits. Reproduce old
experiments from that exact revision or restore their input receipts outside Git;
new local output defaults to ignored `verification/` paths. A checkout of current
source alone does not contain the archived screenshots or old runtime results.

## Check before committing

Run `python3 tools/check-repository-hygiene.py` and
`python3 -m unittest discover -s tools/tests -p test_repository_hygiene.py -v`.
The guard examines the Git index, so `git add -f` cannot silently bypass the policy.
CI runs both alongside the existing structural checks. Ignored local output is
usable; fixture/source exceptions should live with their owner rather than weakening
this boundary.
