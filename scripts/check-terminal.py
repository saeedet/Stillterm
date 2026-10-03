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
            chunk = os.read(master, 65536)
            output.extend(chunk)
            return bool(chunk)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
    return False


def set_size(slave, columns, rows):
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))


def check(binary, mode, theme):
    master, slave = os.openpty()
    set_size(slave, 80, 24)
    original = termios.tcgetattr(slave)
    output = bytearray()
    process = None
    try:
        capture = mode in ("any-key", "mouse-click", "mouse-scroll", "input-signal")
        process = subprocess.Popen(
            [binary, "--seed", "42", "--theme", theme] + (["--exit-on-input"] if capture else []),
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
        if capture:
            assert b"\x1b[?1003h" in output, "screensaver mode must capture mouse input"
        else:
            assert b"\x1b[?1003h" not in output, "ordinary CLI must not capture mouse input"
        if mode in ("any-key", "mouse-click", "mouse-scroll"):
            # A synthetic pointer move on mapping must not dismiss the saver.
            os.write(master, b"\x1b[<35;10;10M")
            time.sleep(0.1)
            assert process.poll() is None, "synthetic pointer move dismissed the saver"
            os.write(master, {"any-key": b"x", "mouse-click": b"\x1b[<0;10;10M",
                              "mouse-scroll": b"\x1b[<64;10;10M"}[mode])
        elif mode == "input-signal":
            process.send_signal(signal.SIGTERM)
        elif mode == "ordinary-key":
            os.write(master, b"x")
            time.sleep(0.1)
            assert process.poll() is None, "ordinary CLI exited on an unrelated key"
            os.write(master, b"q")
        elif mode in ("q", "escape", "ctrl-c"):
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
            if not read_available(master, output, 0):
                break
        expected = 1 if mode == "resize-error" else 0
        assert process.returncode == expected, f"{mode}: {process.returncode}: {output!r}"
        assert termios.tcgetattr(slave) == original, f"{mode}: terminal modes changed"
        assert b"\x1b[?1049l" in output, f"{mode}: alternate screen not left"
        assert b"\x1b[?25h" in output, f"{mode}: cursor not restored"
        if capture:
            assert b"\x1b[?1003l" in output, "mouse capture was not restored"
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
    parser.add_argument("--theme", choices=["monochrome", "matrix"], default="monochrome")
    parser.add_argument("--panic-check", action="store_true", help="also run the Rust panic cleanup test in a PTY")
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    for mode in ("q", "escape", "ctrl-c", "SIGINT", "SIGTERM", "SIGHUP", "resize", "resize-error", "ordinary-key", "any-key", "mouse-click", "mouse-scroll", "input-signal"):
        check(binary, mode, args.theme)
    if args.panic_check:
        check_panic()


def check_panic():
    master, slave = os.openpty()
    original = termios.tcgetattr(slave)
    output = bytearray()
    process = None
    try:
        process = subprocess.Popen(
            ["cargo", "test", "--locked", "-p", "stillterm", "session::tests::panic_restores_terminal", "--", "--ignored", "--nocapture", "--test-threads=1"],
            stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"}, start_new_session=True,
        )
        deadline = time.monotonic() + 60
        inspected = False
        while process.poll() is None and time.monotonic() < deadline:
            read_available(master, output)
            if not inspected and b"STILLTERM_CLEANUP_READY" in output:
                assert termios.tcgetattr(slave) == original, "panic changed terminal modes"
                inspected = True
                os.write(master, b"\n")
        assert process.poll() == 0, f"panic test failed or timed out: {output!r}"
        while select.select([master], [], [], 0)[0]:
            if not read_available(master, output, 0):
                break
        assert inspected, f"panic cleanup handshake missing: {output!r}"
        for sequence in [b"\x1b[?1049h", b"\x1b[?1049l", b"\x1b[?25h", b"\x1b[?1003l"]:
            assert sequence in output, f"panic cleanup missing {sequence!r}: {output!r}"
        print("PASS panic cleanup")
    finally:
        if process is not None and process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    main()
