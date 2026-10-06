# Native Macroquad embedding: pinned source review

Reviewed for issue #81 against `develop@c6d55a6`. Evidence kind: **source-read**.
No native game was compiled, linked, rendered or benchmarked by this review. The shipping
Android/iOS adapter is **NOT_IMPLEMENTED**; native embedding acceptance is **BLOCKED** by
the missing adapter. CMP desktop preview can establish shell layout only.

## Artifact identity

`Cargo.lock` selects these published crates:

| Crate | Version | Published `.crate` SHA-256 from lockfile and local archive |
|---|---|---|
| Macroquad | 0.4.16 | `1cbd54d99d4d3ac59deed7ee4927bbbf4eb33a55635530af3be9395a55746090` |
| Miniquad | 0.4.11 | `dcbaea978d741f3b60ced666b06a2a1e4a2bc81cb8b9dd29f4a40965e1f59e65` |

The local archives were hashed and matched their exact lockfile checksums. The review compared
unpacked `src/lib.rs` and Miniquad's
`src/native/android.rs` / `src/native/ios.rs` byte-for-byte with their archive entries: all
matched. No cache patch or repository fork was present.

Exact provenance check (Python standard library; local Cargo cache only):

```python
import hashlib, pathlib, tarfile
cache = pathlib.Path.home() / ".cargo/registry"
index = "index.crates.io-1949cf8c6b5b557f"
for crate in ["miniquad-0.4.11", "macroquad-0.4.16"]:
    archive = cache / "cache" / index / f"{crate}.crate"
    print(crate, hashlib.sha256(archive.read_bytes()).hexdigest())
    with tarfile.open(archive) as package:
        for source in ["src/native/android.rs", "src/native/ios.rs", "src/lib.rs"]:
            local = cache / "src" / index / crate / source
            if local.exists():
                assert local.read_bytes() == package.extractfile(f"{crate}/{source}").read()
                print(source, "matches published archive")
```

The crates' `.cargo_vcs_info.json` records Macroquad revision
`5e9b5ca912ac65962c05c0da842a4a70eaae34b9` and Miniquad revision
`4f13d4a70caf6a8cbf0187a95461658e7812b566`. Those upstream Git revisions provide stable
context links, but the Miniquad Git source differs from the published crate source. In
particular the published Android implementation uses `set_or_replace_display`, while
the linked Git revision uses `set_display`. Therefore **published archive content and
checksum are the pin; Git line numbers must not be substituted for the archive line numbers**.

## Entry points and owners

Line ranges below refer to the published crates identified above, not to repository files.

| Owner | Source and exact location | Finding |
|---|---|---|
| Macroquad runtime | `macroquad-0.4.16/src/lib.rs:937–979`, `158–173`, `484–505` | `Window::from_config` calls `miniquad::start`; creates the Macroquad global context and stage on the backend thread. There is one mutable global context and a thread identity assertion. It is not an independent per-view runtime. |
| Miniquad platform bootstrap | `miniquad-0.4.11/src/lib.rs:461–514` | `start(Conf, FnOnce() -> Box<dyn EventHandler>)` dispatches to each platform's `run`; it accepts no externally owned surface/controller or teardown handle. |
| Android glue | `java/QuadNative.java:10–23`, `java/MainActivity.java:42–137`; `src/native/android.rs:553–643` | Java surface, lifecycle and touch callbacks enter the Rust backend. Native window acquisition uses the Java `Surface`. The render loop is on a Rust thread. |
| Android recreation | `src/lib.rs:93–111`; `src/native/android.rs:144–180`, `185–259`, `397–524` | The published crate can replace the display and recreate an EGL surface while retaining the handler. This capability is narrower than safe disposal followed by a new runtime. |
| iOS bootstrap | `src/native/ios.rs:491–616`, `843–865` | `run` installs arguments and calls `UIApplicationMain`. Its app delegate creates the window, native view/controller and request loop, then makes that window key. The current SwiftUI/CMP app already owns this bootstrap. |
| iOS view access | `src/lib.rs:433–440` | `apple_view` and `apple_view_ctrl` expose an already-created view/controller. They are getters after backend initialization, not an API to create an embedded controller without `UIApplicationMain`. |

