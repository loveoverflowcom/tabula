#!/usr/bin/env python3
"""Keep generated review evidence out of Git without excluding runtime assets.

The docs evidence/design namespaces contain Markdown records and one intentional
CI token contract. Test fixtures and reproducible runtime art belong with their
source owners, not in evidence folders. Inspect Git's index, including force-added
files, rather than the working tree so ignored local output remains usable.
"""
from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TOKEN_CONTRACT = 'docs/ui/tokens.json'


def forbidden(path: str) -> bool:
    """Only constrain the two documentation namespaces, never game/app assets."""
    normalized = PurePosixPath(path).as_posix()
    in_docs = normalized.startswith(('docs/verification/', 'docs/ui/'))
    return in_docs and PurePosixPath(normalized).suffix != '.md' and normalized != TOKEN_CONTRACT


def check(root: Path) -> list[str]:
    tracked = subprocess.check_output(['git', 'ls-files', '-z'], cwd=root)
    return sorted(path for path in tracked.decode().split('\0') if path and forbidden(path))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    failures = check(args.root)
    if failures:
        print('Generated evidence/design files are tracked:')
        print('\n'.join(f'  {path}' for path in failures))
        print('Keep Markdown summaries; put raw output in ignored verification/ or Actions Artifacts.')
        return 1
    print('PASS repository hygiene: documentation records/token contract only; runtime assets unrestricted')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
