# Issue-60 measured observations

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


Generated from the retained receipts by `python3 tools/renderer-embedding-spike/summarize.py`. [Historical measurements.json](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/measurements.json) retains unrounded values, input SHA256 fingerprints and resource entries. The [execution ledger](README.md) owns commands, the [Historical final source manifest](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/source-manifest.json), build fingerprints, failures and target limitations.

These are local observations on the Mac and Chromium configuration in [Historical environment.json](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/environment.json). All static rows use the 24-command #59 permitted checkpoint, 900×720 logical viewport, DPR1, light/full motion, 3,000 ms warm-up and 300 samples. Native is a separate control. No engine or containment winner is inferred.

## Clock and cache boundaries

Observer ready starts at the browser runner before navigation and ends when it receives the Rust console receipt; native starts at process launch and ends at the stdout receipt. Host mount ready starts inside `mount()` after the shell has loaded and ends at its ready resolution. It includes initialization and the initial Rust view but does not guarantee a draw of the settled revision. The first usable/presented-frame startup timestamp is not separately measured. Macroquad's preload clock covers its verified atlas preload, not complete startup. These clocks must not be subtracted to estimate iframe overhead.

Browser run 1 in each path has a fresh browser process and renderer instance; runs 2–3 reuse that browser/context with a new document/renderer. Disk, OS and graphics-driver caches are not purged. Resource Timing supplies the observed transfer sizes. Native runs each use a fresh process. The [historical pre-freeze Pixi run 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/pre-freeze-static/chromium-pixi-1.json) overlapped compilation on the shared host, so it is excluded from the final frozen-source timing cells. The final receipts were refreshed after compilation and source freeze; their input SHA256 fingerprints are in the machine table. Historical samples must not be reinterpreted as contention-free.

| Receipt | Observer ready ms | Host mount ready ms | Atlas preload ms | Cache class |
|---|---:|---:|---:|---|
| [Historical document 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-1.json) | 98.000 | — | 9.000 | fresh browser + instance |
| [Historical document 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-2.json) | 56.000 | — | 9.000 | same browser; new document + instance |
| [Historical document 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-3.json) | 55.000 | — | 10.000 | same browser; new document + instance |
| [Historical iframe 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-1.json) | 166.000 | 79.800 | 8.000 | fresh browser + instance |
| [Historical iframe 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-2.json) | 106.000 | 77.100 | 9.000 | same browser; new document + instance |
| [Historical iframe 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-3.json) | 103.000 | 75.500 | 9.000 | same browser; new document + instance |
| [Historical pixi 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-1.json) | — | 71.200 | — | fresh browser + instance |
| [Historical pixi 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-2.json) | — | 64.800 | — | same browser; new document + instance |
| [Historical pixi 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-3.json) | — | 95.900 | — | same browser; new document + instance |
| [Historical native 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-1.json) | 354.198 | — | 1.366 | fresh native process |
| [Historical native 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-2.json) | 99.778 | — | 1.348 | fresh native process |
| [Historical native 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-3.json) | 102.128 | — | 1.404 | fresh native process |

## Drawing and frame intervals

Macroquad reports its submit/end CPU timer, converted from µs to ms here, and excludes final frame flush. Pixi reports scene reconstruction plus `app.render`. Frame intervals describe pacing between sampled frames, not GPU completion. The methods, pacing, browser/native targets and sample-window durations differ; their values do not support an engine CPU ranking.

