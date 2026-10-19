Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

$sig = @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
public class W32 {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public class WinInfo { public IntPtr H; public uint Pid; public string Title; public string Cls; public bool Vis; public bool Min; public RECT R; public IntPtr Owner; public IntPtr Parent; }
  public static List<WinInfo> All() {
    var list = new List<WinInfo>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
      var c = new StringBuilder(512); GetClassNameW(h, c, 512);
      RECT r; GetWindowRect(h, out r);
      list.Add(new WinInfo { H = h, Pid = p, Title = t.ToString(), Cls = c.ToString(), Vis = IsWindowVisible(h), Min = IsIconic(h), R = r, Owner = GetWindow(h, 4), Parent = GetParent(h) });
      return true;
    }, IntPtr.Zero);
    return list;
  }
  public static List<WinInfo> ForPids(uint[] pids) {
    return All().Where(w => pids.Contains(w.Pid)).ToList();
  }
}
'@
Add-Type -TypeDefinition $sig -Language CSharp

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
Write-Output "WisprFlow PIDs: $($targetPids -join ', ')"
$pidsArr = [uint32[]]($targetPids | ForEach-Object { [uint32]$_ })

$wins = [W32]::ForPids($pidsArr)
Write-Output "=== TOP-LEVEL WINDOWS OWNED BY WISPRFLOW PROCESSES ==="
Write-Output ("{0,-8} {1,-10} {1,-8} {2,-42} {3,-30} {4}" -f 'HWND','PID','VIS','CLASS','TITLE','RECT')
foreach ($w in $wins | Sort-Object Pid) {
  $r = $w.R
  Write-Output ("{0,-8} {1,-10} {2,-8} {3,-42} {4,-30} {5},{6} {7}x{8}" -f `
    $w.H, $w.Pid, $w.Vis, $w.Cls, ($w.Title -replace "`r?`n",' '), $r.Left, $r.Top, ($r.Right-$r.Left), ($r.Bottom-$r.Top))
}

Write-Output ""
Write-Output "=== SCREEN / MONITORS ==="
foreach ($s in [System.Windows.Forms.Screen]::AllScreens) {
  Write-Output ("Primary={0} Bounds={1} WorkingArea={2}" -f $s.Primary, $s.Bounds, $s.WorkingArea)
}
