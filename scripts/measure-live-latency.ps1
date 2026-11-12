<#
  measure-live-latency.ps1 - mide la latencia de la RUTA LIVE: la que usa la
  app mientras el usuario habla.

  QUE MIDE
  --------
  Por cada audio de referencia:
    - cuanto se decodifico ANTES de soltar la tecla (fragmentos y segundos),
    - cuanto audio quedaba pendiente en el momento de soltar,
    - cuanto tardo el ultimo fragmento,
    - el coste posterior a soltar, etapa por etapa,
    - p50 y p95 con las repeticiones pedidas.

  La ruta por lotes (`--transcribe-file`) mide una pasada sobre un clip ya
  terminado: eso NO es la latencia que percibe el usuario. Aqui el audio se
  alimenta a la velocidad de una grabacion, que es lo que permite que un
  fragmento se decodifique mientras el usuario sigue hablando.

  SIN CONSOLAS
  ------------
  El binario se lanza con CreateNoWindow y la salida se captura en memoria.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-live-latency.ps1 `
        -Exe <dixi.exe> -Model <id> -OutDir <dir> [-Repeat 3] [-Label antes]
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$OutDir = "",
    [string]$FixturesDir = "",
    [int]$Repeat = 3,
    [string]$Label = 'live',
    [string]$InstanceDir = "",
    [string]$Language = "",
    # Alimenta el audio mas rapido que tiempo real: solo para iterar. La
    # latencia que se informa solo vale a 1x.
    [switch]$FastFeed,
    # No guarda el texto transcrito. OBLIGATORIO con audio de voz real: la
    # transcripcion es contenido privado y no debe quedar en un informe.
    [switch]$NoText,
    [string]$ModelsDir = "$env:APPDATA\com.dixi.app\models",
    # Carpetas adicionales de modelos (una descarga aislada, por ejemplo).
    [string[]]$ExtraModelsDir = @()
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $InstanceDir) {
    $InstanceDir = Join-Path $env:TEMP ("dixi-live-{0}-{1}" -f $Label, (Get-Date -Format 'HHmmss'))
}
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot "docs\validation-performance\$Label"
}
if (-not $FixturesDir) {
    $FixturesDir = Join-Path $repoRoot 'docs\validation-phase2a\audio-referencia'
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not (Test-Path $FixturesDir)) {
    Write-Error "No se encuentra el corpus de audio: $FixturesDir"
}

# ---------------------------------------------------------------------------
# Instancia aislada
# ---------------------------------------------------------------------------
Write-Host "[setup] instancia aislada en $InstanceDir" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'new-bench-instance.ps1') -InstanceDir $InstanceDir -Exe $Exe -Model $Model -Language $Language -ModelsDir $ModelsDir -ExtraModelsDir $ExtraModelsDir

$instanceBinary = Join-Path $InstanceDir 'dixi.exe'

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

function Invoke-Live {
    <#
      Lanza el binario sin ventana visible y devuelve el objeto JSON.
      CreateNoWindow evita la consola; nada se escribe en el escritorio.
    #>
    param([string]$Fixture, [string]$ModelId, [int]$Runs, [bool]$Fast)

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $instanceBinary
    $args = "--live-file `"$Fixture`" --model $ModelId --repeat $Runs --json"
    if ($Fast) { $args += " --fast-feed" }
    $psi.Arguments = $args
    $psi.WorkingDirectory = $InstanceDir
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $process = [System.Diagnostics.Process]::Start($psi)
    # El stderr se drena en paralelo: si el pipe se llena y nadie lo lee, la
    # app se queda bloqueada escribiendo y la corrida no termina nunca.
    $errTask = $process.StandardError.ReadToEndAsync()
    $stdout = $process.StandardOutput.ReadToEnd()
    $process.WaitForExit()
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
            return $data
        }
    }
    throw "La transcripcion en vivo de $Fixture no devolvio JSON: $stdout"
}

# ---------------------------------------------------------------------------
# Medicion
# ---------------------------------------------------------------------------
$results = @()

