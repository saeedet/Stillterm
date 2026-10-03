# Linux / Hyprland

Milestone 4 reuses Stillterm's Rust terminal renderer in one Foot window per
monitor. A small Python standard-library adapter owns those windows only
while the screensaver runs. No additional idle daemon or graphical engine is
needed. The desktop's existing locker remains responsible for authentication.

## Compatibility boundary

Source review on 2026-10-03:

| Component | Reviewed version | Integration |
| --- | --- | --- |
| Hyprland | 0.56.2 | Fullscreen visualizer, with Lua and legacy dispatcher support |
| Foot | 1.27.0 | Dedicated configuration, separate terminal process per monitor |
| Hypridle | Current documented listener API | Optional timeout/resume integration; existing lock schedule retained |
| Omarchy | 4.0.4 (`c668141`) | Manual preview only; automatic idle replacement unsupported |

These are source-reviewed targets, not claims of interactive desktop testing.
No Linux desktop is available for physical acceptance testing yet.

Omarchy 4.0.4 hard-codes `omarchy-launch-screensaver` in its Quickshell idle
service. It cancels the pending lock when the last `org.omarchy.screensaver`
window closes, without distinguishing user dismissal from a renderer crash.
Replacing that renderer alone would inherit this behavior. Stillterm therefore
does not replace packaged commands, shadow their names on PATH, or install
Hypridle alongside Omarchy's existing idle service.

Automatic Omarchy support needs a configurable launch/stop interface and a lock
schedule that survives visualizer failure. Until then, use manual preview only.

