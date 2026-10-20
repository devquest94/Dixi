<#
  send-shortcut.ps1 — envia una combinacion de teclas al sistema.

  Sirve para ejercitar el atajo global de Dixi de forma automatica, sin
  depender de la voz ni de la intervencion manual.

  Uso:
    powershell -File scripts/send-shortcut.ps1 -Keys ctrl+space -HoldMs 1500
    powershell -File scripts/send-shortcut.ps1 -Keys escape

  Notas:
   - Se usa keybd_event (SendInput clasico): los hooks de bajo nivel que
     usa el crate handy-keys (rdev) reciben los eventos inyectados.
   - -HoldMs mantiene pulsadas las teclas ese tiempo (modo push-to-talk).
#>
param(
    [Parameter(Mandatory = $true)][string]$Keys,
    [int]$HoldMs = 60,
    [int]$GapMs = 40
)

$ErrorActionPreference = 'Continue'

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Kb {
    [StructLayout(LayoutKind.Sequential)]
    public struct KEYBDINPUT { public uint wVk; public uint wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)]
    public struct INPUT { public uint type; public KEYBDINPUT ki; }
    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint SendInput(uint n, INPUT[] p, int cb);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);

    const uint KEYEVENTF_EXTENDEDKEY = 0x0001;

    public static void Down(byte vk) {
        if (IsExtended(vk)) keybd_event(vk, 0, KEYEVENTF_EXTENDEDKEY, IntPtr.Zero);
        else keybd_event(vk, 0, 0, IntPtr.Zero);
    }
    public static void Up(byte vk) {
        if (IsExtended(vk)) keybd_event(vk, 0, KEYEVENTF_EXTENDEDKEY | 0x0002, IntPtr.Zero);
        else keybd_event(vk, 0, 0x0002, IntPtr.Zero);
    }
    static bool IsExtended(byte vk) {
        return vk == 0xA5 || vk == 0xA3 || vk == 0xA4 || vk == 0xA2
            || vk == 0x21 || vk == 0x22 || vk == 0x23 || vk == 0x24 || vk == 0x25
            || vk == 0x26 || vk == 0x27 || vk == 0x28 || vk == 0x2D || vk == 0x2E;
    }
}
'@

$map = @{
    ctrl = 0x11; control = 0x11; alt = 0x12; shift = 0x10; win = 0x5B
    space = 0x20; escape = 0x1B; esc = 0x1B; enter = 0x0D; return = 0x0D
    tab = 0x09; backspace = 0x08
    a = 0x41; b = 0x42; c = 0x43; d = 0x44; e = 0x45; f = 0x46; g = 0x47
    h = 0x48; i = 0x49; j = 0x4A; k = 0x4B; l = 0x4C; m = 0x4D; n = 0x4E
    o = 0x4F; p = 0x50; q = 0x51; r = 0x52; s = 0x53; t = 0x54; u = 0x55
    v = 0x56; w = 0x57; x = 0x58; y = 0x59; z = 0x5A
}

$parts = $Keys.ToLower().Split('+') | ForEach-Object { $_.Trim() }
$codes = @()
foreach ($p in $parts) {
    if (-not $map.ContainsKey($p)) { Write-Error "tecla desconocida: '$p'"; exit 2 }
    $codes += [byte]$map[$p]
}

foreach ($c in $codes) { [Kb]::Down($c) }
Start-Sleep -Milliseconds $HoldMs
[Array]::Reverse($codes)
foreach ($c in $codes) { [Kb]::Up($c) }
Start-Sleep -Milliseconds $GapMs
Write-Output "[keys] enviada: $Keys (mantenida ${HoldMs}ms)"
