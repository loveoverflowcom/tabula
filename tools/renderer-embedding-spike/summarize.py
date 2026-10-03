#!/usr/bin/env python3
"""Derive issue-60 measurement tables from retained receipts; never run benchmarks."""
import argparse
import hashlib
import json
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def receipt_values(raw):
    return [(item.get("observed_ms", item.get("elapsed_ms")),
             item.get("value", item.get("receipt", {})))
            for item in raw.get("receipts", [])]


def stats_ms(values, divisor=1):
    if not values:
        return None
    return {key: value / divisor if key != "samples" else value
            for key, value in values.items()}


def process_summary(raw, native):
    samples = raw.get("process_samples", [])
    if native:
        parsed = []
        for item in samples:
            parts = item.get("ps_time_rss_kib_cpu", "").split()
            if item.get("ps_exit") != 0 or len(parts) != 3:
                continue
            seconds = 0.0
            for part in parts[0].split(":"):
                seconds = seconds * 60 + float(part)
            parsed.append({"elapsed_ms": item["elapsed_s"] * 1000,
                           "cpu_s_sum": seconds, "rss_kib_sum": int(parts[1])})
        samples = parsed
    if not samples:
        return None
    first, last = samples[0], samples[-1]
    window = (last["elapsed_ms"] - first["elapsed_ms"]) / 1000
    delta = last["cpu_s_sum"] - first["cpu_s_sum"]
    return {"samples": len(samples), "first_elapsed_ms": first["elapsed_ms"],
            "last_elapsed_ms": last["elapsed_ms"], "window_s": window,
            "cpu_s_first": first["cpu_s_sum"], "cpu_s_last": last["cpu_s_sum"],
            "cpu_s_delta": delta,
            "cpu_percent_one_core_equivalent": 100 * delta / window if window else None,
            "rss_kib_first": first["rss_kib_sum"], "rss_kib_last": last["rss_kib_sum"],
            "rss_kib_max": max(s["rss_kib_sum"] for s in samples),
            "scope": "native process" if native else "launched Chromium process tree",
            "caveat": ("Sampled ps cumulative CPU delta, no GPU timer; native process RSS only."
                       if native else "CPU sum excludes exited children and may change membership; includes browser, shell, utility and GPU processes. RSS sum double-counts shared pages; not unique residency or GPU allocation.")}


def browser_resources(raw):
    resources = raw.get("observation", {}).get("resources", [])
    static = [item for item in resources
              if item["name"] not in ["/authority", "/favicon.ico"]]
    dynamic = [item for item in resources if item["name"] == "/authority"]
    return {"entries": len(resources),
            "all_transfer_bytes_sum": sum(i["transfer_bytes"] for i in resources),
            "all_encoded_body_bytes_sum": sum(i["encoded_bytes"] for i in resources),
            "static_entries": len(static),
            "static_transfer_bytes_sum": sum(i["transfer_bytes"] for i in static),
            "static_encoded_body_bytes_sum": sum(i["encoded_bytes"] for i in static),
            "static_zero_transfer_entries": sum(i["transfer_bytes"] == 0 for i in static),
            "authority_responses": dynamic,
            "resources": resources,
            "scope": "Observed document Resource Timing entries, excluding main navigation; iframe child subresources are not all present in the parent timeline. Encoded body sizes include cached payloads and repeated requests; transfer sizes include reported protocol overhead. No shipping bundle or compressed-production-download claim."}