Reviewed sources: [Omarchy idle service](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/shell/plugins/services/idle/Service.qml),
[launcher](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/bin/omarchy-launch-screensaver),
[locking](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/bin/omarchy-system-lock),
[Hyprland IPC](https://wiki.hypr.land/IPC/),
[Hypridle](https://wiki.hypr.land/Hypr-Ecosystem/hypridle/),
[Foot manual](https://codeberg.org/dnkl/foot/src/tag/1.27.0/doc/foot.1.scd).

## Install and preview

Requires Hyprland, Foot 1.27 or newer, Python 3.10 or newer, and the Stillterm CLI.
From a source checkout, build and install for the current user:

```sh
cargo install --path crates/terminal --locked
install -Dm755 platforms/linux/stillterm-hyprland "$HOME/.local/bin/stillterm-hyprland"
```

Ensure both `~/.cargo/bin` and `~/.local/bin` are on the graphical session's PATH.
A terminal-only PATH setting may not reach an idle service. From a terminal inside
Hyprland:

```sh
stillterm-hyprland doctor
stillterm-hyprland start
stillterm-hyprland start -- --theme matrix
```

`doctor` checks dependencies, Foot configuration, monitors, and lock-state IPC
without opening screensaver windows. It reports versions for a bug report; a
successful check does not establish visual compatibility. `start` stays in the
foreground until dismissal. Arguments after `--` go directly to Stillterm, so
`--config /absolute/path/config.toml`, `--fps 20`, and other CLI settings work.

Press any key, click, scroll, or move the pointer to dismiss all Stillterm windows.
From another terminal, `stillterm-hyprland stop` stops the current session's run.
A second start is a no-op. Focus leaving Stillterm, a terminal exiting, locking,
IPC failure, or a display layout change also closes the windows. On hotplug the
adapter exits; the next idle activation picks up the new layout. Mirrored outputs
share their source display instead of getting another window.

The adapter polls compositor state every 250 ms while active, with bounded IPC
and window-start deadlines. It uses a private session socket and an advisory lock,
not saved process IDs or broad process-name matching. It terminates only the Foot
processes it started. Closing their PTYs lets the Rust renderer restore its modes
and exit. Forced process kills cannot guarantee cleanup.

Foot uses a temporary black-background configuration rather than your terminal
preferences. Stillterm sets `idle_inhibit` to `0` on its own windows and never
changes global cursor settings, starts a locker, or requests idle inhibition.
Font appearance, focus behavior, compositor rules, and actual idle/lock timing
still require desktop testing. A short window transition during placement may be
visible. CPU/GPU and memory use include Foot and the compositor, not just Rust.

## Optional Hypridle setup

Use this only on a Hyprland session already managed by **Hypridle**, with a working
independent lock schedule. **Do not use it on Omarchy 4.0.4.** There is no automatic
installer or configuration rewrite.

1. Back up `~/.config/hypr/hypridle.conf`.
2. Add this visual-only listener, choosing a timeout earlier than your existing
   lock timeout:

   ```ini
   listener {
       timeout = 150
       on-timeout = ~/.local/bin/stillterm-hyprland start -- --theme matrix
       on-resume = ~/.local/bin/stillterm-hyprland stop
   }
   ```

3. In the existing `general` section, add
   `on_lock_cmd = ~/.local/bin/stillterm-hyprland stop`. If that hook already has a
   command, preserve it and append `; ~/.local/bin/stillterm-hyprland stop`.
   Prepend `~/.local/bin/stillterm-hyprland stop; ` to the existing
   `before_sleep_cmd`, keeping its original lock/suspend handling intact.
4. Keep all existing lock listeners, `lock_cmd`, and inhibitor settings. Restart
   your existing Hypridle service using your session's usual mechanism.

The visual listener must remain independent of the lock listener. Closing or
crashing Stillterm must not cancel or restart the lock timeout. Do not add a
second idle daemon or wrap the lock command in a conditional on Stillterm success.

## Uninstall

Stop the running visualizer, remove the listener and only the hook additions above,
then restart the existing Hypridle service. Leave its original lock configuration
in place. Remove the adapter:

```sh
stillterm-hyprland stop
rm "$HOME/.local/bin/stillterm-hyprland"
```

Optionally remove a Cargo-installed CLI with `cargo uninstall stillterm`.
Temporary runtime state disappears with the login session; no desktop preferences
or packaged Omarchy files are changed by the adapter.

## Checks and remaining acceptance

```sh
cargo build --locked
python3 scripts/check-terminal.py --panic-check
python3 scripts/check-hyprland.py
```

The PTY tests exercise the real Rust renderer's arbitrary-key, click, scroll,
signal, panic, and terminal-restoration behavior. The launcher tests use isolated
Unix sockets and real child processes to simulate Hyprland and Foot. They cover
monitor placement, literal argument forwarding, concurrent starts, stale sockets,
startup timeout, lock before/during/after startup, pointer and focus dismissal,
hotplug, renderer failure, invalid IPC, and scoped cleanup. Both test suites run
in Linux CI. Simulation does not validate the real compositor or terminal.

Before calling Milestone 4 complete, record exact desktop/terminal versions and
check:

- Both themes fill every independent monitor, including mixed scaling.
- Keyboard, click, scroll, pointer movement, and focus loss dismiss all windows.
- Repeated starts, hotplug, layout changes, and terminal crashes leave no children.
- Locking takes over promptly; visualizer failure cannot postpone the lock timer.
- Suspend/resume, inhibitors, and the existing lock configuration still behave as before.
- CPU/GPU and memory remain acceptable during animation and after dismissal.
- Installation, upgrade, and removal preserve terminal and desktop preferences.

No physical Linux desktop acceptance or Linux performance measurements have been
completed yet. Automatic Omarchy 4 support is also still blocked by the idle-service
interface described above.

## Development archive

On Linux, `scripts/build-linux.sh` creates an architecture-specific development
archive and SHA-256 checksum in `target/linux`. CI uploads this on its Ubuntu
Rust 1.88 job. It includes the CLI, adapter, license, this guide, and build details
(including the build system's C library version). Extract it and install the two
executables into `~/.local/bin`; Python, Foot, and Hyprland remain system dependencies.
This is a GNU/Linux development build, not a published release or a portable-musl
binary. Check its build details before using it on an older distribution.
