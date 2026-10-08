#!/usr/bin/env bash
# Resolve the nested Gradle workspace from this file, independent of the caller's cwd.
set -euo pipefail
mobile_workspace="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../apps/mobile" && pwd)"
action="${1:-run}"
if [[ $# -gt 0 ]]; then shift; fi
cd "$mobile_workspace"
case "$action" in
  run) exec ./gradlew --console=plain :previewApp:hotRun "$@" ;;
  mcp) exec ./gradlew --no-daemon --quiet --console=plain :previewApp:hotMcpServer "$@" ;;
  tasks) exec ./gradlew --console=plain :previewApp:tasks --all "$@" ;;
  test) exec ./gradlew --console=plain :previewApp:test "$@" ;;
  *) echo 'Usage: mobile-preview.sh {run|mcp|tasks|test} [Gradle options]' >&2; exit 2 ;;
esac