def summarize(path, kind, run):
    raw = json.loads(path.read_text())
    native = kind == "native"
    values = receipt_values(raw)
    ready_at, ready = next(((at, value) for at, value in values if value.get("kind") == "ready"), (None, {}))
    measurement = next((value for _, value in values if value.get("kind") == "measurement"), {})
    mounted = raw.get("mounted") or {}
    final = raw.get("final") or {}
    observations = final.get("observations", [])
    child = [item["value"] for item in final.get("metrics", []) if item.get("name") == "runtime"]
    wasm = (observations or child or [{}])[-1]
    controller = final.get("controller") or {}
    return {"source": f"runs/{path.name}", "source_sha256": digest(path),
            "status": raw["status"], "kind": kind, "run": run,
            "browser": raw.get("browser"), "cache": raw["cache"],
            "viewport": raw.get("viewport", ready.get("viewport")),
            "dpr": raw.get("dpr", ready.get("dpi")),
            "theme": final.get("theme", ready.get("theme")),
            "motion": "reduced" if final.get("preferences", {}).get("reduced_motion") else ready.get("motion", "full"),
            "scenario": raw.get("scenario", ready.get("scenario")),
            "samples": raw.get("samples", measurement.get("samples")),
            "checkpoint": final.get("checkpoint", measurement.get("final_checkpoint")),
            "uncontrolled_input_events": controller.get("inputs", measurement.get("uncontrolled_input_events")),
            "warmup_ms": raw.get("warmup_ms", ready.get("warmup_ms")),
            "boot": {"observer_ready_ms": ready_at, "mount_ready_ms": mounted.get("boot_ms"),
                     "asset_preload_ms": ready.get("preload_ms"),
                     "first_presented_frame_ms": None,
                     "scope": "Observer ready is runner start→received Rust console receipt (native start→stdout receipt). Mount ready is host mount call→ready resolution, including initialization and initial Rust view, before a guaranteed settled-revision draw. First usable/presented-frame startup is not separately measured."},
            "renderer_method_ms": stats_ms(measurement.get("submit_end_cpu_us"), 1000) if kind != "pixi" else raw.get("draw_method_ms"),
            "renderer_method": "Macroquad submit/end CPU timer, excludes final frame flush" if kind != "pixi" else "Pixi scene reconstruction + app.render CPU method timer",
            "frame_interval_ms": measurement.get("frame_interval_ms") if kind != "pixi" else raw.get("frame_interval_ms"),
            "postmessage_roundtrip_ms": raw.get("postmessage_roundtrip_ms"),
            "memory": {"document_js_heap_bytes": raw.get("observation", {}).get("js_heap_bytes"),
                       "child_js_heap_bytes": wasm.get("js_heap_bytes"),
                       "wasm_linear_memory_bytes": wasm.get("wasm_linear_memory_bytes"),
                       "estimated_atlas_rgba_bytes": measurement.get("estimated_resident_rgba_bytes"),
                       "controller_resources": controller.get("resources")},
            "process": process_summary(raw, native),
            "download": None if native else browser_resources(raw),
            "child_wasm_resource": None if not child else {key: wasm.get(key) for key in ["wasm_download_duration_ms", "wasm_download_transfer_bytes", "wasm_download_encoded_bytes"]},
            "authority_operations": final.get("authority", []),
            "native_pixels": raw.get("window_pixels")}


def number(value, digits=3):
    return "—" if value is None else f"{value:,.{digits}f}"


def link(row):
    return f"[{row['kind']} {row['run']}]({row['source']})"


