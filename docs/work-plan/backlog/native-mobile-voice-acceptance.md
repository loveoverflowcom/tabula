# Native mobile voice target acceptance

## Outcome

Establish real two-client native voice plus concurrent packaged game audio on
Android and iOS for ADR-0037, then decide whether this client slice can ship.

## Why / boundary

Shared controller/CMP tests and SDK compilation cannot prove a microphone,
WebView/native audio coexistence, background capture release, headset routing or
SFU permission enforcement. Keep target proof separate from production backend
grant/provider/membership/revocation work and broad Phase 6/8 exits.

## Dependencies / next check

Use an authorized device/emulator and simulator/Mac environment, official ephemeral
local LiveKit SFU, two unique synthetic identities and short-lived debug fixtures.
Follow the [harness](../../../tools/native-voice-harness/README.md) and
[ledger](../../verification/native-mobile-voice/README.md). No production secrets,
OAuth grants, account activation, hosted SFU or deployment are implied.

## Acceptance / risks

Record real bidirectional audio, OS permission denial, game audio coexistence,
WebView reload, network loss/reconnect, stale join/permission/mic completion,
repeated actions, route leave/logout hook/capture-resource cleanup, headset
changes and background/foreground behavior on each target. Capture actual SDK,
SFU, OS/device/build versions and resource evidence. Resolve failed ICE routing
rather than declaring success from signaling or UI state.
