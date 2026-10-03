# Linux / Hyprland

Milestone 4 reuses Stillterm's Rust terminal renderer in one Foot window per
monitor. A small Python standard-library adapter will own those windows only
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
will not replace packaged commands, shadow their names on PATH, or install
Hypridle alongside Omarchy's existing idle service.

Automatic Omarchy support needs a configurable launch/stop interface and a lock
schedule that survives visualizer failure. Until then, use manual preview only.

Reviewed sources: [Omarchy idle service](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/shell/plugins/services/idle/Service.qml),
[launcher](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/bin/omarchy-launch-screensaver),
[locking](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/bin/omarchy-system-lock),
[Hyprland IPC](https://wiki.hypr.land/IPC/),
[Hypridle](https://wiki.hypr.land/Hypr-Ecosystem/hypridle/),
[Foot manual](https://codeberg.org/dnkl/foot/src/tag/1.27.0/doc/foot.1.scd).
