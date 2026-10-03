#!/usr/bin/env python3
"""Bounded native Macroquad control; records executed runtime, not compilation."""
import argparse
import json
import selectors
import subprocess
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    command = [str(args.executable.resolve()), "--scenario", "static", "--theme", "light",
               "--motion", "full", "--initial-inputs", "24", "--warmup-ms", "3000",
               "--samples", "300", "--width", "900", "--height", "720"]
    started = time.perf_counter()
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, bufsize=1)
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    receipts, lines, memory = [], [], []
    last_sample = -1
    try:
        while time.perf_counter() - started < 60:
            elapsed = time.perf_counter() - started
            if int(elapsed) != last_sample:
                last_sample = int(elapsed)
                stat = subprocess.run(["ps", "-o", "time=,rss=,%cpu=", "-p", str(process.pid)],
                                      capture_output=True, text=True, check=False)
                memory.append({"elapsed_s": elapsed, "ps_time_rss_kib_cpu": stat.stdout.strip(),
                               "ps_exit": stat.returncode})
            for key, _ in selector.select(timeout=0.2):
                line = key.fileobj.readline()
                if not line:
                    break
                lines.append(line.rstrip())
                marker = line.find("TABULA_BASELINE ")
                if marker >= 0:
                    try:
                        receipt = json.loads(line[marker + len("TABULA_BASELINE "):])
                    except ValueError:
                        continue
                    receipts.append({"elapsed_ms": (time.perf_counter() - started) * 1000,
                                     "receipt": receipt})
            if any(r["receipt"].get("kind") in ("measurement", "failure") for r in receipts):
                break
            if process.poll() is not None:
                break
    finally:
        observed = process.poll()
        if observed is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        selector.close()
    passed = any(r["receipt"].get("kind") == "measurement" for r in receipts)
    result = {"status": "PASS" if passed else "BLOCKED", "command": command,
              "cache": "fresh-native-process; disk/driver caches not purged",
              "window_pixels": "NOT_CAPTURED", "receipts": receipts,
              "process_samples": memory, "runtime_log": lines,
              "observed_exit_before_cleanup": observed,
              "cleanup": "process terminated after bounded measurement"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "output": str(args.output),
                      "receipts": len(receipts)}))
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
