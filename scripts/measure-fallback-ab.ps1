<#
  measure-fallback-ab.ps1 - comparacion INTERCALADA del fallback.

  QUE MIDE
  --------
  Para cada audio y cada repeticion, se ejecutan las dos variantes una
  detrás de otra, con el **mismo binario** y con la **misma carga** en las
  dos:

    - "con":  el fallback se puede disparar (los valores por defecto).
    - "sin":  el fallback no se puede disparar nunca (el limite de retraso
               se pone fuera de alcance).

  Alternar dentro de la misma repeticion es lo que hace la comparacion
  util: las dos variantes se miden en la misma ventana de tiempo, con la
  misma carga de fondo, asi que la diferencia entre ellas no la mete el
  estado de la maquina.

  Se mide lo mismo que en el resto del banco: la espera tras soltar, la ruta
  que se tomo, el retraso pendiente, la CPU y la RAM del arbol de procesos,
  las muestras procesadas y la integridad del texto.

  SIN CONSOLAS
  ------------
  El binario se lanza con CreateNoWindow y la salida se captura en memoria.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-fallback-ab.ps1 `
        -Exe <dixi.exe> -Model <id> -OutDir <dir> -Label ab -LoadCores 4
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$FixturesDir = "$env:TEMP\dixi-eval-fixtures",
    [string]$OutDir = "",
    [string]$InstanceDir = "",
    [string]$Label = 'ab',
    [int]$Repeat = 3,
    [int]$LoadCores = 0,
    [int]$LoadSeconds = 0,
    [string]$Language = 'es',
    [switch]$RefreshBinary,
    [switch]$KeepText,
    [string]$ModelsDir = "$env:APPDATA\com.dixi.app\models",
    [string[]]$ExtraModelsDir = @()
)

$ErrorActionPreference = 'Stop'

if (-not $InstanceDir) {
    $InstanceDir = Join-Path $env:TEMP ("dixi-ab-{0}-{1}" -f $Label, (Get-Date -Format 'HHmmss'))
}
if (-not $OutDir) {
    $OutDir = Join-Path $env:TEMP ("dixi-ab-out-{0}-{1}" -f $Label, (Get-Date -Format 'HHmmss'))
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not (Test-Path $FixturesDir)) {
    Write-Error "No se encuentra el corpus de audio: $FixturesDir"
}

# ---------------------------------------------------------------------------
# Instancia aislada
# ---------------------------------------------------------------------------
$instanceBinary = Join-Path $InstanceDir 'dixi.exe'
if ($RefreshBinary -and (Test-Path $instanceBinary)) {
    $wanted = (Get-FileHash $Exe -Algorithm SHA256).Hash
    $have = (Get-FileHash $instanceBinary -Algorithm SHA256).Hash
    if ($wanted -ne $have) {
        Write-Host "[setup] refrescando el binario de la instancia" -ForegroundColor Yellow
        Copy-Item $Exe $instanceBinary -Force
    }
}
if (-not (Test-Path $instanceBinary)) {
    & (Join-Path $PSScriptRoot 'new-bench-instance.ps1') -InstanceDir $InstanceDir -Exe $Exe -Model $Model -Language $Language -ModelsDir $ModelsDir -ExtraModelsDir $ExtraModelsDir
} else {
    Write-Host "[setup] instancia existente: $InstanceDir" -ForegroundColor Cyan
}

# ---------------------------------------------------------------------------
# Utilidades
# ---------------------------------------------------------------------------
function Get-Percentile {
    param([double[]]$Values, [double]$Percentile)
    if (-not $Values -or $Values.Count -eq 0) { return 0 }
    $sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 1) { return $sorted[0] }
    $rank = [math]::Ceiling($Percentile / 100 * $sorted.Count) - 1
    if ($rank -lt 0) { $rank = 0 }
    if ($rank -ge $sorted.Count) { $rank = $sorted.Count - 1 }
    return $sorted[$rank]
}

function Get-InstanceProcesses {
    $prefix = $InstanceDir
    $procs = @()
    foreach ($p in @(Get-Process -Name 'dixi' -ErrorAction SilentlyContinue)) {
        $path = $null
        try { $path = $p.Path } catch { $path = $null }
        if ($path -and $path.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            $procs += $p
        }
    }
    return $procs
}

function Get-TreeSnapshot {
    $procs = Get-InstanceProcesses
    $cpu = 0.0
    $private = 0L
    $working = 0L
    foreach ($p in $procs) {
        try {
            $cpu += $p.TotalProcessorTime.TotalSeconds
            $private += $p.PrivateMemorySize64
            $working += $p.WorkingSet64
        } catch { }
    }
    return [pscustomobject]@{
        CpuSeconds = $cpu
        PrivateMB  = [math]::Round($private / 1MB, 1)
        WorkingMB  = [math]::Round($working / 1MB, 1)
        Processes  = $procs.Count
    }
}

function Measure-WhileRunning {
    param([System.Diagnostics.Process]$Process)
    $samples = @()
    $first = Get-TreeSnapshot
    $t0 = Get-Date
    while (-not $Process.HasExited) {
        Start-Sleep -Milliseconds 250
        $samples += Get-TreeSnapshot
    }
    $Process.WaitForExit()
    $elapsed = ((Get-Date) - $t0).TotalSeconds
    if ($elapsed -le 0) { $elapsed = 0.001 }
    $last = $first
    foreach ($s in $samples) {
        if ($s.CpuSeconds -gt $last.CpuSeconds) { $last = $s }
    }
    $maxPrivate = 0.0
    $maxWorking = 0.0
    foreach ($s in $samples) {
        if ($s.PrivateMB -gt $maxPrivate) { $maxPrivate = $s.PrivateMB }
        if ($s.WorkingMB -gt $maxWorking) { $maxWorking = $s.WorkingMB }
    }
    return [pscustomobject]@{
        CpuPct       = [math]::Round((($last.CpuSeconds - $first.CpuSeconds) / $elapsed) * 100, 1)
        RamPrivateMB = $maxPrivate
        RamWorkingMB = $maxWorking
    }
}

function Invoke-Live {
    param([string]$Fixture, [string]$ModelId)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $instanceBinary
    $psi.Arguments = "--live-file `"$Fixture`" --model $ModelId --repeat 1 --json"
    $psi.WorkingDirectory = $InstanceDir
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $process = [System.Diagnostics.Process]::Start($psi)
    $errTask = $process.StandardError.ReadToEndAsync()
    $outTask = $process.StandardOutput.ReadToEndAsync()
    $usage = Measure-WhileRunning -Process $process
    $stdout = $outTask.GetAwaiter().GetResult()
    $stderr = $errTask.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0) {
        throw "La transcripcion en vivo de $Fixture fallo (codigo $($process.ExitCode)): $stderr"
    }
    foreach ($line in ($stdout -split "`r?`n")) {
        $t = $line.Trim()
        if ($t.StartsWith('{')) {
            $data = $null
            try { $data = $t | ConvertFrom-Json } catch { }
            if ($null -eq $data) { continue }
            if ($ModelId -and $data.model -ne $ModelId) {
                throw "El banco pidio '$ModelId' pero se midio '$($data.model)'"
            }
            return [pscustomobject]@{ Data = $data; Uso = $usage }
        }
    }
    throw "La transcripcion en vivo de $Fixture no devolvio JSON: $stdout"
}

