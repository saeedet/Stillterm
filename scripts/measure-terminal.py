#!/usr/bin/env python3
"""Measure the CLI in a drained Unix PTY; excludes terminal-emulator rendering."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import resource
import select
import struct
import subprocess
import sys
import termios
import time


def measure(binary, columns, rows, seconds):
    master, slave = os.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
    process = None
    try:
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.monotonic()
        process = subprocess.Popen(
            [binary, "--fps", "30", "--seed", "42"],
            stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color", "NO_COLOR": ""},
            start_new_session=True,
        )
        count = 0
        while time.monotonic() - start < seconds:
            if process.poll() is not None:
                raise RuntimeError("animation exited before measurement finished")
            if select.select([master], [], [], 0.05)[0]:
                count += len(os.read(master, 65536))
        os.write(master, b"q")
        deadline = time.monotonic() + 5
        while process.poll() is None and time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                count += len(os.read(master, 65536))
        if process.poll() is None:
            raise RuntimeError("animation did not exit after q")
        if process.returncode:
            raise RuntimeError(f"animation failed: {process.returncode}")
        wall = time.monotonic() - start
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
        # ru_maxrss is the maximum across reaped children, not a sum. Run each
        # measurement in a fresh Python process to isolate this child's peak.
        peak_bytes = after.ru_maxrss * (1 if sys.platform == "darwin" else 1024)
        print(json.dumps({
            "grid": f"{columns}x{rows}", "seconds": round(wall, 2),
            "cpu_percent_one_core": round(cpu / wall * 100, 3),
            "peak_rss_mib": round(peak_bytes / 1024**2, 2),
            "output_bytes_per_second": round(count / wall),
        }))
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", default="target/release/stillterm")
    parser.add_argument("--columns", type=int, default=120)
    parser.add_argument("--rows", type=int, default=40)
    parser.add_argument("--seconds", type=float, default=10)
    args = parser.parse_args()
    if not 1 <= args.columns <= 65535 or not 1 <= args.rows <= 65535 or args.seconds <= 0:
        parser.error("dimensions must be 1–65535 and duration must be positive")
    measure(str(Path(args.binary).resolve()), args.columns, args.rows, args.seconds)
