# Build a development .scr; never install it or change the selected screensaver.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target'
cargo build --release --locked -p stillterm-windows --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Windows screensaver build failed' }
$out = Join-Path $env:CARGO_TARGET_DIR 'windows'
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item 'target/x86_64-pc-windows-msvc/release/stillterm-windows.exe' "$out/Stillterm.scr" -Force
Copy-Item 'LICENSE' "$out/LICENSE" -Force
@'
Stillterm — Windows screensaver development build

Keep Stillterm.scr in a permanent folder. Right-click it, choose Show more options
if needed, then Install to open Windows Screen Saver Settings. Choose Stillterm,
open Settings to configure it, and use Preview to test it.

Command-line modes: /s = full screen; /c = settings; /p HWND = embedded preview.
No arguments opens settings. Both Monochrome and Matrix themes are included.

The file is unsigned and may trigger Windows download/reputation warnings.
This is a development build, not a signed public release.

Windows controls locking and sign-in requirements. Configure "On resume, display
logon screen" in Windows; Stillterm does not change that option.

To remove: select a different screensaver first, then delete Stillterm.scr.
Settings are stored in %APPDATA%\Stillterm\screensaver.toml; remove that file only
if you also want to reset preferences.

Source, testing checklist, and limitations:
https://github.com/saeedet/Stillterm/blob/main/docs/windows.md
'@ | Set-Content "$out/Read Me.txt" -Encoding utf8
Compress-Archive -Path "$out/Stillterm.scr", "$out/LICENSE", "$out/Read Me.txt" -DestinationPath "$out/Stillterm-windows-development.zip" -Force
(Get-FileHash "$out/Stillterm-windows-development.zip" -Algorithm SHA256).Hash.ToLowerInvariant() + '  Stillterm-windows-development.zip' |
    Set-Content "$out/SHA256SUMS.txt" -Encoding ascii
Write-Output "Built $out/Stillterm.scr"
