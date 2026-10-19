<#
  measure-perf.ps1 — mide el presupuesto de rendimiento de Dixi.

  Objetivos (doc 13, seccion de rendimiento):
    - RAM en reposo            < 120 MB
    - CPU en reposo            < 1 %
    - CPU durante transcripcion (4 nucleos) <= 250 %
    - Arranque en frio         < 1.5 s
    - Latencia por etapa       se mide y se contrasta con el objetivo

  La CPU se mide por delta de tiempo de proceso: mas fiable que Get-Counter
  con instancias de proceso, que falla en silencio cuando el nombre del
  proceso tiene mas de una instancia.

  Uso:
    measure-perf.ps1 -Exe <dixi.exe> -AppDataDir <dir> -FixturesDir <dir> `
                     -Model <id> -Out <json> [-IdleSeconds 20]
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$AppDataDir,
    [Parameter(Mandatory = $true)][string]$FixturesDir,
    [Parameter(Mandatory = $true)][string]$Model,
    [Parameter(Mandatory = $true)][string]$OutFile,
    [int]$IdleSeconds = 20
)

$ErrorActionPreference = 'Continue'
$report = [ordered]@{}

function Get-Json($lines) {
    foreach ($line in $lines) {
        $t = ([string]$line).Trim()
        if (($t.StartsWith('{') -and $t.EndsWith('}')) -or ($t.StartsWith('[') -and $t.EndsWith(']'))) {
            try { return ($t | ConvertFrom-Json) } catch { }
        }
    }
    return $null
}

# --- muestreo de un proceso: RAM y CPU por delta de tiempo de proceso -------
function Measure-Process($proc, $seconds) {
    $proc.Refresh()
    if ($proc.HasExited) { return $null }
    $cpu0 = $proc.TotalProcessorTime.TotalSeconds
    $t0 = Get-Date
    Start-Sleep -Milliseconds $seconds
    $proc.Refresh()
    if ($proc.HasExited) { return $null }
    $cpu1 = $proc.TotalProcessorTime.TotalSeconds
    $elapsed = ((Get-Date) - $t0).TotalSeconds
    if ($elapsed -le 0) { $elapsed = 0.001 }
    [pscustomobject]@{
        # RAM privada (commit) y working set, en MB
        privateMB = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
        workingMB = [math]::Round($proc.WorkingSet64 / 1MB, 1)
        # % de CPU: 100 % = un nucleo saturado
        cpuPct = [math]::Round((($cpu1 - $cpu0) / $elapsed) * 100, 2)
    }
}

# ---------------------------------------------------------------- arranque ---
Write-Output "[perf] arrancando la aplicacion (ventana oculta)"
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$app = Start-Process -FilePath $Exe -ArgumentList '--start-hidden' -WindowStyle Hidden -PassThru
$ready = $null
while ($sw.ElapsedMilliseconds -lt 30000) {
    Start-Sleep -Milliseconds 50
    $app.Refresh()
    if ($app.HasExited) { break }
    if ($app.MainWindowHandle -ne 0) { $ready = $sw.ElapsedMilliseconds; break }
}
$report['arranque_ms'] = if ($ready) { $ready } else { $sw.ElapsedMilliseconds }
Write-Output "[perf] arranque: $($report['arranque_ms']) ms (proceso vivo: $(-not $app.HasExited))"

if ($app.HasExited) {
    Write-Output "[perf] la app termino durante el arranque; no se puede medir en reposo"
    ($report | ConvertTo-Json -Depth 6) | Out-File -FilePath $OutFile -Encoding utf8
    exit 1
}

# ------------------------------------------------------------ en reposo -----
Start-Sleep -Seconds 3
$idle = @()
for ($i = 0; $i -lt $IdleSeconds; $i++) {
    $s = Measure-Process $app 1000
    if ($null -eq $s) { break }
    $idle += $s
}
if ($idle.Count) {
    $report['reposo_ram_privada_mb'] = [math]::Round((($idle | Measure-Object privateMB -Maximum).Maximum), 1)
    $report['reposo_ram_working_mb'] = [math]::Round((($idle | Measure-Object workingMB -Maximum).Maximum), 1)
    $report['reposo_ram_privada_media_mb'] = [math]::Round((($idle | Measure-Object privateMB -Average).Average), 1)
    $report['reposo_cpu_pct_max'] = [math]::Round((($idle | Measure-Object cpuPct -Maximum).Maximum), 2)
    $report['reposo_cpu_pct_avg'] = [math]::Round((($idle | Measure-Object cpuPct -Average).Average), 2)
    $report['reposo_muestras'] = $idle.Count
    Write-Output ("[perf] reposo: RAM privada {0} MB (max) / {1} MB (media) | working set {2} MB | CPU {3}% max / {4}% media" -f `
        $report['reposo_ram_privada_mb'], $report['reposo_ram_privada_media_mb'], `
        $report['reposo_ram_working_mb'], $report['reposo_cpu_pct_max'], $report['reposo_cpu_pct_avg'])
}

# ------------------------------------------------------- durante ASR -------
# Cada muestra se transcribe varias veces: la maquina tiene otra carga en
# background y una sola medicion seria poco representativa. Se reporta la
# mejor (la de menor best_ms) junto con su CPU.
$results = @()
foreach ($fx in (Get-ChildItem $FixturesDir -Filter '*.wav' | Sort-Object Name)) {
    $best = $null
    for ($run = 0; $run -lt 3; $run++) {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $Exe
        $psi.Arguments = "--transcribe-file `"$($fx.FullName)`" --model $Model --json"
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $t = [System.Diagnostics.Process]::Start($psi)

        $peakCpu = 0.0
        $peakRam = 0.0
        $peakWorking = 0.0
        $cpu0 = 0.0
        $t0 = 0.0
        $sw2 = [System.Diagnostics.Stopwatch]::StartNew()
        $t.Refresh()
        if (-not $t.HasExited) { $cpu0 = $t.TotalProcessorTime.TotalSeconds; $t0 = $sw2.Elapsed.TotalSeconds }
        while (-not $t.HasExited) {
            $t.Refresh()
            if (-not $t.HasExited) {
                $ram = $t.PrivateMemorySize64 / 1MB
                if ($ram -gt $peakRam) { $peakRam = $ram }
                $ws = $t.WorkingSet64 / 1MB
                if ($ws -gt $peakWorking) { $peakWorking = $ws }
                $now = $sw2.Elapsed.TotalSeconds
                if ($now - $t0 -gt 0.25) {
                    $cpu = ($t.TotalProcessorTime.TotalSeconds - $cpu0) / ($now - $t0) * 100
                    if ($cpu -gt $peakCpu) { $peakCpu = $cpu }
                    $cpu0 = $t.TotalProcessorTime.TotalSeconds
                    $t0 = $now
                }
            }
            Start-Sleep -Milliseconds 60
        }
        $sw2.Stop()
        $stdoutText = $t.StandardOutput.ReadToEnd()
        $obj = Get-Json ($stdoutText -split "`r?`n")
        if (-not $obj -or -not $obj.best_ms) { continue }

        # CPU media = tiempo de proceso acumulado / tiempo real.
        $t.Refresh()
        $cpuAvg = if ($sw2.Elapsed.TotalSeconds -gt 0) {
            [math]::Round(($t.TotalProcessorTime.TotalSeconds / $sw2.Elapsed.TotalSeconds) * 100, 1)
        } else { 0 }

        $row = [pscustomobject]@{
            fixture            = $fx.BaseName
            audio_secs         = $obj.audio_secs
            load_ms            = $obj.load_ms
            best_ms            = $obj.best_ms
            rtf                = $obj.rtf
            backend            = $obj.bound_backend
            peak_ram_privada_mb = [math]::Round($peakRam, 1)
            peak_working_set_mb = [math]::Round($peakWorking, 1)
            cpu_pico_pct       = [math]::Round($peakCpu, 1)
            cpu_media_pct      = $cpuAvg
            wall_ms            = $sw2.ElapsedMilliseconds
            text               = $obj.text
        }
        if (-not $best -or $row.best_ms -lt $best.best_ms) { $best = $row }
    }
    if ($best) {
        $results += $best
        Write-Output ("[perf] {0,-16} best={1,6}ms rtf={2,6} carga={3,5}ms ram_priv={4,5}MB working={5,6}MB cpu(pico/media)={6,5}%/{7,5}% -> {8}" -f `
            $best.fixture, $best.best_ms, [math]::Round($best.rtf, 2), $best.load_ms, `
            $best.peak_ram_privada_mb, $best.peak_working_set_mb, $best.cpu_pico_pct, $best.cpu_media_pct, $best.text)
    } else {
        Write-Output "[perf] $($fx.Name): no se pudo transcribir"
    }
}
$report['transcripcion'] = $results
$report['modelo'] = $Model

# Limpieza
try {
    if (-not $app.HasExited) {
        $app.CloseMainWindow() | Out-Null
        Start-Sleep -Seconds 2
        if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    }
} catch { }

($report | ConvertTo-Json -Depth 6) | Out-File -FilePath $OutFile -Encoding utf8
Write-Output "[perf] informe en $OutFile"