foreach ($fixture in (Get-ChildItem $FixturesDir -Filter '*.wav' | Sort-Object Name)) {
    Write-Host "[medir] $($fixture.Name)" -ForegroundColor Cyan
    try {
        $data = Invoke-Live -Fixture $fixture.FullName -ModelId $Model -Runs $Repeat -Fast ([bool]$FastFeed)
    } catch {
        Write-Warning "No se pudo medir $($fixture.Name): $_"
        continue
    }

    $runs = @($data.runs)
    $post = @($runs | ForEach-Object { [double]$_.post_release_ms })
    $finalize = @($runs | ForEach-Object { [double]$_.stages.stages.finalize })
    $whileDecoding = @($runs | ForEach-Object { [int]$_.chunks_decoded_while_recording })
    $pending = @($runs | ForEach-Object { [double]$_.pending_secs_at_release })
    $lastChunk = @($runs | ForEach-Object { [double]$_.last_chunk_ms })

    $results += [pscustomobject]@{
        Fixture   = $fixture.Name
        AudioSecs = [double]$data.audio_secs
        Model     = $data.model
        Backend   = $data.bound_backend
        LoadMs    = [int]$data.load_ms
        Runs      = $runs.Count
        Path      = ($runs | Select-Object -First 1).path
        PostMsMin = [int](($post | Measure-Object -Minimum).Minimum)
        PostMsP50 = [int](Get-Percentile -Values $post -Percentile 50)
        PostMsP95 = [int](Get-Percentile -Values $post -Percentile 95)
        PostMsMax = [int](($post | Measure-Object -Maximum).Maximum)
        FinalMsMin = [int](($finalize | Measure-Object -Minimum).Minimum)
        FinalMsP50 = [int](Get-Percentile -Values $finalize -Percentile 50)
        ChunksWhile = [int](($whileDecoding | Measure-Object -Maximum).Maximum)
        PendingSecsMin = [math]::Round((($pending | Measure-Object -Minimum).Minimum), 2)
        LastChunkMsMin = [int](($lastChunk | Measure-Object -Minimum).Minimum)
        Text      = $(if ($NoText) { '' } else { $data.text })
        Detalle   = $runs
    }
}

if ($results.Count -eq 0) {
    Write-Error "No se pudo medir ningun audio; revise los logs en .logs"
}

# ---------------------------------------------------------------------------
# Informe
# ---------------------------------------------------------------------------
function Stat {
    param([string]$Name, [scriptblock]$Selector)
    $values = @($results | ForEach-Object { [double](& $Selector $_) })
    [pscustomobject]@{
        Metrica = $Name
        Media   = [math]::Round(($values | Measure-Object -Average).Average, 1)
        P50     = [math]::Round((Get-Percentile -Values $values -Percentile 50), 1)
        P95     = [math]::Round((Get-Percentile -Values $values -Percentile 95), 1)
        Max     = [math]::Round(($values | Measure-Object -Maximum).Maximum, 1)
    }
}

# Todas las ejecuciones sueltas, una fila por repeticion. Los percentiles
# "globales" se calculan sobre ESTA lista (7 grabaciones x N repeticiones),
# no sobre las siete filas por grabacion: un percentil sobre siete minimos no
# es el percentil de las ejecuciones.
$ejecuciones = @()
foreach ($r in $results) {
    foreach ($d in $r.Detalle) {
        $ejecuciones += [pscustomobject]@{
            Fixture     = $r.Fixture
            AudioSecs   = $r.AudioSecs
            Corrida     = $d.run
            PostMs      = [double]$d.post_release_ms
            FinalMs     = [double]$d.stages.stages.finalize
            TotalMs     = [double]$d.stages.total_ms
            Camino      = $d.path
            PendienteS = [double]$d.pending_secs_at_release
        }
    }
}

# El corte entre "dictado corto" y "dictado prolongado" esta medido, no
# supuesto: el cambio de regimen del streaming ocurre entre 14,40 s y
# 17,82 s de audio. El corte se pone en 16 s, en medio de ese intervalo.
$CORTE_SEGUNDOS = 16.0

function StatDe {
    param([string]$Nombre, [object[]]$Valores)
    if (-not $Valores -or $Valores.Count -eq 0) { return $null }
    [pscustomobject]@{
        Metrica  = $Nombre
        N        = $Valores.Count
        Media    = [math]::Round(($Valores | Measure-Object -Average).Average, 1)
        Min      = [math]::Round(($Valores | Measure-Object -Minimum).Minimum, 1)
        P50      = [math]::Round((Get-Percentile -Values $Valores -Percentile 50), 1)
        P95      = [math]::Round((Get-Percentile -Values $Valores -Percentile 95), 1)
        Max      = [math]::Round(($Valores | Measure-Object -Maximum).Maximum, 1)
    }
}

# 1) POR GRABACION: cada fila es una grabacion, con la distribucion de SUS
#    repeticiones.
$porGrabacion = @()
foreach ($r in $results) {
    $vals = @($r.Detalle | ForEach-Object { [double]$_.post_release_ms })
    $s = StatDe -Nombre ("grabacion " + $r.Fixture) -Valores $vals
    $porGrabacion += [pscustomobject]@{
        Grabacion  = $r.Fixture
        AudioSegs  = $r.AudioSecs
        N          = $vals.Count
        Min        = $s.Min
        P50        = $s.P50
        P95        = $s.P95
        Max        = $s.Max
    }
}

