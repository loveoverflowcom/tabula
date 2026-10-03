"""Rebuild the issue59 table from captured, controlled runtime receipts."""
from pathlib import Path
import json
import statistics
root = Path(__file__).resolve().parent
records = []
for path in sorted((root / 'runs').glob('*.json')):
    run = json.loads(path.read_text())
    ready, measurement, environment = run['ready'], run['measurement'], run['environment']
    assert ready['commit'] == 'issue59-source-60460d1affc8c682'
    assert measurement['samples'] >= 600 and measurement['script_complete']
    assert measurement['uncontrolled_input_events'] == 0
    assert measurement['decodes_after_preload'] == measurement['uploads_after_preload'] == 0
    assert measurement['decodes'] == measurement['uploads'] == 2
    assert environment['viewport'] == environment['canvas_pixels'] == [900, 720]
    assert measurement['viewport'] == [900, 720] and measurement['dpi'] == 1
    assert not run['console_errors']
    scripted = ready['scenario'] == 'scripted'
    expected = ('b7e81e41d5bd48f076a736858b6855b663591034127628be52b887ae1ce19a73' if scripted else
                'e4ed3465b826a55c12d68d8f8bcbef5422fa5d128a251da1f4f659470060d032')
    assert measurement['final_checkpoint'] == expected
    assert measurement['accepted_inputs'] == (44 if scripted else 24)
    assert measurement['scripted_steps'] == (20 if scripted else 0)
    records.append((path.stem, ready, measurement, environment))
lines = [
    '# Browser measurements, 2026-10-03', '',
    'See [conditions and limits](README.md) and the [reproduction protocol](../../perf/tiles-renderer-baseline.md).', '',
    'Every row is one fresh WASM renderer/cache instance in the existing visible Chromium process.',
    'Three static runs per rendering mode are controlled repeats. Each theme/motion script is a single',
    'runtime coverage run with a nonzero host-event count, excluded from this table. Preload verifies',
    'and uploads both atlas densities. Browser/network/driver caches were not purged.', '',
    'Method timing is wall time, with roughly 1 ms browser clock quantization. Frame interval includes',
    'browser scheduling. Memory is a post-run readback, not peak or process RSS. No forced GC was used.',
    'All rows have zero input events and zero captured warnings/errors. Script coverage is separate.', '',
    '| Run | Frames | Preload ms | Method mean/p95 ms | Frame p50/p95 ms | WASM MiB | JS heap MiB at readback | Readback after start s |',
    '|---|---:|---:|---:|---:|---:|---:|---:|',
]
for name, ready, m, e in records:
    cpu, frame = m['submit_end_cpu_us'], m['frame_interval_ms']
    lines.append(f"| [{name}](runs/{name}.json) | {m['samples']} | {ready['preload_ms']:.1f} | "
                 f"{cpu['mean']/1000:.3f}/{cpu['p95']/1000:.3f} | {frame['p50']:.1f}/{frame['p95']:.1f} | "
                 f"{e['wasm_linear_memory_bytes']/1048576:.2f} | {e['js_heap_bytes']/1048576:.2f} | {e['elapsed_ms']/1000:.1f} |")
lines += ['', 'Each run retains 2 textures, estimated RGBA residency 2,488,320 bytes (2.37 MiB),',
          'with exactly 2 decodes/uploads before the loop and 0 during the measured workload.',
          'Static rows share 24 accepted inputs/13 cells; scripts share 44 inputs/25 cells and',
          'the exact checkpoints listed in the ledger. Static primitive control loads the same',
          'pack and replaces only Sprite geometry with solid quads; it is not old artwork.', '']
for prefix in ('static-sprite', 'static-primitive'):
    selected = [m for name, _, m, _ in records if name.startswith(prefix)]
    if len(selected) == 3:
        lines.append(f"{prefix}: median of run means = {statistics.median(m['submit_end_cpu_us']['mean'] for m in selected)/1000:.3f} ms; "
                     f"observed frame p95 range = {min(m['frame_interval_ms']['p95'] for m in selected):.1f}–{max(m['frame_interval_ms']['p95'] for m in selected):.1f} ms.")
lines += ['', 'The primitive control is a local contract/workload check, not evidence that another',
          'engine or embedding would be slower. These coarse method samples exclude the final',
          'Macroquad/GPU flush. Native/WebKit, physical DPI2, GPU and total process CPU/RSS remain NOT_RUN.', '']
(root / 'measurements.md').write_text('\n'.join(lines))
print(f'Validated and summarized {len(records)} controlled receipts')
