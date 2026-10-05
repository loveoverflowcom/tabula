# Isolated native voice harness

This is an explicit local development harness, not production auth, a token
service, a custom SFU or a substitute for actual two-client audio evidence.
It runs an **already installed official `livekit-server`**, creates a fresh random
signing key in a private temporary directory, and issues ten-minute audio-only
grants for `client-a`/`client-b` in the fixed public synthetic room
`tabula-native-voice-dev`. No hosted account, production grant, persistent external
credential or deployment is created. Shutdown removes temporary keys/fixtures.

Official installation/setup reference:
https://docs.livekit.io/transport/self-hosting/local/

## Run on the machine hosting your simulator/emulators

```sh
python3 tools/native-voice-harness/run.py                     # iOS simulator localhost
python3 tools/native-voice-harness/run.py --android-emulator  # Android emulator 10.0.2.2
```

Missing official server or failed startup is a failure/blocker, never a skipped
integration pass. This script does not install software or create a listener
until invoked in an environment where that action is allowed. Signaling binds to
host loopback. The emulator address is advertised for its NAT path; that route
still needs real ICE/media acceptance and is not assumed proven by config.

While the harness runs, copy one of its private `client-a.json`/`client-b.json`
files to ignored `mobile/voice-dev-grant.json`, then build/install a **Debug** app
on the corresponding client. Repeat with the other identity for client B. Do
not reuse one identity for both clients: LiveKit would replace the old participant.
The fixture expires within ten minutes of generation; regenerate deliberately
for later tests. Do not commit it, put it in game assets or print a token.

Android packages the optional file in debug assets only; native code requires a
debuggable build. Its debug cleartext exception covers only the three exact local
hosts. iOS’s Debug-only copy phase removes stale fixture resources from Release;
Swift Release does not read fixtures. Release always fails closed as unavailable.
Removing the source also clears a previous generated debug resource.

Use the host’s **Join voice**, then **Turn mic on** and answer the native OS
permission prompt. Join starts receive-only. Native room credentials are never
supplied to game JS. Leave voice preserves the game; confirmed route leave,
background entry, logout hook/deadline/interruption and disposal stop the native
room. There is no automatic microphone resume.

## Required real acceptance, still outstanding

Record exact SDK/server/build/device versions and two unique clients. Verify
both real audio directions, mic-on/off, listen-only token rejection at the SFU,
permission denial/retry, concurrent game sound, WebView reload while voice stays
connected, transport loss/reconnect, rapid join/leave and stale permission/join,
logout/route cleanup, capture indicators, headset connect/unplug, real background
cleanup and explicit foreground rejoin. Inspect native capture/network resources,
not only button labels. This harness has no production account/session revocation
integration or secret-channel policy. Do not claim Werewolf enforcement from it.

```sh
python3 tools/native-voice-harness/test_run.py
```

Those tests prove synthetic JWT shape/signature and private fixture-file behavior;
they do not connect any SDK/SFU or exercise an OS microphone.
