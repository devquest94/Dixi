param(
  [string]$OutRoot = "D:\Codigo\Dixi\.tmp-audit\pages",
  [string]$Title   = "Wispr Flow"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$sig = @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W5 {
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
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
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
    uint dummy;
    uint fg = GetWindowThreadProcessId(GetForegroundWindow(), out dummy);
    uint me = GetCurrentThreadId();
    AttachThreadInput(me, fg, true);
    BringWindowToTop(h); ShowWindow(h, 5); SetForegroundWindow(h); SetActiveWindow(h);
    AttachThreadInput(me, fg, false);
  }
  public static void Esc() {
    keybd_event(0x1B, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    keybd_event(0x1B, 0, 2, UIntPtr.Zero);
  }
  public static void Click(int x, int y) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(120);
    mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero);
  }
}
'@
Add-Type -TypeDefinition $sig -Language CSharp

function Esc([string]$s){ if($null -eq $s){return ''}; return ($s -replace "`r?`n",' | ' -replace "`t",' ') }
function DumpTree($el, $depth, $sb) {
  if ($depth -gt 24) { return }
  try {
    $ct=$el.Current.ControlType.ProgrammaticName -replace 'ControlType\.',''
    $nm=Esc $el.Current.Name
    $aid=$el.Current.AutomationId
    $bb=$el.Current.BoundingRectangle
    [void]$sb.AppendLine(('{0}[{1}] name="{2}" aid="{3}" rect=({4},{5},{6},{7})' -f ('  '*$depth),$ct,$nm,$aid,[int]$bb.X,[int]$bb.Y,[int]$bb.Width,[int]$bb.Height))
  } catch { [void]$sb.AppendLine(('  '*$depth)+"[ERR] "+$_.Exception.Message); return }
  $c=$null
  try { $c=[System.Windows.Automation.TreeWalker]::ControlViewWalker.GetFirstChild($el) } catch {}
  while ($null -ne $c) {
    DumpTree $c ($depth+1) $sb
    try { $c=[System.Windows.Automation.TreeWalker]::ControlViewWalker.GetNextSibling($c) } catch { break }
  }
}
function FindByAid($root, [string]$aid) {
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $aid)
  return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}
function Grab($hwnd, $path) {
  $r = New-Object W5+RECT
  [void][W5]::GetWindowRect($hwnd, [ref]$r)
  $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($ww, $hh)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($ww, $hh)))
  $g.Dispose(); $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
  Write-Output "  shot -> $(Split-Path $path -Leaf) (${ww}x${hh})"
}

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
$pidsArr = [uint32[]]($targetPids | ForEach-Object { [uint32]$_ })
$main = [W5]::All($pidsArr) | Where-Object { $_.T -eq $Title } | Select-Object -First 1
if (-not $main) { Write-Output "not found"; exit 1 }

[W5]::ForceForeground($main.H)
Start-Sleep -Milliseconds 1200
Write-Output "Sending ESC x3 to dismiss overlays ..."
1..3 | ForEach-Object { [W5]::Esc(); Start-Sleep -Milliseconds 700 }
Start-Sleep -Milliseconds 800
Grab $main.H (Join-Path $OutRoot "00-after-esc.png")

$sb = New-Object System.Text.StringBuilder
[void]$sb.AppendLine("=== TREE AFTER ESC ===")
$e = [System.Windows.Automation.AutomationElement]::FromHandle($main.H)
if ($e) { DumpTree $e 0 $sb }
[System.IO.File]::WriteAllText("D:\Codigo\Dixi\.tmp-audit\after-esc.uia.txt", $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))
Write-Output "tree chars=$($sb.Length)"

Write-Output "Restoring (hide) ..."
[void][W5]::ShowWindow($main.H, 0)
Start-Sleep -Milliseconds 400
Write-Output "OK"
