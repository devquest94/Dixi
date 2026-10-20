<#
  measure-dixi-tree.ps1 — mide el consumo del ARBOL COMPLETO de procesos de Dixi.

  El arbol incluye:
    - dixi.exe (proceso principal de la app)
    - msedgewebview2.exe (el runtime WebView2, varios procesos)
    - dixi.exe (el worker aislado de transcribe.cpp)

  Se reportan DOS magnitudes de memoria, porque no son lo mismo:
    - RAM privada (commit propio): lo que la app reserva de verdad.
    - Working set: lo que ocupa en RAM incluyendo paginas compartidas con
      el runtime de WebView2, que es un componente del sistema compartido.

  Uso:
    powershell -File scripts/measure-dixi-tree.ps1 [-Seconds 20] [-IntervalMs 250] [-Label baseline]
#>
param(
    [int]$Seconds = 20,
    [int]$IntervalMs = 250,
    [string]$Label = 'baseline',
    [string]$Out
)

$ErrorActionPreference = 'Continue'

function Get-DixiTree {
    $all = Get-CimInstance Win32_Process
    $roots = @($all | Where-Object { $_.Name -eq 'dixi.exe' })
    if ($roots.Count -eq 0) { return @() }
    $ids = @($roots | ForEach-Object { $_.ProcessId })
    $tree = @($roots)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($p in $all) {
            if (($ids -contains $p.ParentProcessId) -and -not ($ids -contains $p.ProcessId)) {
                $ids += $p.ProcessId
                $tree += $p
                $changed = $true
            }
        }
    }
    return $tree
}

function Get-TreeSnapshot($tree) {
    $mem = @{}
    foreach ($t in $tree) {
        $gp = Get-Process -Id $t.ProcessId -ErrorAction SilentlyContinue
        if (-not $gp) { continue }
        $mem[$t.ProcessId] = [pscustomobject]@{
            Name       = $t.Name
            Parent     = $t.ParentProcessId
            PrivateMB  = [math]::Round($gp.PrivateMemorySize64 / 1MB, 1)
            WorkingMB  = [math]::Round($gp.WorkingSet64 / 1MB, 1)
            CpuSeconds = $gp.TotalProcessorTime.TotalSeconds
        }
    }
    return $mem
}

Write-Output "[tree] midiendo '$Label' durante $Seconds s (intervalo ${IntervalMs}ms)"
$samples = @()
$prev = @{}
$sw = [System.Diagnostics.Stopwatch]::StartNew()

while ($sw.Elapsed.TotalSeconds -lt $Seconds) {
    $tree = Get-DixiTree
    $snap = Get-TreeSnapshot $tree
    $now = $sw.Elapsed.TotalSeconds

    $priv = 0.0; $ws = 0.0; $cpuDelta = 0.0
    $byName = @{}
    foreach ($pidKey in $snap.Keys) {
        $s = $snap[$pidKey]
        $priv += $s.PrivateMB
        $ws += $s.WorkingMB
        if ($prev.ContainsKey($pidKey)) {
            $dt = $now - $prev[$pidKey].T
            if ($dt -gt 0) { $cpuDelta += ($s.CpuSeconds - $prev[$pidKey].C) / $dt * 100 }
        }
        $k = $s.Name
        if (-not $byName.ContainsKey($k)) { $byName[$k] = @{ private = 0.0; ws = 0.0; n = 0 } }
        $byName[$k].private += $s.PrivateMB
        $byName[$k].ws += $s.WorkingMB
        $byName[$k].n++
    }

    $samples += [pscustomobject]@{
        t = [math]::Round($now, 2)
        privateMB = [math]::Round($priv, 1)
        workingMB = [math]::Round($ws, 1)
        cpuPct = [math]::Round($cpuDelta, 2)
        processes = $snap.Count
    }

    $prev = @{}
    foreach ($pidKey in $snap.Keys) {
        $prev[$pidKey] = @{ C = $snap[$pidKey].CpuSeconds; T = $now }
    }
    Start-Sleep -Milliseconds $IntervalMs
}

