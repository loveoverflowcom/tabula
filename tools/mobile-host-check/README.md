# Historical mobile WebView check

This harness drove the ADR-0033 staged web document in desktop Chrome with a native
bridge stand-in. Its results are retained in
[the historical ledger](../../docs/verification/mobile-game-host/README.md).

ADR-0043 retires mobile WebView gameplay and `stage-mobile-game`; this harness is
not a current native mobile acceptance path. Do not recreate the retired mobile
web package to claim #81 passes. Web loader/bridge tests remain in
`apps/game-client/web/tests/` for their existing web consumers. Native implementation
and device evidence are tracked in
[the new ledger](../../docs/verification/mobile-native-host/README.md).