| Receipt | Method mean / p50 / p95 / max ms | Interval mean / p50 / p95 / max ms | Samples |
|---|---|---|---:|
| [Historical document 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-1.json) | 0.990 / 1.000 / 2.000 / 3.000 | 27.110 / 27.000 / 29.000 / 109.000 | 300 |
| [Historical document 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-2.json) | 1.083 / 1.000 / 2.000 / 2.000 | 27.157 / 27.000 / 29.000 / 109.000 | 300 |
| [Historical document 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-3.json) | 0.970 / 1.000 / 2.000 / 2.000 | 27.010 / 27.000 / 28.000 / 107.000 | 300 |
| [Historical iframe 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-1.json) | 1.047 / 1.000 / 2.000 / 2.000 | 27.123 / 27.000 / 29.000 / 139.000 | 300 |
| [Historical iframe 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-2.json) | 1.077 / 1.000 / 2.000 / 2.000 | 27.147 / 27.000 / 29.000 / 110.000 | 300 |
| [Historical iframe 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-3.json) | 0.980 / 1.000 / 2.000 / 2.000 | 27.030 / 27.000 / 29.000 / 110.000 | 300 |
| [Historical pixi 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-1.json) | 1.351 / 1.300 / 1.600 / 1.900 | 12.300 / 10.000 / 20.100 / 21.000 | 300 |
| [Historical pixi 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-2.json) | 1.333 / 1.300 / 1.500 / 1.900 | 12.337 / 10.000 / 20.000 / 21.000 | 300 |
| [Historical pixi 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-3.json) | 1.351 / 1.300 / 1.600 / 2.200 | 12.302 / 10.000 / 20.100 / 21.000 | 300 |
| [Historical native 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-1.json) | 25.953 / 25.780 / 27.019 / 32.927 | 26.442 / 26.275 / 27.483 / 33.444 | 300 |
| [Historical native 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-2.json) | 26.462 / 26.536 / 27.108 / 29.682 | 26.941 / 27.011 / 27.580 / 30.162 | 300 |
| [Historical native 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-3.json) | 26.629 / 26.697 / 27.197 / 29.017 | 27.108 / 27.156 / 27.682 / 29.481 | 300 |

## Input and interop

The iframe ping is host→child→host message task dispatch only: 100 samples per run, no Rust input or paint. Coarse timer resolution produces zero median samples. It is neither input feedback nor match latency.

| Receipt | Ping mean / p50 / p95 / max ms | Samples |
|---|---|---:|
| [Historical iframe 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-1.json) | 0.022 / 0.000 / 0.100 / 0.300 | 100 |
| [Historical iframe 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-2.json) | 0.022 / 0.000 / 0.100 / 0.300 | 100 |
| [Historical iframe 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-3.json) | 0.025 / 0.000 / 0.100 / 0.300 | 100 |

[Historical Chromium interaction receipt](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-interactions.json) overall status: **PARTIAL**. Its accepted-input check is listed independently below; a passed subcheck does not turn a partial or failed overall receipt into PASS. Programmatic typed input timing runs through native loopback HTTP, Rust presenter/rules, returned-view validation and the first draw callback for that exact revision. It excludes physical input-device latency and display presentation. With only the recorded inputs, no p95 distribution or production SLA is justified.

| Input | Outcome | Revision | Accepted commands | Round trip to exact-revision draw ms |
|---|---|---:|---:|---:|
| Tab | local | 2 | 24 | 14.400 |
| Enter | accepted | 3 | 25 | 8.800 |

Static Pixi drawing repeats one Rust view without per-frame HTTP. The raw `authority_operations` retain actual native HTTP/view-update durations; Resource Timing retains each `/authority` response body size. Rust lowering, serialization, transport and JS validation were not timed separately, and request/copy sizes were not separately measured. No deployable Rust/WASM FFI cost follows from this native shim.

## Process CPU and memory

Each row uses the first and last retained `ps` sample, not exact workload start/end. CPU is cumulative-seconds difference divided by elapsed sample-window seconds (100% = one CPU core equivalent); it is not macOS Activity Monitor or GPU utilization. Browser rows include the launched browser process tree, including shell/renderer/utility/GPU processes, exclude exited children, and can change membership. RSS sums double-count shared pages. Native rows cover only that process. The windows differ and support no ranking. MiB is KiB / 1024.

| Receipt | ps samples | Window s | CPU Δ s | CPU one-core % | RSS first / last / max MiB | JS heap MiB | WASM linear MiB |
|---|---:|---:|---:|---:|---|---:|---:|
| [Historical document 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-1.json) | 11 | 10.007 | 2.740 | 27.381 | 898.469 / 907.125 / 950.656 | 2.910 | 8.938 |
| [Historical document 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-2.json) | 11 | 10.006 | 2.890 | 28.883 | 919.984 / 917.766 / 921.188 | 3.485 | 8.938 |
| [Historical document 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-3.json) | 11 | 10.004 | 2.710 | 27.089 | 925.297 / 924.266 / 926.609 | 2.932 | 8.938 |
| [Historical iframe 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-1.json) | 11 | 10.008 | 3.130 | 31.275 | 899.609 / 925.219 / 925.219 | 5.765 | 8.938 |
| [Historical iframe 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-2.json) | 11 | 10.009 | 2.940 | 29.374 | 936.547 / 931.750 / 936.547 | 5.825 | 8.938 |
| [Historical iframe 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-3.json) | 11 | 10.011 | 2.840 | 28.369 | 938.828 / 933.688 / 938.828 | 5.740 | 8.938 |
| [Historical pixi 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-1.json) | 7 | 6.007 | 3.620 | 60.263 | 974.734 / 1,093.922 / 1,093.922 | 12.743 | — |
| [Historical pixi 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-2.json) | 7 | 6.004 | 3.500 | 58.294 | 1,005.766 / 1,103.219 / 1,103.219 | 13.025 | — |
| [Historical pixi 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-3.json) | 7 | 6.006 | 3.510 | 58.442 | 1,011.328 / 1,109.922 / 1,109.922 | 15.369 | — |
| [Historical native 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-1.json) | 12 | 11.072 | 2.290 | 20.683 | 0.031 / 70.750 / 70.750 | — | — |
| [Historical native 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-2.json) | 12 | 11.021 | 2.280 | 20.688 | 1.875 / 69.484 / 69.484 | — | — |
| [Historical native 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/native-static-3.json) | 12 | 11.009 | 2.200 | 19.983 | 1.891 / 70.406 / 70.406 | — | — |

