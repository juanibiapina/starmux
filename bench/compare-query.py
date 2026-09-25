#!/usr/bin/env python3
"""Compare direct argv and one tmux socket query: compare-query.py BINARY [PAIRS]."""
import os
import re
import shlex
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

binary = str(Path(sys.argv[1]).resolve())
count = int(sys.argv[2]) if len(sys.argv) > 2 else 200


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def percentile(values, fraction):
    return sorted(values)[int((len(values) - 1) * fraction)]


client = run("tmux", "list-clients", "-F", "#{client_name}").splitlines()[0]
socket = run("tmux", "display-message", "-p", "-c", client, "#{socket_path}")
adapter = run(binary, "init", "tmux-argv")
transport = re.search(r"#\(starmux render (.*)\)'", adapter).group(1)
fields = shlex.split(run("tmux", "-S", socket, "display-message", "-p", "-c", client, "--", transport))
commands = {
    "argv": [binary, "render", *fields],
    "query": [binary, "render-query", f"--socket={socket}", f"--client={client}"],
}

with tempfile.TemporaryDirectory(prefix="sidebar-query-bench-") as root:
    config = Path(root) / "no-providers.toml"
    config.write_text('format = "$sessions$divider"\n')
    env = {**os.environ, "STARMUX_CONFIG": str(config)}
    samples = {"argv": [], "query": []}
    for i in range(count + 20):
        output = {}
        order = ("argv", "query") if i % 2 else ("query", "argv")
        for name in order:
            started = time.perf_counter_ns()
            child = subprocess.Popen(commands[name], stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
            line = child.stdout.readline()
            elapsed = (time.perf_counter_ns() - started) / 1e6
            stderr = child.stderr.read()
            if child.wait(timeout=5) or b"#[nl]" not in line:
                raise RuntimeError(f"{name} failed: {stderr.decode(errors='replace')}")
            output[name] = line
            if i >= 20:
                samples[name].append(elapsed)
        if output["argv"] != output["query"]:
            raise RuntimeError("query and argv rendered different live snapshots; retry without changing tmux state")
    windows = fields.count("--end-window")
    sessions = next(x.split("=", 1)[1] for x in fields if x.startswith("--session-count="))
    print(f"client={client} sessions={sessions} windows={windows} pairs={count}")
    print(f"format command: argv={sum(len(x) + 1 for x in commands['argv'])} bytes, query={sum(len(x) + 1 for x in commands['query'])} bytes")
    for name, values in samples.items():
        print(f"{name:5} first-line median={statistics.median(values):.2f}ms p95={percentile(values, .95):.2f}ms")
    print(f"query - argv median={statistics.median(samples['query']) - statistics.median(samples['argv']):+.2f}ms p95={percentile(samples['query'], .95) - percentile(samples['argv'], .95):+.2f}ms")
