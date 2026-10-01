# Exercise the real executable in a disposable preview host; never change settings.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$binary = (Resolve-Path 'target/windows/Stillterm.scr').Path
Add-Type @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
public static class StilltermPreviewTest {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct Msg {
        public IntPtr Hwnd; public uint Message; public UIntPtr WParam;
        public IntPtr LParam; public uint Time; public Point Pt; public uint Private;
    }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr CreateWindowExW(uint ex, string cls, string title, uint style, int x, int y, int w, int h, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr data);
    [DllImport("user32.dll")] public static extern bool DestroyWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool PeekMessageW(out Msg msg, IntPtr hwnd, uint min, uint max, uint remove);
    [DllImport("user32.dll")] static extern bool TranslateMessage(ref Msg msg);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessageW(ref Msg msg);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, UIntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int w, int h, uint flags);
    public static IntPtr Create() { return CreateWindowExW(0, "STATIC", "Stillterm preview test", 0x90000000, 0, 0, 320, 200, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero); }
    public static void Pump(int milliseconds) {
        var clock = Stopwatch.StartNew();
        while (clock.ElapsedMilliseconds < milliseconds) {
            Msg msg;
            while (PeekMessageW(out msg, IntPtr.Zero, 0, 0, 1)) { TranslateMessage(ref msg); DispatchMessageW(ref msg); }
            Thread.Sleep(10);
        }
    }
}
'@
$hostWindow = [StilltermPreviewTest]::Create()
if ($hostWindow -eq [IntPtr]::Zero) { throw 'Cannot create preview test host' }
$process = $null
try {
    $start = [System.Diagnostics.ProcessStartInfo]::new($binary, "/p $($hostWindow.ToInt64())")
    $start.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($start)
    [StilltermPreviewTest]::Pump(1500)
    if ($process.HasExited) { throw 'Embedded preview exited before its parent' }
    $child = [StilltermPreviewTest]::GetWindow($hostWindow, 5)
    if ($child -eq [IntPtr]::Zero) { throw 'Screensaver did not create an embedded child' }
    [StilltermPreviewTest]::SendMessageW($child, 0x0100, [UIntPtr]27, [IntPtr]::Zero) | Out-Null
    [StilltermPreviewTest]::SendMessageW($child, 0x0201, [UIntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    [StilltermPreviewTest]::SetWindowPos($hostWindow, [IntPtr]::Zero, 0, 0, 480, 300, 0x14) | Out-Null
    [StilltermPreviewTest]::Pump(500)
    if ($process.HasExited) { throw 'Preview input incorrectly dismissed the saver' }
    $rect = [StilltermPreviewTest+Rect]::new()
    if (-not [StilltermPreviewTest]::GetClientRect($child, [ref]$rect) -or $rect.Right -ne 480 -or $rect.Bottom -ne 300) {
        throw 'Embedded preview did not follow parent resize'
    }
    [StilltermPreviewTest]::DestroyWindow($hostWindow) | Out-Null
    $hostWindow = [IntPtr]::Zero
    [StilltermPreviewTest]::Pump(500)
    if (-not $process.WaitForExit(5000)) { throw 'Preview retained a process after parent destruction' }
    Write-Output 'PASS real .scr embedding, preview input, resize, and parent destruction'
} finally {
    if ($hostWindow -ne [IntPtr]::Zero) { [StilltermPreviewTest]::DestroyWindow($hostWindow) | Out-Null }
    if ($null -ne $process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
}
