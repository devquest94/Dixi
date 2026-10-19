param([string]$OutDir = "D:\Codigo\Dixi\.tmp-audit\shots")
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }

$sig = @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Cap {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public class W { public IntPtr H; public string T; public RECT R; }
  public static W[] Vis(uint[] pids) {
    var list = new System.Collections.Generic.List<W>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (System.Array.IndexOf(pids, p) >= 0) {
        var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
        RECT r; GetWindowRect(h, out r);
        if (IsWindowVisible(h) && (r.Right - r.Left) > 0) list.Add(new W { H = h, T = t.ToString(), R = r });
      }
      return true;
    }, IntPtr.Zero);
    return list.ToArray();
  }
}
'@
Add-Type -TypeDefinition $sig -Language CSharp

function Save-Screen($x, $y, $w, $h, $path) {
  if ($w -le 0 -or $h -le 0) { return }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Output "SAVED $path ($w x $h) from ($x,$y)"
}

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
$pidsArr = [uint32[]]($targetPids | ForEach-Object { [uint32]$_ })

Write-Output "=== CAPTURING VISIBLE WISPR WINDOWS ==="
foreach ($w in [Cap]::Vis($pidsArr)) {
  $r = $w.R
  $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
  $safe = ($w.T -replace '[^A-Za-z0-9]', '_')
  if ([string]::IsNullOrWhiteSpace($safe)) { $safe = 'untitled' }
  Save-Screen $r.Left $r.Top $ww $hh (Join-Path $OutDir "win_$safe.png")
}

Write-Output ""
Write-Output "=== CAPTURING FULL DESKTOP (primary) ==="
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
Save-Screen $b.X $b.Y $b.Width $b.Height (Join-Path $OutDir "desktop_primary.png")

Write-Output ""
Write-Output "=== CAPTURING FULL DESKTOP (secondary) ==="
$b2 = ([System.Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary } | Select-Object -First 1).Bounds
Save-Screen $b2.X $b2.Y $b2.Width $b2.Height (Join-Path $OutDir "desktop_secondary.png")
