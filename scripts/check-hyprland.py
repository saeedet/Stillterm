#!/usr/bin/env python3
"""Exercise the real launcher against isolated Unix IPC and terminal processes."""

import json
import os
from pathlib import Path
import signal
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
LAUNCHER = ROOT / "platforms/linux/stillterm-hyprland"
APP_ID = "org.stillterm.screensaver"


def until(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.03)
    raise AssertionError("Timed out waiting for the launcher")


class IPC(socketserver.UnixStreamServer):
    allow_reuse_address = True


class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        command = self.request.recv(65536).decode()
        try:
            reply = self.server.owner.reply(command)
            if reply is not None:
                self.request.sendall(reply.encode())
        except (BrokenPipeError, ConnectionResetError):
            pass


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="st-hypr-", dir="/tmp")
        self.root = Path(self.temp.name)
        self.runtime = self.root / "run"
        self.runtime.mkdir(mode=0o700)
        self.ipc_dir = self.runtime / "hypr/test"
        self.ipc_dir.mkdir(parents=True)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.state = self.root / "state"
        self.state.mkdir()
        self.env = {**os.environ, "XDG_RUNTIME_DIR": str(self.runtime), "HYPRLAND_INSTANCE_SIGNATURE": "test",
                    "PATH": str(self.bin) + os.pathsep + os.environ["PATH"], "ST_TEST_STATE": str(self.state)}
        self.write_program("stillterm", 'print("--exit-on-input")')
        self.write_program("foot", '''import json, os, pathlib, signal, sys, time
state = pathlib.Path(os.environ["ST_TEST_STATE"])
if "--check-config" in sys.argv:
    sys.exit(0)
pid = str(os.getpid())
(state / (pid + ".args")).write_text(json.dumps(sys.argv[1:]))
def stop(*_):
    (state / (pid + ".stopped")).touch()
    sys.exit(0)
signal.signal(signal.SIGTERM, stop)
while True:
    if (state / "crash").exists() and (state / "crash").read_text() == pid:
        sys.exit(2)
    time.sleep(0.02)
''')
        self.monitors = [dict(id=0, name="DP-1", width=1920, height=1080, scale=1, focused=True),
                         dict(id=1, name="HDMI-A-1", width=3840, height=2160, scale=2, focused=False)]
        self.locked = False
        self.pointer = {"x": 100, "y": 100}
        self.focus_away = False
        self.lua = True
        self.reject_dispatch = False
        self.map_windows = True
        self.malformed = False
        self.requests = []
        self.placements = {}
        self.server = IPC(str(self.ipc_dir / ".socket.sock"), Handler)
        self.server.owner = self
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.02}, daemon=True)
        self.thread.start()
        self.processes = []

    def tearDown(self):
        for process in self.processes:
            if process.poll() is None:
                process.terminate()
            try:
                process.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temp.cleanup()

    def write_program(self, name, body):
        path = self.bin / name
        path.write_text(f"#!{sys.executable}\n{body}\n")
        path.chmod(0o755)

    def children(self):
        return sorted(int(path.stem) for path in self.state.glob("*.args"))

    def reply(self, request):
        self.requests.append(request)
        if self.malformed:
            return "invalid JSON"
        if request == "/eval return":
            return "ok" if self.lua else "eval is only supported with the lua config manager"
        if request.startswith("/dispatch "):
            if self.reject_dispatch:
                return "rejected"
            # Assign the most recently mapped child to the requested connector.
            if ".move(" in request or "movewindow " in request:
                for monitor in self.monitors:
                    if monitor["name"] in request:
                        self.placements[self.children()[-1]] = monitor["id"]
            return "ok"
        clients = [dict(pid=pid, **{"class": APP_ID}, address=f"0x{pid:x}",
                        monitor=self.placements.get(pid, 0), fullscreen=2) for pid in self.children()]
        values = {"j/locked": {"locked": self.locked}, "j/monitors": self.monitors,
                  "j/version": {"tag": "v0.56.2"}, "j/cursorpos": self.pointer,
                  "j/clients": clients if self.map_windows else [],
                  "j/activewindow": {"pid": 999, "class": "unrelated"} if self.focus_away else (clients[-1] if clients else {})}
        return json.dumps(values[request])

    def start(self, *options):
        process = subprocess.Popen([sys.executable, str(LAUNCHER), "start", *options],
                                   env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.processes.append(process)
        return process

    def ready(self, process):
        until(lambda: len(self.placements) == 2 or process.poll() is not None)
        self.assertIsNone(process.poll(), process.communicate() if process.poll() is not None else "")
        # Wait until the startup pointer has been sampled, then allow the idle loop.
        until(lambda: "j/cursorpos" in self.requests)

    def stopped(self, process, code=0):
        output, error = process.communicate(timeout=7)
        self.assertEqual(process.returncode, code, output + error)
        for pid in self.children():
            if not (self.state / "crash").exists() or (self.state / "crash").read_text() != str(pid):
                self.assertTrue((self.state / f"{pid}.stopped").exists(), f"terminal {pid} leaked")
        self.assertFalse(list(self.runtime.glob("stillterm-*/control.sock")))

    def test_monitors_options_and_scoped_stop(self):
        process = self.start("--", "--theme", "matrix", "--characters", "ab';$(example)")
        self.ready(process)
        self.assertEqual(set(self.placements.values()), {0, 1})
        for path in self.state.glob("*.args"):
            args = json.loads(path.read_text())
            self.assertIn("--exit-on-input", args)
            self.assertEqual(args[-1], "ab';$(example)")
        self.assertEqual(sum('prop="idle_inhibit", value="0"' in r for r in self.requests), 2)
        subprocess.run([sys.executable, str(LAUNCHER), "stop"], env=self.env, check=True)
        self.stopped(process)
        subprocess.run([sys.executable, str(LAUNCHER), "stop"], env=self.env, check=True)
        # No global cursor settings, lock commands, or broad process killing.
        self.assertTrue(all(r.startswith(("j/", "/eval return", "/dispatch hl.dsp.window.", "/dispatch hl.dsp.focus(")) for r in self.requests))

    def test_duplicate_start_does_not_create_windows(self):
        process = self.start()
        self.ready(process)
        duplicate = self.start()
        self.assertEqual(duplicate.wait(timeout=3), 0)
        self.assertEqual(len(self.children()), 2)
        process.terminate()
        self.stopped(process)

    def test_lock_handoff(self):
        process = self.start()
        self.ready(process)
        self.locked = True
        self.stopped(process)

    def test_refuse_start_while_locked(self):
        self.locked = True
        process = self.start()
        self.stopped(process)
        self.assertEqual(self.children(), [])

    def test_mouse_movement(self):
        process = self.start()
        self.ready(process)
        self.pointer = {"x": 101, "y": 100}
        self.stopped(process)

    def test_focus_loss(self):
        process = self.start()
        self.ready(process)
        self.focus_away = True
        self.stopped(process)

    def test_monitor_hotplug(self):
        process = self.start()
        self.ready(process)
        self.monitors = self.monitors[:1]
        self.stopped(process)

    def test_renderer_crash(self):
        process = self.start()
        self.ready(process)
        (self.state / "crash").write_text(str(self.children()[0]))
        self.stopped(process)

    def test_compositor_failure(self):
        process = self.start()
        self.ready(process)
        self.malformed = True
        self.stopped(process, code=1)

    def test_rejected_window_configuration(self):
        self.reject_dispatch = True
        self.stopped(self.start(), code=1)

    def test_window_map_timeout_cleans_up(self):
        self.map_windows = False
        self.stopped(self.start(), code=1)

    def test_stop_during_startup(self):
        self.map_windows = False
        process = self.start()
        until(lambda: self.children())
        process.send_signal(signal.SIGHUP)
        self.stopped(process)

    def test_legacy_dispatchers(self):
        self.lua = False
        process = self.start()
        self.ready(process)
        self.assertEqual(sum("idle_inhibit 0" in r for r in self.requests), 2)
        self.assertEqual(sum("movewindow mon:" in r for r in self.requests), 2)
        process.terminate()
        self.stopped(process)

    def test_doctor_opens_no_windows(self):
        result = subprocess.run([sys.executable, str(LAUNCHER), "doctor"], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("manual preview only", result.stdout)
        self.assertEqual(self.children(), [])

    def test_lock_during_startup(self):
        self.map_windows = False
        process = self.start()
        until(lambda: self.children())
        self.locked = True
        self.stopped(process)

    def test_fallback_monitor_is_rejected(self):
        self.monitors = [dict(id=0, name="FALLBACK")]
        self.stopped(self.start(), code=1)
        self.assertEqual(self.children(), [])

    def test_unsafe_runtime_is_rejected(self):
        self.runtime.chmod(0o755)
        self.stopped(self.start(), code=1)
        self.assertEqual(self.children(), [])

    def test_stale_socket_and_unrelated_process(self):
        import hashlib
        suffix = hashlib.sha256(b"test").hexdigest()[:12]
        directory = self.runtime / ("stillterm-" + suffix)
        directory.mkdir(mode=0o700)
        (directory / "control.sock").touch()
        sentinel = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
        try:
            process = self.start()
            self.ready(process)
            process.terminate()
            self.stopped(process)
            self.assertIsNone(sentinel.poll())
        finally:
            sentinel.terminate()
            sentinel.wait()

    def test_missing_dependency(self):
        self.write_program("stillterm", 'print("old version")')
        self.stopped(self.start(), code=1)
        self.assertEqual(self.children(), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
