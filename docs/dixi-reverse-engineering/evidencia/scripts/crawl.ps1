param(
  [string]$OutRoot = "D:\Codigo\Dixi\.tmp-audit\pages"
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
public class W3 {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr dst, int x, int y, int w, int h, IntPtr src, int sx, int sy, int rop);
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
}
'@
Add-Type -TypeDefinition $sig -Language CSharp

function Esc([string]$s){ if($null -eq $s){return ''}; return ($s -replace "`r?`n",' | ' -replace "`t",' ') }

function DumpTree($el, $depth, $sb) {
  if ($depth -gt 22) { return }
  try {
    $ct=$el.Current.ControlType.ProgrammaticName -replace 'ControlType\.',''
    $nm=Esc $el.Current.Name
    $aid=$el.Current.AutomationId
    $bb=$el.Current.BoundingRectangle
    $st=''
    try { $st=[System.Windows.Automation.AutomationElement]::IsOffscreen($el) } catch {}
    [void]$sb.AppendLine(('{0}[{1}] name="{2}" aid="{3}" off={4} rect=({5},{6},{7},{8})' -f ('  '*$depth),$ct,$nm,$aid,$st,[int]$bb.X,[int]$bb.Y,[int]$bb.Width,[int]$bb.Height))
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

function Capture($hwnd, $path) {
  # strategy 1: GetDC + BitBlt (window's own DC)
  $r = New-Object W3+RECT
  [void][W3]::GetWindowRect($hwnd, [ref]$r)
  $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
  if ($ww -le 0 -or $hh -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap($ww, $hh)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dst = $g.GetHdc()
  $src = [W3]::GetDC($hwnd)
  $ok = [W3]::BitBlt($dst, 0, 0, $ww, $hh, $src, 0, 0, 0x00CC0020)
  [void][W3]::ReleaseDC($hwnd, $src)
  $g.ReleaseHdc($dst)
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  # detect all-black
  $allBlack = $true
  for ($x = 0; $x -lt $ww -and $allBlack; $x += 17) {
    for ($y = 0; $y -lt $hh; $y += 17) {
      $c = $bmp.GetPixel($x, $y)
      if (($c.R + $c.G + $c.B) -gt 24) { $allBlack = $false; break }
    }
  }
  $bmp.Dispose()
  return (-not $allBlack)
}

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
$pidsArr = [uint32[]]($targetPids | ForEach-Object { [uint32]$_ })
$wins = [W3]::All($pidsArr)
$main = $wins | Where-Object { $_.T -eq 'Wispr Flow' } | Select-Object -First 1
if (-not $main) { Write-Output "main window not found"; exit 1 }

[void][W3]::ShowWindow($main.H, 5)
Start-Sleep -Milliseconds 1500
[void][W3]::SetForegroundWindow($main.H)
Start-Sleep -Milliseconds 800

$rootEl = [System.Windows.Automation.AutomationElement]::FromHandle($main.H)

$pages = @(
  @{ id='home';         aid='focus-overlay-target-sidebar-home';        label='Dictado' },
  @{ id='notetaker';    aid='focus-overlay-target-sidebar-notetaker';   label='Tomador de notas' },
  @{ id='insights';     aid='focus-overlay-target-sidebar-insights';    label='Estadisticas' },
  @{ id='dictionary';   aid='focus-overlay-target-sidebar-dictionary';  label='Diccionario' },
  @{ id='snippets';     aid='focus-overlay-target-sidebar-snippets';    label='Fragmentos' },
  @{ id='style';        aid='focus-overlay-target-sidebar-style';       label='Estilo' },
  @{ id='transforms';   aid='focus-overlay-target-sidebar-transforms';  label='Transforms' },
  @{ id='notes';        aid='focus-overlay-target-sidebar-notes';       label='Bloc de notas' }
)

foreach ($p in $pages) {
  Write-Output "--- page: $($p.id) ($($p.label)) ---"
  $rootEl = [System.Windows.Automation.AutomationElement]::FromHandle($main.H)
  $btn = FindByAid $rootEl $p.aid
  if ($btn) {
    try {
      $inv = $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
      $inv.Invoke()
      Write-Output "  invoked ok"
    } catch { Write-Output "  invoke failed: $($_.Exception.Message)" }
  } else { Write-Output "  BUTTON NOT FOUND aid=$($p.aid)" }
  Start-Sleep -Milliseconds 2200
  [void][W3]::SetForegroundWindow($main.H)
  Start-Sleep -Milliseconds 700

  $sb = New-Object System.Text.StringBuilder
  [void]$sb.AppendLine("=== UIA TREE :: $($p.label) ($($p.id)) ===")
  $e2 = [System.Windows.Automation.AutomationElement]::FromHandle($main.H)
  if ($e2) { DumpTree $e2 0 $sb }
  $txtPath = Join-Path $OutRoot "$($p.id).uia.txt"
  [System.IO.File]::WriteAllText($txtPath, $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))
  Write-Output "  tree -> $txtPath ($($sb.Length) chars)"

  $imgPath = Join-Path $OutRoot "$($p.id).png"
  $ok = Capture $main.H $imgPath
  Write-Output "  shot -> $imgPath ok=$ok"
}

Write-Output "Restoring (hide main window) ..."
[void][W3]::ShowWindow($main.H, 0)
Start-Sleep -Milliseconds 500
Write-Output "CRAWL DONE"
