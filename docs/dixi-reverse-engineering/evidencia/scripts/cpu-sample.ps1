param([int]$Seconds = 20)
$ErrorActionPreference = 'Continue'

$targetPids = (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId
$logical = (Get-CimInstance Win32_Processor).NumberOfLogicalProcessors

function Snap($pids) {
  $h = @{}
  foreach ($p in $pids) {
    try {
      $pr = Get-Process -Id $p -ErrorAction Stop
      $h[$p] = [pscustomobject]@{
        Name = $pr.ProcessName
        Cpu  = [double]$pr.CPU
        WS   = [double]$pr.WorkingSet64
        PM   = [double]$pr.PrivateMemorySize64
        Thr  = $pr.Threads.Count
        Handles = $pr.HandleCount
      }
    } catch { }
  }
  return $h
}

$sysA = (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average
$s1 = Snap $targetPids
Start-Sleep -Seconds $Seconds
$s2 = Snap $targetPids
$sysB = (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average

Write-Output "=== WISPR FLOW - CPU EN REPOSO (muestra de $Seconds s) ==="
Write-Output ("  nucleos logicos: {0} | carga sistema inicio: {1} pct | fin: {2} pct" -f $logical, $sysA, $sysB)
Write-Output ""

$totalDelta = 0.0
$rows = @()
foreach ($k in $s2.Keys) {
  if ($s1.ContainsKey($k)) {
    $d = $s2[$k].Cpu - $s1[$k].Cpu
    $totalDelta = $totalDelta + $d
    $rows += [pscustomobject]@{
      PID = $k
      Name = $s2[$k].Name
      CpuPctCore = [math]::Round(($d / $Seconds) * 100, 2)
      CpuPctMachine = [math]::Round(($d / $Seconds) * 100 / $logical, 3)
      WS_MB = [math]::Round($s2[$k].WS / 1MB, 1)
      PM_MB = [math]::Round($s2[$k].PM / 1MB, 1)
      Threads = $s2[$k].Thr
      Handles = $s2[$k].Handles
    }
  }
}
$rows | Sort-Object CpuPctCore -Descending | Format-Table -AutoSize | Out-String -Width 200

$pctCore = [math]::Round(($totalDelta / $Seconds) * 100, 2)
$pctMach = [math]::Round(($totalDelta / $Seconds) * 100 / $logical, 3)
Write-Output ("  CPU TOTAL Wispr: {0} s de tiempo de procesador en {1} s de reloj" -f [math]::Round($totalDelta,2), $Seconds)
Write-Output ("  = {0} pct de un nucleo | {1} pct de la maquina" -f $pctCore, $pctMach)
Write-Output ""

$ws = ($s2.Values | Measure-Object -Property WS -Sum).Sum
$pm = ($s2.Values | Measure-Object -Property PM -Sum).Sum
$thr = ($s2.Values | Measure-Object -Property Thr -Sum).Sum
$hnd = ($s2.Values | Measure-Object -Property Handles -Sum).Sum
Write-Output "=== MEMORIA ==="
Write-Output ("  working set total : {0} MB" -f [math]::Round($ws/1MB,1))
Write-Output ("  private memory    : {0} MB" -f [math]::Round($pm/1MB,1))
Write-Output ("  hilos totales     : {0}" -f $thr)
Write-Output ("  handles totales   : {0}" -f $hnd)
Write-Output ""

$os = Get-CimInstance Win32_OperatingSystem
$usedPct = [math]::Round((($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / $os.TotalVisibleMemorySize) * 100, 1)
Write-Output "=== SISTEMA ==="
Write-Output ("  RAM total {0} MB | libre {1} MB | usada {2} MB ({3} pct)" -f [math]::Round($os.TotalVisibleMemorySize/1KB,0), [math]::Round($os.FreePhysicalMemory/1KB,0), [math]::Round(($os.TotalVisibleMemorySize-$os.FreePhysicalMemory)/1KB,0), $usedPct)
Write-Output ("  procesos Wispr: {0}" -f $s2.Count)