def markdown(data):
    lines = ["# Issue-60 measured observations", "",
             "Generated from the retained receipts by `python3 tools/renderer-embedding-spike/summarize.py`. "
             "[measurements.json](measurements.json) retains unrounded values, input SHA256 fingerprints and resource entries. "
             "The [execution ledger](README.md) owns commands, the [final source manifest](source-manifest.json), build fingerprints, failures and target limitations.", "",
             "These are local observations on the Mac and Chromium configuration in [environment.json](environment.json). "
             "All static rows use the 24-command #59 permitted checkpoint, 900×720 logical viewport, DPR1, light/full motion, "
             "3,000 ms warm-up and 300 samples. Native is a separate control. No engine or containment winner is inferred.", "",
             "## Clock and cache boundaries", "",
             "Observer ready starts at the browser runner before navigation and ends when it receives the Rust console receipt; "
             "native starts at process launch and ends at the stdout receipt. Host mount ready starts inside `mount()` after the shell has loaded "
             "and ends at its ready resolution. It includes initialization and the initial Rust view but does not guarantee a draw of the settled revision. "
             "The first usable/presented-frame startup timestamp is not separately measured. Macroquad's preload clock covers its verified atlas preload, "
             "not complete startup. These clocks must not be subtracted to estimate iframe overhead.", "",
             "Browser run 1 in each path has a fresh browser process and renderer instance; runs 2–3 reuse that browser/context with a new document/renderer. "
             "Disk, OS and graphics-driver caches are not purged. Resource Timing supplies the observed transfer sizes. Native runs each use a fresh process. "
             "The [historical pre-freeze Pixi run 1](runs/pre-freeze-static/chromium-pixi-1.json) overlapped compilation on the shared host, "
             "so it is excluded from the final frozen-source timing cells. The final receipts were refreshed after compilation and source freeze; "
             "their input SHA256 fingerprints are in the machine table. Historical samples must not be reinterpreted as contention-free.", "",
             "| Receipt | Observer ready ms | Host mount ready ms | Atlas preload ms | Cache class |",
             "|---|---:|---:|---:|---|"]
    for row in data["static_runs"]:
        boot = row["boot"]
        cache = "fresh native process" if row["kind"] == "native" else "fresh browser + instance" if row["run"] == 1 else "same browser; new document + instance"
        lines.append(f"| {link(row)} | {number(boot['observer_ready_ms'])} | {number(boot['mount_ready_ms'])} | {number(boot['asset_preload_ms'])} | {cache} |")
    lines += ["", "## Drawing and frame intervals", "",
              "Macroquad reports its submit/end CPU timer, converted from µs to ms here, and excludes final frame flush. "
              "Pixi reports scene reconstruction plus `app.render`. Frame intervals describe pacing between sampled frames, not GPU completion. "
              "The methods, pacing, browser/native targets and sample-window durations differ; their values do not support an engine CPU ranking.", "",
              "| Receipt | Method mean / p50 / p95 / max ms | Interval mean / p50 / p95 / max ms | Samples |",
              "|---|---|---|---:|"]
    for row in data["static_runs"]:
        def four(value):
            return " / ".join(number((value or {}).get(k)) for k in ["mean", "p50", "p95", "max"])
        lines.append(f"| {link(row)} | {four(row['renderer_method_ms'])} | {four(row['frame_interval_ms'])} | {row['samples']} |")
    lines += ["", "## Input and interop", "",
              "The iframe ping is host→child→host message task dispatch only: 100 samples per run, no Rust input or paint. "
              "Coarse timer resolution produces zero median samples. It is neither input feedback nor match latency.", "",
              "| Receipt | Ping mean / p50 / p95 / max ms | Samples |", "|---|---|---:|"]
    for row in data["static_runs"]:
        ping = row["postmessage_roundtrip_ms"]
        if ping:
            lines.append(f"| {link(row)} | {' / '.join(number(ping[k]) for k in ['mean','p50','p95','max'])} | {ping['samples']} |")
    inputs = data.get("input_samples", {})
    lines += ["", f"[Chromium interaction receipt]({inputs.get('source', 'runs/chromium-interactions.json')}) overall status: **{inputs.get('overall_status', 'NOT_RUN')}**. "
              "Its accepted-input check is listed independently below; a passed subcheck does not turn a partial or failed overall receipt into PASS. "
              "Programmatic typed input timing runs through native loopback HTTP, Rust presenter/rules, returned-view validation and the first draw callback for that exact revision. "
              "It excludes physical input-device latency and display presentation. With only the recorded inputs, no p95 distribution or production SLA is justified.", "",
              "| Input | Outcome | Revision | Accepted commands | Round trip to exact-revision draw ms |", "|---|---|---:|---:|---:|"]
    for sample in inputs.get("samples", []):
        lines.append(f"| {sample.get('key',sample.get('input_kind','?'))} | {sample.get('outcome')} | {sample.get('revision')} | {sample.get('input_count')} | {number(sample.get('roundtrip_to_draw_ms'))} |")
    lines += ["", "Static Pixi drawing repeats one Rust view without per-frame HTTP. The raw `authority_operations` retain actual native HTTP/view-update durations; "
              "Resource Timing retains each `/authority` response body size. Rust lowering, serialization, transport and JS validation were not timed separately, "
              "and request/copy sizes were not separately measured. No deployable Rust/WASM FFI cost follows from this native shim.", "",
              "## Process CPU and memory", "",
              "Each row uses the first and last retained `ps` sample, not exact workload start/end. CPU is cumulative-seconds difference divided by elapsed sample-window seconds "
              "(100% = one CPU core equivalent); it is not macOS Activity Monitor or GPU utilization. Browser rows include the launched browser process tree, "
              "including shell/renderer/utility/GPU processes, exclude exited children, and can change membership. RSS sums double-count shared pages. "
              "Native rows cover only that process. The windows differ and support no ranking. MiB is KiB / 1024.", "",
              "| Receipt | ps samples | Window s | CPU Δ s | CPU one-core % | RSS first / last / max MiB | JS heap MiB | WASM linear MiB |",
              "|---|---:|---:|---:|---:|---|---:|---:|"]
    for row in data["static_runs"]:
        p, m = row["process"], row["memory"]
        if not p:
            continue
        heap = m["document_js_heap_bytes"]
        wasm = m["wasm_linear_memory_bytes"]
        rss = " / ".join(number(p[k]/1024) for k in ["rss_kib_first","rss_kib_last","rss_kib_max"])
        lines.append(f"| {link(row)} | {p['samples']} | {number(p['window_s'])} | {number(p['cpu_s_delta'])} | {number(p['cpu_percent_one_core_equivalent'])} | {rss} | {number(heap/1048576 if heap is not None else None)} | {number(wasm/1048576 if wasm is not None else None)} |")
    lines += ["", "JS heap is one document observation, with child observations retained separately for iframe; it is not total process residency. "
              "Macroquad's atlas estimate is 2,488,320 RGBA bytes (two sources), while Pixi reports 46 region textures over two sources and one owned font. "
              "Heap/WASM/texture estimates do not establish GPU allocation, leaks or exact reclamation. "
              "No forced GC was used. The following lifecycle snapshots occur after disposal in 50 short rendered mounts. "
              "The host retains up to 100 diagnostic history snapshots. These observations show a short memory curve, not proof of process/GPU reclamation or absence of leaks.", "",
              "| Lifecycle receipt | Completed | JS heap first / last / max MiB | Process-tree RSS first / last / max MiB | Observation window s |",
              "|---|---:|---|---|---:|"]
    for cycle in data.get("lifecycle_runs", []):
        curve = cycle["memory_curve"]
        heap = [item["environment"]["js_heap_bytes"] for item in curve]
        rss = [item["rss_kib_sum"] for item in curve]
        window = (curve[-1]["environment"]["elapsed_ms"] - curve[0]["environment"]["elapsed_ms"])/1000
        lines.append(f"| [{cycle['kind']} cycles]({cycle['source']}) | {cycle['completed']} | {' / '.join(number(n/1048576) for n in [heap[0],heap[-1],max(heap)])} | {' / '.join(number(n/1024) for n in [rss[0],rss[-1],max(rss)])} | {number(window)} |")
    lines += ["",
              "## Observed downloads", "",
              "These sums count the recorded document Resource Timing entries. Static sums exclude `/authority` and favicon; main navigation is not a Resource Timing entry. "
              "For iframe, the parent timeline does not include all child subresources, so its sum is incomplete for the embedded game. "
              "The child WASM-only transfer observation is retained in the machine table. Encoded body size can remain nonzero for cached responses; transfer size includes reported protocol overhead. "
              "Repeated authority requests are counted individually. The host serves local bytes; these values are not compressed production download size, network latency or a complete shipping footprint.", "",
              "| Receipt | All entry transfer / encoded bytes | Static transfer / encoded bytes | Static zero-transfer entries | Authority responses |",
              "|---|---:|---:|---:|---:|"]
    for row in data["static_runs"]:
        d = row["download"]
        if d:
            lines.append(f"| {link(row)} | {d['all_transfer_bytes_sum']:,} / {d['all_encoded_body_bytes_sum']:,} | {d['static_transfer_bytes_sum']:,} / {d['static_encoded_body_bytes_sum']:,} | {d['static_zero_transfer_entries']} / {d['static_entries']} | {len(d['authority_responses'])} |")
    lines += ["", "Safari/WebKit has no runtime measurement: [probe](safari-probe.json) records remote automation disabled and Playwright WebKit absent. "
              "Native pixels were not captured. Audio, physical higher-DPI/mobile/WebView execution, GPU completion timers, full accessibility action play and online handoff remain unmeasured. "
              "The separate Chromium DPR2 interaction is emulation, not a physical-display measurement.", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, default=Path(__file__).resolve().parents[2] / "docs/verification/issue-60")
    args = parser.parse_args()
    root = args.evidence.resolve()
    rows = [summarize(root / "runs" / f"chromium-{kind}-{run}.json", kind, run)
            for kind in ["document", "iframe", "pixi"] for run in range(1,4)]
    rows += [summarize(root / "runs" / f"native-static-{run}.json", "native", run) for run in range(1,4)]
    expected = "e4ed3465b826a55c12d68d8f8bcbef5422fa5d128a251da1f4f659470060d032"
    for row in rows:
        if (row["status"] != "PASS" or row["scenario"] != "static" or row["samples"] != 300
                or row["warmup_ms"] != 3000 or row["checkpoint"] != expected
                or row["viewport"] != [900,720] or row["dpr"] != 1
                or row["uncontrolled_input_events"] != 0
                or str(row["theme"]).lower() != "light" or row["motion"] != "full"):
            raise ValueError(f"Receipt does not match the declared controlled static table: {row['source']}")
    data = {"schema_version": 1, "environment_source": "environment.json",
            "environment_sha256": digest(root / "environment.json"),
            "units": {"time": "ms unless named s", "bytes": "bytes", "rss": "KiB from ps"},
            "claims": "Local observations only; methods/cache/windows differ; no renderer or containment ranking",
            "static_runs": rows, "lifecycle_runs": [],
            "historical_conditions": {"source": "runs/pre-freeze-static/chromium-pixi-1.json",
                                      "note": "Historical run overlapped shared-host compilation, as recorded in the execution ledger; excluded from final frozen-source timing cells."}}
    historical = root / data["historical_conditions"]["source"]
    if historical.exists():
        data["historical_conditions"]["source_sha256"] = digest(historical)
    for kind in ["pixi", "iframe"]:
        path = root / "runs" / f"chromium-{kind}-cycles.json"
        if path.exists():
            raw = json.loads(path.read_text())
            data["lifecycle_runs"].append({"source": f"runs/{path.name}", "source_sha256": digest(path),
                                          "kind": kind, "status": raw["status"], "completed": raw["completed"],
                                          "memory_curve": raw["memory_curve"], "cleanup_claim": raw["cleanup_claim"]})
    interaction = root / "runs/chromium-interactions.json"
    if interaction.exists():
        raw = json.loads(interaction.read_text())
        accepted = next((c for c in raw["checks"] if c["name"].startswith("Rust presenter accepted input")), {})
        data["input_samples"] = {"source": "runs/chromium-interactions.json", "source_sha256": digest(interaction),
                                 "overall_status": raw["status"], "check_name": accepted.get("name"), "check_status": accepted.get("status"),
                                 "samples": accepted.get("evidence", {}).get("samples", [])}
    (root / "measurements.json").write_text(json.dumps(data, indent=2) + "\n")
    (root / "measurements.md").write_text(markdown(data))
    print(json.dumps({"static_receipts": len(rows), "input_overall_status": data.get("input_samples", {}).get("overall_status"),
                      "outputs": [str(root / "measurements.md"), str(root / "measurements.json")]}))


if __name__ == "__main__":
    main()
