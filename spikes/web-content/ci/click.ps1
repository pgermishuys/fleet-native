# A real left click at screen pixel (x, y), so the page gets an OS mouse event, not a script one.
param([int]$x, [int]$y)
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Mouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
}
"@
[Mouse]::SetCursorPos($x, $y) | Out-Null
Start-Sleep -Milliseconds 150
[Mouse]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
[Mouse]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
