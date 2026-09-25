#!/usr/bin/env python3
"""Run after cargo build --release; first output is read as one physical line."""
import os
import statistics
import subprocess
import sys
import time

binary = sys.argv[1] if len(sys.argv) > 1 else "target/release/starmux"
count = int(sys.argv[2]) if len(sys.argv) > 2 else 100


def args(windows):
    result = ["render", "--input-version=1", "--width=30", "--client-width=100", "--client-height=25", "--current-session=$0", "--current-pane=%0", "--pane-path=/tmp", "--session-count=1", "--session-id=$0", "--session-name=bench", f"--window-count={windows}"]
    for i in range(windows):
        result.extend([f"--window-id=@{i}", f"--window-index={i}", f"--window-name=window-{i}", f"--selected={int(i == 0)}", f"--window-pane=%{i}", "--window-path=/tmp", "--pi-state=", "--window-icon=", "--end-window"])
    return result + ["--end-session"]


def p(values, percent):
    return sorted(values)[int((len(values) - 1) * percent)]


for size in (1, 10, 100, 1000):
    first = []
    settled = []
    for _ in range(count):
        started = time.perf_counter_ns()
        child = subprocess.Popen([binary, *args(size)], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env={**os.environ, "STARMUX_CONFIG": "/dev/null"})
        line = child.stdout.readline()
        first.append((time.perf_counter_ns() - started) / 1e6)
        if b"#[nl]" not in line:
            raise RuntimeError("incomplete first snapshot")
        child.communicate(timeout=5)
        if child.returncode:
            raise RuntimeError(f"render exit {child.returncode}")
        settled.append((time.perf_counter_ns() - started) / 1e6)
    print(f"{size:4} windows first median={statistics.median(first):.2f}ms p95={p(first, .95):.2f}ms settled median={statistics.median(settled):.2f}ms p95={p(settled, .95):.2f}ms")
