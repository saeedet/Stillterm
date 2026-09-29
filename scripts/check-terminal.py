#!/usr/bin/env python3
"""Exercise terminal cleanup using a real Unix PTY (Python standard library only)."""

import argparse
import errno
import fcntl
import os
from pathlib import Path
import select
import signal
import struct
import subprocess
import termios
import time


def read_available(master, output, timeout=0.05):
    if select.select([master], [], [], timeout)[0]:
        try:
            output.extend(os.read(master, 65536))
        except OSError as error:
            if error.errno != errno.EIO:
                raise


def set_size(slave, columns, rows):
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))


def check(binary, mode):
    master, slave = os.openpty()
    set_size(slave, 80, 24)
    original = termios.tcgetattr(slave)
    output = bytearray()
    process = None
    try:
        process = subprocess.Popen(
            [binary, "--seed", "42"],
            stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"},
            start_new_session=True,
        )
        deadline = time.monotonic() + 5
        while b"\x1b[?25l" not in output and time.monotonic() < deadline:
            read_available(master, output)
            if process.poll() is not None:
                raise AssertionError(f"{mode}: exited before setup: {output!r}")
        assert b"\x1b[?25l" in output, f"{mode}: cursor was not hidden"
        assert not termios.tcgetattr(slave)[3] & termios.ICANON

        # Let the renderer produce a few frames before exercising its lifecycle.
        until = time.monotonic() + 0.2
        while time.monotonic() < until:
            read_available(master, output)
        if mode in ("q", "escape", "ctrl-c"):
            os.write(master, {"q": b"q", "escape": b"\x1b", "ctrl-c": b"\x03"}[mode])
        elif mode in ("SIGINT", "SIGTERM", "SIGHUP"):
            process.send_signal(getattr(signal, mode))
        elif mode == "resize":
            for columns, rows in [(120, 40), (1, 1), (0, 0), (80, 24)]:
                set_size(slave, columns, rows)
                process.send_signal(signal.SIGWINCH)
                until = time.monotonic() + 0.1
                while time.monotonic() < until:
                    read_available(master, output)
            os.write(master, b"q")
        elif mode == "resize-error":
            set_size(slave, 65535, 65535)
            process.send_signal(signal.SIGWINCH)

        deadline = time.monotonic() + 5
        while process.poll() is None and time.monotonic() < deadline:
            read_available(master, output)
        assert process.poll() is not None, f"{mode}: exit timed out"
        while select.select([master], [], [], 0)[0]:
            read_available(master, output, 0)
        expected = 1 if mode == "resize-error" else 0
        assert process.returncode == expected, f"{mode}: {process.returncode}: {output!r}"
        assert termios.tcgetattr(slave) == original, f"{mode}: terminal modes changed"
        assert b"\x1b[?1049l" in output, f"{mode}: alternate screen not left"
        assert b"\x1b[?25h" in output, f"{mode}: cursor not restored"
        print(f"PASS {mode}")
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", default="target/debug/stillterm")
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    for mode in ("q", "escape", "ctrl-c", "SIGINT", "SIGTERM", "SIGHUP", "resize", "resize-error"):
        check(binary, mode)


if __name__ == "__main__":
    main()