Write-Output ""
Write-Output "=== ARBOL DE PROCESOS (detalle) ==="
$final = Get-TreeSnapshot (Get-DixiTree)
$dixiIds = @($final.Keys | Where-Object { $final[$_].Name -eq 'dixi.exe' })
foreach ($k in ($final.Keys | Sort-Object { $final[$_].Name }, { $final[$_].PrivateMB } -Descending)) {
    $s = $final[$k]
    $role = ''
    if ($s.Name -eq 'dixi.exe') {
        # El worker de ASR es un dixi.exe cuyo PADRE es otro dixi.exe.
        if ($dixiIds -contains $s.Parent) { $role = '  <- worker ASR' } else { $role = '  <- app principal' }
    } elseif ($s.Name -eq 'msedgewebview2.exe') { $role = '  <- WebView2' }
    "  {0,-20} pid={1,-7} privado={2,7:N1} MB  ws={3,7:N1} MB{4}" -f $s.Name, $k, $s.PrivateMB, $s.WorkingMB, $role
}

Write-Output ""
Write-Output "=== AGREGADO POR TIPO ==="
$agg = @{}
foreach ($k in $final.Keys) {
    $n = $final[$k].Name
    if (-not $agg.ContainsKey($n)) { $agg[$n] = @{ private = 0.0; ws = 0.0; n = 0 } }
    $agg[$n].private += $final[$k].PrivateMB
    $agg[$n].ws += $final[$k].WorkingMB
    $agg[$n].n++
}
foreach ($n in ($agg.Keys | Sort-Object { $agg[$_].private } -Descending)) {
    "  {0,-20} x{1,-3} privado={2,7:N1} MB  ws={3,7:N1} MB" -f $n, $agg[$n].n, $agg[$n].private, $agg[$n].ws
}

Write-Output ""
Write-Output "=== SERIE (muestras) ==="
$samples | ForEach-Object { "  t={0,6:N2}s  privado={1,7:N1} MB  ws={2,7:N1} MB  cpu={3,6:N2}%" -f $_.t, $_.privateMB, $_.workingMB, $_.cpuPct }

$privMax = ($samples | Measure-Object privateMB -Maximum).Maximum
$privAvg = ($samples | Measure-Object privateMB -Average).Average
$wsMax = ($samples | Measure-Object workingMB -Maximum).Maximum
$cpuMax = ($samples | Measure-Object cpuPct -Maximum).Maximum
$cpuAvg = ($samples | Measure-Object cpuPct -Average).Average
$procMax = ($samples | Measure-Object processes -Maximum).Maximum

Write-Output ""
Write-Output "=== RESUMEN '$Label' ==="
"  RAM privada (commit) : max {0:N1} MB | media {1:N1} MB" -f $privMax, $privAvg
"  Working set (total)  : max {0:N1} MB | media {1:N1} MB" -f $wsMax, (($samples | Measure-Object workingMB -Average).Average)
"  CPU (arbol completo) : max {0:N1} % | media {1:N1} %   (100 % = 1 nucleo saturado)" -f $cpuMax, $cpuAvg
"  Procesos del arbol   : max {0}" -f $procMax

if ($Out) {
    [pscustomobject]@{
        label = $Label
        samples = $samples
        private_max_mb = [math]::Round($privMax, 1)
        private_avg_mb = [math]::Round($privAvg, 1)
        working_max_mb = [math]::Round($wsMax, 1)
        cpu_max_pct = [math]::Round($cpuMax, 2)
        cpu_avg_pct = [math]::Round($cpuAvg, 2)
        processes_max = $procMax
    } | ConvertTo-Json -Depth 6 | Out-File -FilePath $Out -Encoding utf8
    Write-Output "[tree] informe en $Out"
}
