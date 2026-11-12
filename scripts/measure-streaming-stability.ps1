<#
  measure-streaming-stability.ps1 - mide la ruta en vivo con carga controlada.

  QUE MIDE
  --------
  Por cada audio de referencia y cada repeticion:
    - la espera tras soltar (lo que el usuario percibe),
    - la ruta que se tomo ("live" o "batch": el batch es la recuperacion por
      lotes cuando la maquina no da mas),
    - el audio que quedaba pendiente en el momento de soltar,
    - la CPU y la RAM del ARBOL de procesos de la instancia durante la corrida.

  El texto transcrito se puede guardar para comprobar que la recuperacion
  por lotes no pierde segmentos ni duplica palabras. Con audio de voz real
  el texto es contenido privado: por defecto NO se guarda (-KeepText lo pide
  explicitamente).

  CARGA
  -----
  Con -LoadCores se lanzan procesos en segundo plano que ocupan nucleos, sin
  ventana visible, durante -LoadSeconds. Sirve para reproducir la maquina
  ocupada.

  SIN CONSOLAS
  ------------
  El binario se lanza con CreateNoWindow y la salida se captura en memoria.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-streaming-stability.ps1 `
        -Exe <dixi.exe> -Model <id> -OutDir <dir> -Label antes -LoadCores 4
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$FixturesDir = "$env:TEMP\dixi-eval-fixtures",
    [string]$OutDir = "",
    [string]$InstanceDir = "",
    [string]$Label = 'estabilidad',
    [int]$Repeat = 5,
    # Nucleos a ocupar con carga durante la corrida. 0 = sin carga.
    [int]$LoadCores = 0,
    [int]$LoadSeconds = 0,
    [string]$Language = 'es',
    # Cancela la transcripcion N ms despues de soltar, como hace el atajo de
    # cancelar. 0 = sin cancelacion.
    [int]$CancelAfterMs = 0,
    # Limite de retraso pendiente que usa el banco (ms). 0 = el que trae la
    # app. Un limite enorme desactiva la pasada completa, para comparar.
    [int]$BacklogLimitMs = 0,
    # Fraccion de la grabacion que tiene que estar pendiente para que la
    # pasada completa compense. 0 = la que trae la app.
    [double]$BacklogMinFraction = 0,
    [switch]$KeepText,
    [string]$ModelsDir = "$env:APPDATA\com.dixi.app\models",
    [string[]]$ExtraModelsDir = @()
)

$ErrorActionPreference = 'Stop'

# El banco hereda el entorno: con esto se puede cambiar el limite de retraso
# sin recompilar (y comparar la pasada completa contra el cierre del motor).
if ($BacklogLimitMs -gt 0) {
    $env:DIXI_STREAM_BACKLOG_LIMIT_MS = "$BacklogLimitMs"
    Write-Host "[banco] limite de retraso: $BacklogLimitMs ms" -ForegroundColor Yellow
}
if ($BacklogMinFraction -gt 0) {
    $env:DIXI_STREAM_BACKLOG_MIN_FRACTION = "$BacklogMinFraction"
    Write-Host "[banco] fraccion minima de retraso: $BacklogMinFraction" -ForegroundColor Yellow
}

if (-not $InstanceDir) {
    $InstanceDir = Join-Path $env:TEMP ("dixi-estab-{0}-{1}" -f $Label, (Get-Date -Format 'HHmmss'))
}
if (-not $OutDir) {
    $OutDir = Join-Path $env:TEMP ("dixi-estab-out-{0}-{1}" -f $Label, (Get-Date -Format 'HHmmss'))
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not (Test-Path $FixturesDir)) {
    Write-Error "No se encuentra el corpus de audio: $FixturesDir"
}

# ---------------------------------------------------------------------------
# Instancia aislada. Si el ejecutable ya esta en la instancia se respeta: la
# instancia es la que define el binario medido (asi se mide el "antes" con su
# binario y el "despues" con el nuevo).
# ---------------------------------------------------------------------------
$instanceBinary = Join-Path $InstanceDir 'dixi.exe'
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

# Los procesos de ESTA instancia: los dixi.exe cuyo ejecutable esta dentro de
# la carpeta de la instancia. La app real del usuario esta en otra carpeta y
# no se toca ni se cuenta.
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

# Muestrea el arbol mientras dura la corrida: el pico de RAM y el consumo de
# CPU medios del mismo intervalo.
function Measure-WhileRunning {
    param([System.Diagnostics.Process]$Process)
    $samples = @()
    $first = Get-TreeSnapshot
    $t0 = Get-Date
    while (-not $Process.HasExited) {
        Start-Sleep -Milliseconds 250
        $snap = Get-TreeSnapshot
        $samples += $snap
    }
    $Process.WaitForExit()
    $elapsed = ((Get-Date) - $t0).TotalSeconds
    if ($elapsed -le 0) { $elapsed = 0.001 }
    # The cumulative CPU of the tree only grows while the processes live: the
    # last sample can be taken after the bench exited, when the tree is
    # already empty. The largest sample is the last one that saw it alive.
    $last = $first
    foreach ($s in $samples) {
        if ($s.CpuSeconds -gt $last.CpuSeconds) { $last = $s }
    }
    $cpuPct = [math]::Round((($last.CpuSeconds - $first.CpuSeconds) / $elapsed) * 100, 1)
    $maxPrivate = 0.0
    $maxWorking = 0.0
    foreach ($s in $samples) {
        if ($s.PrivateMB -gt $maxPrivate) { $maxPrivate = $s.PrivateMB }
        if ($s.WorkingMB -gt $maxWorking) { $maxWorking = $s.WorkingMB }
    }
    return [pscustomobject]@{
        CpuPct        = $cpuPct
        RamPrivateMB  = $maxPrivate
        RamWorkingMB  = $maxWorking
        PeakProcesses = if ($samples.Count -gt 0) { ($samples | Measure-Object -Property Processes -Maximum).Maximum } else { 0 }
        Segundos      = [math]::Round($elapsed, 2)
    }
}

function Invoke-Live {
    <#
      Lanza el banco en vivo sin ventana visible y devuelve el objeto JSON.
      CreateNoWindow evita la consola; nada se escribe en el escritorio.
    #>
    param([string]$Fixture, [string]$ModelId, [int]$Runs)

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $instanceBinary
    $psi.Arguments = "--live-file `"$Fixture`" --model $ModelId --repeat $Runs --json"
    if ($CancelAfterMs -gt 0) { $psi.Arguments += " --cancel-after-ms $CancelAfterMs" }
    $psi.WorkingDirectory = $InstanceDir
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $process = [System.Diagnostics.Process]::Start($psi)
    # stdout y stderr se drena en paralelo: si el pipe se llena y nadie lo
    # lee, la app se queda bloqueada escribiendo y la corrida no termina
    # nunca (el JSON de una corrida puede pasar de los 4 KB del pipe).
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
            # El modelo medido tiene que ser el pedido: un banco que mide otra
            # cosa y lo anota en la columna equivocada no sirve.
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
    # Un momento para que la carga se asiente antes de medir.
    Start-Sleep -Seconds 2
}