# 2) AGRUPADA: todas las ejecuciones del motor.
$agrupada = @(
    (StatDe -Nombre 'AGRUPADO - espera tras soltar (ms)' -Valores @($ejecuciones | ForEach-Object { $_.PostMs })),
    (StatDe -Nombre 'AGRUPADO - decodificacion final (ms)' -Valores @($ejecuciones | ForEach-Object { $_.FinalMs }))
)

# 3) POR SUBCONJUNTO: cortas frente a largas.
$cortas = @($ejecuciones | Where-Object { $_.AudioSecs -le $CORTE_SEGUNDOS })
$largas = @($ejecuciones | Where-Object { $_.AudioSecs -gt $CORTE_SEGUNDOS })
$porSubconjunto = @(
    (StatDe -Nombre ("SUBCONJUNTO <= " + $CORTE_SEGUNDOS + " s - espera tras soltar (ms)") -Valores @($cortas | ForEach-Object { $_.PostMs })),
    (StatDe -Nombre ("SUBCONJUNTO > " + $CORTE_SEGUNDOS + " s - espera tras soltar (ms)") -Valores @($largas | ForEach-Object { $_.PostMs }))
)

# El resumen por grabacion que se hacia antes (minimo por grabacion) se
# conserva, pero con otro nombre: es un statistic POR GRABACION, no global.
$stats = @(
    (StatDe -Nombre 'POR GRABACION - minimo de la espera tras soltar (ms)' -Valores @($results | ForEach-Object { [double]$_.PostMsMin })),
    (StatDe -Nombre 'POR GRABACION - p50 de la espera tras soltar (ms)' -Valores @($results | ForEach-Object { [double]$_.PostMsP50 })),
    (StatDe -Nombre 'POR GRABACION - p95 de la espera tras soltar (ms)' -Valores @($results | ForEach-Object { [double]$_.PostMsP95 }))
)

$todasEstadisticas = @($agrupada + $porSubconjunto + $stats | Where-Object { $null -ne $_ })

$todasEstadisticas | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'estadisticas-live.json') -Encoding UTF8
# Una fila por ejecucion: la base desde la que se recalcula cualquier
# percentil, sin volver a medir.
$ejecuciones | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'ejecuciones-live.json') -Encoding UTF8
$porGrabacion | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'por-grabacion-live.json') -Encoding UTF8
$results | Select-Object -ExcludeProperty Detalle | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'resultados-live.json') -Encoding UTF8
# El detalle por corrida se guarda aparte: es el unico sitio donde queda el
# fragmento a fragmento.
$results | Select-Object Fixture, Detalle | ConvertTo-Json -Depth 8 | Set-Content -Path (Join-Path $OutDir 'detalle-live.json') -Encoding UTF8

$rows = foreach ($s in $todasEstadisticas) { "| $($s.Metrica) | $($s.N) | $($s.Media) | $($s.Min) | $($s.P50) | $($s.P95) | $($s.Max) |" }
$detail = foreach ($r in $results) {
    "| $($r.Fixture) | $($r.AudioSecs) | $($r.Path) | $($r.ChunksWhile) | $($r.PendingSecsMin) | $($r.FinalMsMin) | $($r.PostMsMin) | $($r.LastChunkMsMin) |"
}

@"
# Latencia de la ruta en vivo - $Label

Banco sobre la ruta que usa la app mientras el usuario habla: el audio se
alimenta a la velocidad de una grabacion y se mide lo que queda pendiente en
el momento de soltar la tecla.

- Binario: ``$Exe``
- Modelo: ``$Model``
- Instancia aislada: ``$InstanceDir``
- Repeticiones por audio: $Repeat
- Idioma forzado: $(if ($Language) { $Language } else { 'auto (el de la app)' })

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
$($rows -join "`n")

## Detalle por audio

| Audio | Audio (s) | Ruta | Fragmentos mientras grababa | Pendiente al soltar (s) | Final (ms) | Tras soltar (ms) | Ultimo fragmento (ms) |
|---|---|---|---|---|---|---|---|
$($detail -join "`n")
"@ | Set-Content -Path (Join-Path $OutDir 'informe-live.md') -Encoding UTF8

Write-Host ''
Write-Host "Informe escrito en $(Join-Path $OutDir 'informe-live.md')" -ForegroundColor Green
$stats | Format-Table -AutoSize
