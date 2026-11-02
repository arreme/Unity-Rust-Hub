#!/usr/bin/env python3
"""Measure the app's peak RSS, including short-lived Unity CLI children.

Linux-only. Run a release build first:

    cargo build --release -p unity-hub
    scripts/measure-memory.py

The sampler walks /proc directly so it does not depend on psutil.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time


def proc_rss_mb(pid: int) -> float:
    try:
        with open(f"/proc/{pid}/status", encoding="utf-8") as status:
            for line in status:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1]) / 1024.0
    except FileNotFoundError:
        pass
    return 0.0


def direct_children(root: int) -> list[int]:
    children: list[int] = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        pid = int(entry)
        try:
            with open(f"/proc/{pid}/stat", encoding="utf-8") as stat:
                ppid = int(stat.read().split()[3])
        except (FileNotFoundError, ProcessLookupError, IndexError, ValueError):
            continue
        if ppid == root:
            children.append(pid)
    return children


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        default=os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            "target",
            "release",
            "unity-rust-hub",
        ),
    )
    parser.add_argument("--seconds", type=float, default=6.0)
    parser.add_argument("--interval", type=float, default=0.02)
    args = parser.parse_args()

    if not os.path.isfile(args.binary):
        print(f"Binary not found: {args.binary}", file=sys.stderr)
        return 2

    process = subprocess.Popen(
        [args.binary], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    peak_total = 0.0
    peak_children = 0.0
    peak_child_count = 0
    peak_app = 0.0

    deadline = time.monotonic() + args.seconds
    try:
        while time.monotonic() < deadline and process.poll() is None:
            app = proc_rss_mb(process.pid)
            children = direct_children(process.pid)
            child_rss = sum(proc_rss_mb(pid) for pid in children)
            total = app + child_rss
            peak_app = max(peak_app, app)
            peak_total = max(peak_total, total)
            if child_rss > peak_children:
                peak_children = child_rss
                peak_child_count = len(children)
            time.sleep(args.interval)
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

    print(f"app peak RSS:        {peak_app:6.1f} MB")
    print(f"child peak RSS:      {peak_children:6.1f} MB ({peak_child_count} children)")
    print(f"total peak RSS:      {peak_total:6.1f} MB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