# ---------------------------------------------------------------------------
# Medicion
# ---------------------------------------------------------------------------
$rows = @()

try {
    foreach ($fixture in (Get-ChildItem $FixturesDir -Filter '*.wav' | Sort-Object Name)) {
        Write-Host "[medir] $($fixture.Name)" -ForegroundColor Cyan
        try {
            $result = Invoke-Live -Fixture $fixture.FullName -ModelId $Model -Runs $Repeat
        } catch {
            Write-Warning "No se pudo medir $($fixture.Name): $_"
            continue
        }
        $data = $result.Data
        $usage = $result.Uso
        $text = [string]$data.text
        $runs = @($data.runs)
        for ($i = 0; $i -lt $runs.Count; $i++) {
            $d = $runs[$i]
            $rows += [pscustomobject]@{
                Audio       = $fixture.Name
                AudioSegs   = [double]$data.audio_secs
                Corrida     = [int]$d.run
                Ruta        = [string]$d.path
                EsperaMs    = [double]$d.post_release_ms
                FinalMs     = [double]$d.stages.stages.finalize
                TotalMs     = [double]$d.stages.total_ms
                PendienteS  = [double]$d.pending_secs_at_release
                RetrasoMs   = [double]$d.backlog_ms_at_release
                CpuPct      = [double]$usage.CpuPct
                RamPrivMB   = [double]$usage.RamPrivateMB
                RamWorkMB   = [double]$usage.RamWorkingMB
                Procesos    = [int]$usage.PeakProcesses
                Cancelado  = [bool]$d.cancelled
                CancelMs   = [int]$d.cancel_after_ms
                Texto       = $(if ($KeepText) { $text } else { '' })
            }
        }
        Write-Host ("         espera p50 {0} ms | ruta {1} | CPU {2} % | RAM {3} MB" -f `
            (Get-Percentile -Values @($runs | ForEach-Object { [double]$_.post_release_ms }) -Percentile 50), `
            (($runs | ForEach-Object { $_.path }) -join ','), `
            $usage.CpuPct, $usage.RamPrivateMB)
    }
} finally {
    foreach ($p in $loadProcesses) {
        try { if (-not $p.HasExited) { $p.Kill() } } catch { }
    }
}

if ($rows.Count -eq 0) {
    Write-Error "No se pudo medir ningun audio; revise los logs en .logs"
}

# ---------------------------------------------------------------------------
# Informe
# ---------------------------------------------------------------------------
function StatDe {
    param([string]$Nombre, [object[]]$Valores)
    if (-not $Valores -or $Valores.Count -eq 0) { return $null }
    [pscustomobject]@{
        Metrica = $Nombre
        N       = $Valores.Count
        Media   = [math]::Round(($Valores | Measure-Object -Average).Average, 1)
        Min     = [math]::Round(($Valores | Measure-Object -Minimum).Minimum, 1)
        P50     = [math]::Round((Get-Percentile -Values $Valores -Percentile 50), 1)
        P95     = [math]::Round((Get-Percentile -Values $Valores -Percentile 95), 1)
        Max     = [math]::Round(($Valores | Measure-Object -Maximum).Maximum, 1)
    }
}

$live = @($rows | Where-Object { $_.Ruta -eq 'live' })
$batch = @($rows | Where-Object { $_.Ruta -eq 'batch' })

$estadisticas = @(
    (StatDe -Nombre 'TODAS - espera tras soltar (ms)' -Valores @($rows | ForEach-Object { $_.EsperaMs })),
    (StatDe -Nombre 'RUTA LIVE - espera tras soltar (ms)' -Valores @($live | ForEach-Object { $_.EsperaMs })),
    (StatDe -Nombre 'RUTA BATCH - espera tras soltar (ms)' -Valores @($batch | ForEach-Object { $_.EsperaMs })),
    (StatDe -Nombre 'TODAS - CPU del arbol (%)' -Valores @($rows | ForEach-Object { $_.CpuPct })),
    (StatDe -Nombre 'TODAS - RAM privada del arbol (MB)' -Valores @($rows | ForEach-Object { $_.RamPrivMB })),
    (StatDe -Nombre 'TODAS - retraso pendiente al soltar (ms)' -Valores @($rows | ForEach-Object { $_.RetrasoMs }))
) | Where-Object { $null -ne $_ }

$estadisticas | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'estadisticas-estabilidad.json') -Encoding UTF8
# Una fila por ejecucion: la base desde la que se recalcula cualquier
# percentil, sin volver a medir.
$rows | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'ejecuciones-estabilidad.json') -Encoding UTF8

$tabla = foreach ($s in $estadisticas) { "| $($s.Metrica) | $($s.N) | $($s.Media) | $($s.Min) | $($s.P50) | $($s.P95) | $($s.Max) |" }
$detalle = foreach ($r in $rows) {
    "| $($r.Audio) | $($r.Corrida) | $($r.Ruta) | $($r.EsperaMs) | $($r.FinalMs) | $($r.RetrasoMs) | $($r.CpuPct) | $($r.RamPrivMB) | $($r.RamWorkMB) |"
}

@"
# Estabilidad del streaming - $Label

Banco sobre la ruta que usa la app mientras el usuario habla, con la maquina
en el estado de carga indicado.

- Binario: ``$instanceBinary``
- Modelo: ``$Model``
- Instancia aislada: ``$InstanceDir``
- Corpus: ``$FixturesDir``
- Repeticiones por audio: $Repeat
- Nucleos ocupados por la carga: $LoadCores
- Texto transcrito conservado: $(if ($KeepText) { 'si' } else { 'no' })

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
$($tabla -join "`n")

## Detalle por ejecucion

| Audio | Corrida | Ruta | Espera tras soltar (ms) | Final (ms) | Retraso pendiente (ms) | CPU (%) | RAM privada (MB) | RAM working (MB) |
|---|---|---|---|---|---|---|---|---|
$($detalle -join "`n")
"@ | Set-Content -Path (Join-Path $OutDir 'informe-estabilidad.md') -Encoding UTF8

Write-Host ''
Write-Host "Informe escrito en $(Join-Path $OutDir 'informe-estabilidad.md')" -ForegroundColor Green
$estadisticas | Format-Table -AutoSize