JS heap is one document observation, with child observations retained separately for iframe; it is not total process residency. Macroquad's atlas estimate is 2,488,320 RGBA bytes (two sources), while Pixi reports 46 region textures over two sources and one owned font. Heap/WASM/texture estimates do not establish GPU allocation, leaks or exact reclamation. No forced GC was used. The following lifecycle snapshots occur after disposal in 50 short rendered mounts. The host retains up to 100 diagnostic history snapshots. These observations show a short memory curve, not proof of process/GPU reclamation or absence of leaks.

| Lifecycle receipt | Completed | JS heap first / last / max MiB | Process-tree RSS first / last / max MiB | Observation window s |
|---|---:|---|---|---:|
| [Historical pixi cycles](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-cycles.json) | 50 | 9.152 / 11.221 / 12.005 | 886.672 / 959.906 / 959.906 | 2.385 |
| [Historical iframe cycles](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-cycles.json) | 50 | 3.453 / 2.914 / 6.472 | 891.750 / 1,022.703 / 1,022.703 | 4.128 |

## Observed downloads

These sums count the recorded document Resource Timing entries. Static sums exclude `/authority` and favicon; main navigation is not a Resource Timing entry. For iframe, the parent timeline does not include all child subresources, so its sum is incomplete for the embedded game. The child WASM-only transfer observation is retained in the machine table. Encoded body size can remain nonzero for cached responses; transfer size includes reported protocol overhead. Repeated authority requests are counted individually. The host serves local bytes; these values are not compressed production download size, network latency or a complete shipping footprint.

| Receipt | All entry transfer / encoded bytes | Static transfer / encoded bytes | Static zero-transfer entries | Authority responses |
|---|---:|---:|---:|---:|
| [Historical document 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-1.json) | 1,402,941 / 1,400,841 | 1,402,641 / 1,400,841 | 0 / 6 | 0 |
| [Historical document 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-2.json) | 300 / 1,400,841 | 0 / 1,400,841 | 6 / 6 | 0 |
| [Historical document 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-document-3.json) | 300 / 1,400,841 | 0 / 1,400,841 | 6 / 6 | 0 |
| [Historical iframe 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-1.json) | 442,844 / 438,644 | 442,544 / 438,644 | 0 / 13 | 0 |
| [Historical iframe 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-2.json) | 300 / 438,644 | 0 / 438,644 | 13 / 13 | 0 |
| [Historical iframe 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-iframe-3.json) | 300 / 438,644 | 0 / 438,644 | 13 / 13 | 0 |
| [Historical pixi 1](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-1.json) | 2,756,601 / 2,750,601 | 2,701,697 / 2,696,897 | 0 / 16 | 3 |
| [Historical pixi 2](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-2.json) | 54,904 / 2,750,601 | 0 / 2,696,897 | 16 / 16 | 3 |
| [Historical pixi 3](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/runs/chromium-pixi-3.json) | 54,904 / 2,750,601 | 0 / 2,696,897 | 16 / 16 | 3 |

Safari/WebKit has no runtime measurement: [Historical probe](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-60/safari-probe.json) records remote automation disabled and Playwright WebKit absent. Native pixels were not captured. Audio, physical higher-DPI/mobile/WebView execution, GPU completion timers, full accessibility action play and online handoff remain unmeasured. The separate Chromium DPR2 interaction is emulation, not a physical-display measurement.
