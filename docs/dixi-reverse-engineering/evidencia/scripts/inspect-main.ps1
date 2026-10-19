param(
  [string]$OutDir = "D:\Codigo\Dixi\.tmp-audit\shots",
  [string]$Title  = "Wispr Flow"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }

$sig = @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W2 {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public class W { public IntPtr H; public string T; public string C; public RECT R; public bool V; public IntPtr Par; }
  public static W[] All(uint[] pids) {
    var list = new System.Collections.Generic.List<W>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (System.Array.IndexOf(pids, p) >= 0) {
        var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
        var c = new StringBuilder(512); GetClassNameW(h, c, 512);
        RECT r; GetWindowRect(h, out r);
        list.Add(new W { H = h, T = t.ToString(), C = c.ToString(), R = r, V = IsWindowVisible(h), Par = GetParent(h) });
      }
      return true;
    }, IntPtr.Zero);
    return list.ToArray();
  }
}
'@
Add-Type -TypeDefinition $sig -Language CSharp

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
$pidsArr = [uint32[]]($targetPids | ForEach-Object { [uint32]$_ })
$wins = [W2]::All($pidsArr)

$target = $wins | Where-Object { $_.T -eq $Title } | Select-Object -First 1
if (-not $target) { Write-Output "WINDOW '$Title' NOT FOUND"; exit 1 }

$r = $target.R; $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
Write-Output "TARGET hwnd=$($target.H) title='$($target.T)' class=$($target.C) visible=$($target.V) rect=($($r.Left),$($r.Top)) ${ww}x${hh}"

Write-Output "Showing window (SW_SHOW) ..."
[void][W2]::ShowWindow($target.H, 5)
Start-Sleep -Milliseconds 1200
[void][W2]::SetForegroundWindow($target.H)
Start-Sleep -Milliseconds 1200

# --- capture via PrintWindow (works even if partially occluded) ---
$path = Join-Path $OutDir "main_$($Title -replace '[^A-Za-z0-9]','_').png"
$bmp = New-Object System.Drawing.Bitmap($ww, $hh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [W2]::PrintWindow($target.H, $hdc, 2)
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "SAVED $path (PrintWindow ok=$ok)"

# --- UIA dump of the (now visible) window ---
$sb = New-Object System.Text.StringBuilder
function Esc([string]$s){ if($null -eq $s){return ''}; return ($s -replace "`r?`n",' | ') }
function Walk($el, $depth) {
  if ($depth -gt 16) { return }
  try {
    $ct=$el.Current.ControlType.ProgrammaticName -replace 'ControlType\.',''
    $nm=Esc $el.Current.Name
    $aid=$el.Current.AutomationId
    $cls=$el.Current.ClassName
    $bb=$el.Current.BoundingRectangle
    [void]$sb.AppendLine(('{0}[{1}] name="{2}" aid="{3}" cls="{4}" rect=({5},{6},{7},{8})' -f ('  '*$depth),$ct,$nm,$aid,$cls,[int]$bb.X,[int]$bb.Y,[int]$bb.Width,[int]$bb.Height))
  } catch { [void]$sb.AppendLine(('  '*$depth)+"[ERR] "+$_.Exception.Message); return }
  $c=$null
  try { $c=[System.Windows.Automation.TreeWalker]::ControlViewWalker.GetFirstChild($el) } catch {}
  while ($null -ne $c) {
    Walk $c ($depth+1)
    try { $c=[System.Windows.Automation.TreeWalker]::ControlViewWalker.GetNextSibling($c) } catch { break }
  }
}
$el = [System.Windows.Automation.AutomationElement]::FromHandle($target.H)
if ($el) {
  [void]$sb.AppendLine("=== UIA TREE: $($target.T) ===")
  Walk $el 0
} else { [void]$sb.AppendLine("UIA FromHandle returned null") }

$uiaPath = "D:\Codigo\Dixi\.tmp-audit\uia-main.txt"
[System.IO.File]::WriteAllText($uiaPath, $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))
Write-Output "WROTE $uiaPath chars=$($sb.Length)"

Write-Output "Restoring: hiding window again (SW_HIDE) ..."
[void][W2]::ShowWindow($target.H, 0)
Start-Sleep -Milliseconds 600
Write-Output "DONE"