# ---------------------------------------------------------------------------
# Carga controlada
# ---------------------------------------------------------------------------
$loadProcesses = @()
if ($LoadCores -gt 0) {
    Write-Host "[carga] ocupando $LoadCores nucleo(s) durante $LoadSeconds s" -ForegroundColor Yellow
    for ($i = 0; $i -lt $LoadCores; $i++) {
        $loadProcesses += Start-Process powershell.exe -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
            (Join-Path $PSScriptRoot 'cpu-load.ps1'), '-Nucleos', '1', '-Segundos', "$LoadSeconds"
        ) -WindowStyle Hidden -PassThru
    }
    Start-Sleep -Seconds 2
}

# ---------------------------------------------------------------------------
# Medicion intercalada
# ---------------------------------------------------------------------------
$rows = @()

try {
    foreach ($fixture in (Get-ChildItem $FixturesDir -Filter '*.wav' | Sort-Object Name)) {
        for ($rep = 1; $rep -le $Repeat; $rep++) {
            # El orden se alterna entre repeticiones: si la maquina se va
            # putting peor con el tiempo, la primera variante de cada par no
            # tiene por que pagar siempre el estado peor.
            $modos = if ($rep % 2 -eq 1) { @('con', 'sin') } else { @('sin', 'con') }
            foreach ($mode in $modos) {
                # El fallback se desactiva poniendo el limite de retraso fuera
                # de alcance: el mismo binario, la misma configuracion y el
                # mismo entorno salvo ese valor.
                if ($mode -eq 'sin') {
                    $env:DIXI_STREAM_BACKLOG_LIMIT_MS = '999999999'
                } else {
                    Remove-Item Env:\DIXI_STREAM_BACKLOG_LIMIT_MS -ErrorAction SilentlyContinue
                }
                try {
                    $result = Invoke-Live -Fixture $fixture.FullName -ModelId $Model
                } catch {
                    Write-Warning "No se pudo medir $($fixture.Name) ($mode): $_"
                    continue
                }
                $data = $result.Data
                $usage = $result.Uso
                $run = @($data.runs)[0]
                $rows += [pscustomobject]@{
                    Audio       = $fixture.Name
                    AudioSegs   = [double]$data.audio_secs
                    Corrida     = [int]$rep
                    Modo        = $mode
                    Ruta        = [string]$run.path
                    EsperaMs    = [double]$run.post_release_ms
                    FinalMs     = [double]$run.stages.stages.finalize
                    TotalMs     = [double]$run.stages.total_ms
                    RetrasoMs   = [double]$run.backlog_ms_at_release
                    DecodificadoS = [double]$run.decoded_secs_while_recording
                    Muestras    = [double]$data.audio_secs * 16000
                    CpuPct      = [double]$usage.CpuPct
                    RamPrivMB   = [double]$usage.RamPrivateMB
                    Texto       = $(if ($KeepText) { [string]$data.text } else { '' })
                }
                Write-Host ("  {0} {1} r{2}: espera {3} ms | ruta {4} | retraso {5} ms" -f `
                    $fixture.Name, $mode, $rep, $run.post_release_ms, $run.path, $run.backlog_ms_at_release)
                # Se escribe en cada corrida: si hay que parar la medicion a
                # medias, lo medido hasta ese punto sirve.
                $rows | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'ab-ejecuciones.json') -Encoding UTF8
            }
        }
    }
} finally {
    foreach ($p in $loadProcesses) {
        try { if (-not $p.HasExited) { $p.Kill() } } catch { }
    }
}

if ($rows.Count -eq 0) {
    Write-Error "No se pudo medir ningun audio; revise los logs en .logs"
}

$rows | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'ab-ejecuciones.json') -Encoding UTF8

# Resumen por modo.
$resumen = @()
foreach ($mode in @('con', 'sin')) {
    $group = @($rows | Where-Object { $_.Modo -eq $mode })
    if ($group.Count -eq 0) { continue }
    $w = @($group | ForEach-Object { $_.EsperaMs })
    $resumen += [pscustomobject]@{
        Modo      = $mode
        N         = $group.Count
        P50       = [int](Get-Percentile -Values $w -Percentile 50)
        P95       = [int](Get-Percentile -Values $w -Percentile 95)
        Maximo    = [int](($w | Measure-Object -Maximum).Maximum)
        Minimo    = [int](($w | Measure-Object -Minimum).Minimum)
        Media     = [int](($w | Measure-Object -Average).Average)
        CpuPct    = [math]::Round((($group | ForEach-Object { $_.CpuPct }) | Measure-Object -Average).Average, 1)
        RamPrivMB = [math]::Round((($group | ForEach-Object { $_.RamPrivMB }) | Measure-Object -Average).Average, 1)
    }
}
$resumen | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'ab-resumen.json') -Encoding UTF8
$resumen | Format-Table -AutoSize

# Al terminar no debe quedar ningun proceso del banco vivo.
$left = @(Get-InstanceProcesses)
Write-Host ("[limpieza] procesos del banco vivos al terminar: {0}" -f $left.Count)
$left | ForEach-Object { Write-Host ("  pid {0} {1}" -f $_.Id, $_.Path) }

Write-Host "Informe escrito en $OutDir" -ForegroundColor Green
