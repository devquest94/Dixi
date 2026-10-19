param(
  [string]$OutRoot = "D:\Codigo\Dixi\.tmp-audit\pages",
  [string]$Title   = "Wispr Flow"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
if (-not (Test-Path $OutRoot)) { New-Item -ItemType Directory -Path $OutRoot | Out-Null }

$sig = @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W4 {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SetActiveWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public class W { public IntPtr H; public string T; public RECT R; public bool V; }
  public static W[] All(uint[] pids) {
    var list = new System.Collections.Generic.List<W>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (System.Array.IndexOf(pids, p) >= 0) {
        var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
        RECT r; GetWindowRect(h, out r);
        list.Add(new W { H = h, T = t.ToString(), R = r, V = IsWindowVisible(h) });
      }
      return true;
    }, IntPtr.Zero);
    return list.ToArray();
  }
  public static void ForceForeground(IntPtr h) {
    uint dummyPid;
    uint fg = GetWindowThreadProcessId(GetForegroundWindow(), out dummyPid);
    uint me = GetCurrentThreadId();
    AttachThreadInput(me, fg, true);
    BringWindowToTop(h);
    ShowWindow(h, 5);
    SetForegroundWindow(h);
    SetActiveWindow(h);
    AttachThreadInput(me, fg, false);
  }
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
'@
Add-Type -TypeDefinition $sig -Language CSharp

$pages = @(
  @{ id='home';       aid='focus-overlay-target-sidebar-home' },
  @{ id='notetaker';  aid='focus-overlay-target-sidebar-notetaker' },
  @{ id='insights';   aid='focus-overlay-target-sidebar-insights' },
  @{ id='dictionary'; aid='focus-overlay-target-sidebar-dictionary' },
  @{ id='snippets';   aid='focus-overlay-target-sidebar-snippets' },
  @{ id='style';      aid='focus-overlay-target-sidebar-style' },
  @{ id='transforms'; aid='focus-overlay-target-sidebar-transforms' },
  @{ id='notes';      aid='focus-overlay-target-sidebar-notes' }
)

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
$pidsArr = [uint32[]]($targetPids | ForEach-Object { [uint32]$_ })
$wins = [W4]::All($pidsArr)
$main = $wins | Where-Object { $_.T -eq $Title } | Select-Object -First 1
if (-not $main) { Write-Output "main window not found"; exit 1 }

[W4]::ForceForeground($main.H)
Start-Sleep -Milliseconds 1500

function Grab($hwnd, $path) {
  $r = New-Object W4+RECT
  [void][W4]::GetWindowRect($hwnd, [ref]$r)
  $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($ww, $hh)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($ww, $hh)))
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Output "  shot -> $path (${ww}x${hh} at $($r.Left),$($r.Top))"
}

function FindByAid($root, [string]$aid) {
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $aid)
  return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

foreach ($p in $pages) {
  [W4]::ForceForeground($main.H)
  Start-Sleep -Milliseconds 500
  $rootEl = [System.Windows.Automation.AutomationElement]::FromHandle($main.H)
  $btn = FindByAid $rootEl $p.aid
  if ($btn) {
    try { ([System.Windows.Automation.InvokePattern]$btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)).Invoke() } catch { Write-Output "  invoke err: $($_.Exception.Message)" }
  }
  Start-Sleep -Milliseconds 2500
  [W4]::ForceForeground($main.H)
  Start-Sleep -Milliseconds 1200
  Grab $main.H (Join-Path $OutRoot "$($p.id).png")
}

Write-Output "Restoring (hide) ..."
[void][W4]::ShowWindow($main.H, 0)
Start-Sleep -Milliseconds 500
Write-Output "SHOTS DONE"