Stable upstream context: [Macroquad context/bootstrap](https://github.com/not-fl3/macroquad/blob/5e9b5ca912ac65962c05c0da842a4a70eaae34b9/src/lib.rs),
[Miniquad bootstrap](https://github.com/not-fl3/miniquad/blob/4f13d4a70caf6a8cbf0187a95461658e7812b566/src/lib.rs),
[Android backend](https://github.com/not-fl3/miniquad/blob/4f13d4a70caf6a8cbf0187a95461658e7812b566/src/native/android.rs),
[Android surface glue](https://github.com/not-fl3/miniquad/blob/4f13d4a70caf6a8cbf0187a95461658e7812b566/java/MainActivity.java),
[iOS backend](https://github.com/not-fl3/miniquad/blob/4f13d4a70caf6a8cbf0187a95461658e7812b566/src/native/ios.rs).

## Concrete constraints

Android `send_message` uses a thread-local sender and unwraps both sender lookup and send
(`android.rs:75–83`). Callbacks must remain on the owning UI thread and cannot safely arrive
after the receiver has closed. `Destroy` sets quit, but the spawned thread's handle is discarded
(`249–252`, `405–524`): there is no completion barrier before reopening. The initial wait for
surface/size ignores other messages (`410–427`), including cancellation before a surface
exists. Pause forwards a minimized event (`236`) while `frame` still performs update
(`257–269`). The source itself documents incomplete JNI thread detach (`302–325`). These
are source-read risks, not observed device failures.

iOS has no supported externally owned child/controller bootstrap in this version. Its request
thread uses an unconditional loop (`ios.rs:616–684`) even though `Destroy` can set a quit
field (`209–213`). Drawing dimensions derive from `UIScreen` (`323–357`), requiring an
adapter to resolve embedded view bounds and safe areas. The issue therefore cannot be accepted
by linking a static library and calling `start` from the existing CMP game screen.

The workspace forbids unsafe code (`Cargo.toml`, ADR-021, AGENTS.md §8); this PR adds no FFI,
exported symbols or exception. Existing unsafe code inside upstream dependencies does not
authorize copying it into Tabula. A future thin ABI needs its ownership/error contract reviewed
before implementation, including panic containment; game/core crates remain forbidden from
platform dependencies (I-1, I-11).

## Smallest adapter proposal and remaining proof

This is a proposed implementation boundary, not code delivered by PR1:

1. Android: one `SurfaceView` behind `GameHost`, using the upstream callback shape. A pinned
   backend extension must add bounded cancellation before surface creation, callback generation
   fencing, explicit stop/join acknowledgement, JNI/window reference release, and suspended
   frame scheduling. Never start a second Macroquad context until the first worker has exited.
2. iOS: extract native view/controller creation from app bootstrap in a narrowly scoped,
   pinned Miniquad adapter. CMP continues owning `UIApplication` and navigation; the adapter
   owns its controller/context and an explicitly stoppable request loop. Prototype a child
   controller before deciding whether a full native game controller is required.
3. Keep launch/preferences/back/lifecycle/events as the small bridge. Rust keeps the local
   authority, projection, presenter, asset validation and render loop. Neither `RenderList`
   nor canonical state crosses into Kotlin/Swift (I-5/I-6/I-10).
4. CPU preparation and bounded cache can precede the surface. GPU upload follows context
   creation on its owner thread. Surface loss does not construct another match. Process death
   follows existing unsaved-local-match semantics; it must not imply durable recovery.

Before shipping either adapter: exercise in-app CMP → gameplay → CMP; cancellation before
first surface; touch cancel; resize/DPI; background without render work; surface loss/recreation;
repeated disposal/reopen; stale callbacks; real preload failure/cancellation. Record target build,
device/OS/backend and cache/asset state for usable-frame latency, frame distribution, input
latency and CPU/RAM. A standalone app, successful compilation, simulator or desktop screenshot
does not complete the required real-device evidence. No 60/120 FPS claim is made.
