$ErrorActionPreference = 'Continue'
$all = Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }
$iso = @($all | Where-Object { $_.CommandLine -like '*isolated-profile*' })
$isoIds = @($iso | ForEach-Object { $_.ProcessId })
# helper processes parented by the isolated main process
$isoHelper = @($all | Where-Object { $_.Name -like '*Helper*' -and $isoIds -contains $_.ParentProcessId })
$targets = @($iso + $isoHelper) | Sort-Object ProcessId -Unique
Write-Output "ISOLATED processes to terminate: $(($targets | ForEach-Object { $_.ProcessId }) -join ', ')"
foreach ($p in $targets) {
  try {
    Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop
    Write-Output "  stopped PID $($p.ProcessId) ($($p.Name))"
  } catch {
    Write-Output "  could not stop PID $($p.ProcessId): $($_.Exception.Message)"
  }
}
Start-Sleep -Seconds 3
Write-Output ""
Write-Output "=== VERIFY LIVE INSTANCE HEALTH ==="
$live = Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }
Write-Output "  wispr processes remaining: $(($live | ForEach-Object { $_.ProcessId }) -join ', ')"
$main = $live | Where-Object { $_.CommandLine -notmatch '--type=' -and $_.CommandLine -notlike '*Helper*' }
foreach ($m in $main) {
  $ud = 'LIVE'
  if ($m.CommandLine -match 'isolated-profile') { $ud = 'ISOLATED' }
  $ws = (Get-Process -Id $m.ProcessId -ErrorAction SilentlyContinue).WorkingSet64
  Write-Output ("  PID {0} name='{1}' profile={2} WS={3}MB" -f $m.ProcessId, $m.Name, $ud, [math]::Round($ws/1MB,1))
}
$helper = $live | Where-Object { $_.Name -like '*Helper*' }
foreach ($h in $helper) {
  Write-Output ("  HELPER PID {0} parent={1} WS={2}MB" -f $h.ProcessId, $h.ParentProcessId, [math]::Round((Get-Process -Id $h.ProcessId -ErrorAction SilentlyContinue).WorkingSet64/1MB,1))
}
Write-Output ""
Write-Output "  port 9333 still listening: $([bool](Get-NetTCPConnection -State Listen -LocalPort 9333 -ErrorAction SilentlyContinue))"
Write-Output "  isolated profile dir still present: $(Test-Path 'D:\Codigo\Dixi\.tmp-audit\isolated-profile')"
Write-Output ""
Write-Output "=== LIVE WINDOW CHECK ==="
$sig = @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W6 {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public static string[] Titles(uint[] pids) {
    var list = new System.Collections.Generic.List<string>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (System.Array.IndexOf(pids, p) >= 0) {
        var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
        list.Add((IsWindowVisible(h) ? "[V] " : "[H] ") + t.ToString());
      }
      return true;
    }, IntPtr.Zero);
    return list.ToArray();
  }
}
'@
Add-Type -TypeDefinition $sig -Language CSharp
$liveIds = [uint32[]](@($live | ForEach-Object { [uint32]$_.ProcessId }))
[W6]::Titles($liveIds) | ForEach-Object { Write-Output "  $_" }
