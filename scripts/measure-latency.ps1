<#
  measure-latency.ps1 - mide la latencia del motor ASR con un banco
  reproducible, y compara dos binarios (antes y despues de una
  optimizacion).

  QUE MIDE
  --------
  Por cada audio de referencia:
    - la carga en frio del modelo (load_ms),
    - la mejor inferencia del corpus (best_ms),
    - el factor de tiempo real (rtf),
    - el desglose por etapa que emite la traza de latencia
      (dispatch, audio, model_load, asr, rules, post, history, insert).

  Para el corpus completo se informa la mediana (p50), el p95 y el maximo de
  cada magnitud, que es lo que permite comparar dos versiones sin que una
  sola ejecucion decida el resultado.

  AISLAMIENTO
  ----------
  El banco corre sobre una instancia **aislada** (carpeta portable propia con
  los modelos compartidos por junction), igual que measure-phase2b.ps1: la
  instalacion real del usuario no se toca y los cientos de megabytes de los
  modelos no se duplican.

  SIN CONSOLAS
  ------------
  El binario se lanza con CreateNoWindow y la salida se captura en memoria:
  no se abre ninguna ventana.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-latency.ps1 `
        -Exe <dixi.exe> -Model <id> -OutDir <dir> [-Repeat 3] [-Label antes]
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$OutDir = "",
    [string]$FixturesDir = "",
    [int]$Repeat = 3,
    [string]$Label = 'medicion',
    [string]$InstanceDir = "",
    # Idioma de reconocimiento forzado. Vacio = el valor por defecto de la app
    # ('auto'), que deja que el modelo detecte el idioma. Un codigo ('es',
    # 'es-US') lo fija, que es lo que hay que comparar.
    [string]$Language = "",
    # No guarda el texto transcrito. OBLIGATORIO con audio de voz real: la
    # transcripcion es contenido privado y no debe quedar en un informe.
    [switch]$NoText,
    [string]$ModelsDir = "$env:APPDATA\com.dixi.app\models"
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $InstanceDir) {
    # Cada corrida usa su propia carpeta: no se borra nada de disco.
    $InstanceDir = Join-Path $env:TEMP ("dixi-latency-{0}-{1}" -f $Label, (Get-Date -Format 'HHmmss'))
}

if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot "docs\validation-performance\$Label"
}
if (-not $FixturesDir) {
    $FixturesDir = Join-Path $repoRoot 'docs\validation-phase2a\audio-referencia'
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if (-not (Test-Path $Exe)) {
    Write-Error "No se encuentra el binario: $Exe"
}
if (-not (Test-Path $FixturesDir)) {
    Write-Error "No se encuentra el corpus de audio: $FixturesDir"
}

# El modelo pedido, guardado antes de que ningun bucle lo pueda pisar.
$selectedModel = $Model

# ---------------------------------------------------------------------------
# Instancia aislada
# ---------------------------------------------------------------------------
Write-Host "[setup] instancia aislada en $InstanceDir" -ForegroundColor Cyan

# Cada corrida usa su propia carpeta: no se borra nada de disco.
New-Item -ItemType Directory -Force -Path $InstanceDir | Out-Null

Copy-Item $Exe (Join-Path $InstanceDir 'dixi.exe') -Force
Set-Content -Path (Join-Path $InstanceDir 'portable') -Value 'Dixi Portable Mode' -Encoding UTF8
New-Item -ItemType Directory -Force -Path (Join-Path $InstanceDir 'Data') | Out-Null

# Los recursos (iconos de la bandeja, sonidos) se resuelven con respecto al
# ejecutable cuando la app no esta empaquetada.
$resourcesSource = Join-Path $repoRoot 'src-tauri\resources'
if (Test-Path $resourcesSource) {
    Copy-Item $resourcesSource (Join-Path $InstanceDir 'resources') -Recurse -Force
}

# Los modelos se comparten por ENLACES DUROS, archivo por archivo: no se
# duplican los cientos de megabytes y la instalacion real no se toca.
#
# Un junction de carpeta NO sirve aqui: la app puede borrar un `.partial`
# residual en `models_dir`, y con un junction ese borrado cae en la carpeta
# real del usuario. Con un enlace duro, borrar el nombre de la instancia no
# afecta al original, que sigue teniendo su propio enlace.
$instanceModels = Join-Path $InstanceDir 'Data\models'
if (Test-Path $ModelsDir) {
    New-Item -ItemType Directory -Force -Path $instanceModels | Out-Null
    $linked = 0
    # OJO: la variable del bucle NO puede llamarse `$model`: PowerShell no
    # distingue mayusculas, asi que pisaria el parametro `-Model` con el
    # ultimo fichero del directorio y el banco mediria un modelo distinto del
    # pedido, en silencio.
    foreach ($modelFile in (Get-ChildItem $ModelsDir -File -ErrorAction SilentlyContinue)) {
        $target = Join-Path $instanceModels $modelFile.Name
        if (Test-Path $target) { continue }
        try {
            New-Item -ItemType HardLink -Path $target -Target $modelFile.FullName -ErrorAction Stop | Out-Null
            $linked++
        } catch {
            Write-Warning "No se pudo enlazar $($modelFile.Name): $_"
        }
    }
    Write-Host "[setup] $linked modelo(s) enlazados por hard link (sin copia, sin junction)"
    if ($linked -eq 0) {
        Write-Warning "No hay modelos enlazados; la transcripcion no podra cargar ninguno."
    }
} else {
    Write-Warning "No hay modelos en $ModelsDir; la transcripcion no podra cargar ninguno."
}

# El modelo se queda en memoria durante la medicion: interesa la inferencia,
# no lapolitica de descarga.
# El idioma de reconocimiento. Vacio deja el valor por defecto de la app
# ('auto'), que deja que el modelo detecte el idioma; un codigo lo fija.
$instanceSettings = @{
    selected_model        = $selectedModel
    model_unload_timeout  = 'never'
    debug_mode            = $true
    start_hidden          = $true
    show_tray_icon        = $false
    transcribe_accelerator = 'auto'
}
if ($Language) { $instanceSettings['selected_language'] = $Language }

@{
    settings = $instanceSettings
} | ConvertTo-Json -Depth 5 | ForEach-Object {
    # Sin BOM: ver la nota en new-bench-instance.ps1. Con BOM, el plugin
    # `store` descarta el fichero y la app arranca con los ajustes por defecto.
    [System.IO.File]::WriteAllText(
        (Join-Path $InstanceDir 'Data\settings_store.json'),
        $_,
        (New-Object System.Text.UTF8Encoding($false))
    )
}

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

function Invoke-Transcription {
    <#
      Lanza el binario sin ventana visible y devuelve el objeto JSON.
      CreateNoWindow evita la consola; nada se escribe en el escritorio.
    #>
    param([string]$Fixture, [string]$ModelId, [int]$Runs)

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $instanceBinary
    $psi.Arguments = "--transcribe-file `"$Fixture`" --model $ModelId --repeat $Runs --json"
    $psi.WorkingDirectory = $InstanceDir
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $process = [System.Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()

    if ($process.ExitCode -ne 0) {
        throw "La transcripcion de $Fixture fallo (codigo $($process.ExitCode)): $stderr"
    }

    foreach ($line in ($stdout -split "`r?`n")) {
        $t = $line.Trim()
        if ($t.StartsWith('{')) {
            $data = $null
            try { $data = $t | ConvertFrom-Json } catch { }
            if ($null -eq $data) { continue }
            # El modelo medido tiene que ser el pedido. Un banco que mide otra
            # cosa y lo anota en la columna equivocada no sirve: el fallo es
            # silencioso y el informe sale verde.
            if ($ModelId -and $data.model -ne $ModelId) {
                throw "El banco pidio '$ModelId' pero se midio '$($data.model)'"
            }
            return $data
        }
    }
    throw "La transcripcion de $Fixture no devolvio JSON: $stdout"
}

# ---------------------------------------------------------------------------
# Medicion
# ---------------------------------------------------------------------------
$results = @()

# El corpus de referencia esta a 22050 Hz y la ruta headless solo acepta
# 16000 Hz (el formato que produce la captura del microfono). Se remuestrea
# una vez por corrida; el remuestreo es lineal e identico antes y despues,
# asi que la comparacion sigue siendo valida.
$benchAudioDir = Join-Path $InstanceDir 'audio-16k'
Write-Host "[setup] remuestreando el corpus a 16 kHz" -ForegroundColor Cyan
$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) { $python = Get-Command python3 -ErrorAction SilentlyContinue }
if (-not $python) {
    Write-Error "Se necesita python para remuestrear el corpus a 16 kHz."
}
& $python.Source (Join-Path $PSScriptRoot 'prepare-bench-audio.py') $FixturesDir $benchAudioDir
if ($LASTEXITCODE -ne 0) { Write-Error "El remuestreo del corpus fallo." }

foreach ($fixture in (Get-ChildItem $benchAudioDir -Filter '*.wav' | Sort-Object Name)) {
    Write-Host "[medir] $($fixture.Name)" -ForegroundColor Cyan
    try {
        $data = Invoke-Transcription -Fixture $fixture.FullName -ModelId $Model -Runs $Repeat
    } catch {
        Write-Warning "No se pudo medir $($fixture.Name): $_"
        continue
    }

    # De cada corrida se toma el MINIMO por etapa: una corrida que compite con
    # el resto de la maquina dice mas de la maquina que del modelo. El
    # minimo es el estimador menos ruidoso del tiempo real del motor.
    $asrMin = [int]::MaxValue
    $rulesMin = [int]::MaxValue
    $totalMin = [int]::MaxValue
    # Ademas del minimo se guardan todos los valores por etapa: sin la
    # distribucion no se puede afirmar nada sobre la latencia habitual, y el
    # minimo de las repeticiones es el estimador menos ruidoso pero no el
    # tipico.
    $asrAll = @()
    $totalAll = @()
    foreach ($run in @($data.runs)) {
        if (-not $run) { continue }
        $st = $run.stages
        if ($st) {
            if ([int]$st.asr -lt $asrMin) { $asrMin = [int]$st.asr }
            if ([int]$st.rules -lt $rulesMin) { $rulesMin = [int]$st.rules }
            if ([int]$run.total_ms -lt $totalMin) { $totalMin = [int]$run.total_ms }
            $asrAll += [double]$st.asr
            $totalAll += [double]$run.total_ms
        }
    }
    if ($asrMin -eq [int]::MaxValue) { $asrMin = 0 }
    if ($rulesMin -eq [int]::MaxValue) { $rulesMin = 0 }
    if ($totalMin -eq [int]::MaxValue) { $totalMin = 0 }
    $asrP50 = [int](Get-Percentile -Values $asrAll -Percentile 50)
    $asrP95 = [int](Get-Percentile -Values $asrAll -Percentile 95)
    $totalP50 = [int](Get-Percentile -Values $totalAll -Percentile 50)
    $totalP95 = [int](Get-Percentile -Values $totalAll -Percentile 95)

    $results += [pscustomobject]@{
        Fixture    = $fixture.Name
        AudioSecs  = [double]$data.audio_secs
        Model      = $data.model
        Backend    = $data.bound_backend
        LoadMs     = [int]$data.load_ms
        BestMs     = [int]$data.best_ms
        Rtf        = [double]$data.rtf
        AsrMs      = $asrMin
        AsrP50Ms   = $asrP50
        AsrP95Ms   = $asrP95
        TotalP50Ms = $totalP50
        TotalP95Ms = $totalP95
        RulesMs    = $rulesMin
        TotalMs    = $totalMin
        Text       = $(if ($NoText) { '' } else { $data.text })
        Ejecuciones = @($data.runs)
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

# Una fila por ejecucion. Los percentiles agrupados se calculan sobre
# ESTA lista, no sobre las siete filas por grabacion.
$ejecuciones = @()
foreach ($r in $results) {
    foreach ($d in $r.Ejecuciones) {
        $ejecuciones += [pscustomobject]@{
            Fixture   = $r.Fixture
            AudioSecs = $r.AudioSecs
            Corrida   = $d.run
            AsrMs     = [double]$d.stages.asr
            TotalMs   = [double]$d.total_ms
        }
    }
}

$CORTE_SEGUNDOS = 16.0

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

$cortas = @($ejecuciones | Where-Object { $_.AudioSecs -le $CORTE_SEGUNDOS })
$largas = @($ejecuciones | Where-Object { $_.AudioSecs -gt $CORTE_SEGUNDOS })

$stats = @(
    (StatDe -Nombre 'AGRUPADO - inferencia ASR (ms)' -Valores @($ejecuciones | ForEach-Object { $_.AsrMs })),
    (StatDe -Nombre ('SUBCONJUNTO <= ' + $CORTE_SEGUNDOS + ' s - inferencia ASR (ms)') -Valores @($cortas | ForEach-Object { $_.AsrMs })),
    (StatDe -Nombre ('SUBCONJUNTO > ' + $CORTE_SEGUNDOS + ' s - inferencia ASR (ms)') -Valores @($largas | ForEach-Object { $_.AsrMs })),
    (StatDe -Nombre 'POR GRABACION - minimo de la inferencia ASR (ms)' -Valores @($results | ForEach-Object { [double]$_.AsrMs })),
    (StatDe -Nombre 'POR GRABACION - p50 de la inferencia ASR (ms)' -Valores @($results | ForEach-Object { [double]$_.AsrP50Ms })),
    (StatDe -Nombre 'POR GRABACION - p95 de la inferencia ASR (ms)' -Valores @($results | ForEach-Object { [double]$_.AsrP95Ms })),
    (StatDe -Nombre 'Carga del modelo (ms)' -Valores @($results | ForEach-Object { [double]$_.LoadMs }))
)
$stats = @($stats | Where-Object { $null -ne $_ })

$audioTotal = ($results | Measure-Object -Property AudioSecs -Sum).Sum
$asrTotal = ($results | Measure-Object -Property AsrMs -Sum).Sum
$rtfGlobal = if ($asrTotal -gt 0) { [math]::Round($audioTotal / ($asrTotal / 1000), 3) } else { 0 }

$stats | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'estadisticas.json') -Encoding UTF8
$ejecuciones | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'ejecuciones.json') -Encoding UTF8
# El texto se guarda: es la evidencia del WER/CER (scripts/measure-wer.py).
# El corpus es audio sintetico de frases conocidas, no voz ni datos del
# usuario.
$results | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $OutDir 'resultados.json') -Encoding UTF8

$rows = foreach ($s in $stats) {
    "| $($s.Metrica) | $($s.N) | $($s.Media) | $($s.Min) | $($s.P50) | $($s.P95) | $($s.Max) |"
}
$detail = foreach ($r in $results) {
    "| $($r.Fixture) | $($r.AudioSecs) | $($r.LoadMs) | $($r.AsrMs) | $($r.AsrP50Ms) | $($r.AsrP95Ms) | $($r.TotalP50Ms) | $($r.TotalP95Ms) | $($r.Rtf) |"
}

@"
# Latencia del motor ASR - $Label

Banco reproducible sobre el corpus de referencia
(``docs/validation-phase2a/audio-referencia``), con ``--repeat $Repeat`` por
audio y quedandose con la mejor pasada.

- Binario: ``$Exe``
- Modelo: ``$Model``
- Instancia aislada: ``$InstanceDir`` (modelos por junction)
- CPU / RAM de referencia: 4 nucleos, 16 GB

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
$($rows -join "`n")

Factor de tiempo real global (audio total / inferencia total): **$rtfGlobal x**

## Detalle por audio

| Audio | Audio (s) | Carga (ms) | ASR min | ASR p50 | ASR p95 | Total p50 | Total p95 | RTF |
|---|---|---|---|---|---|---|---|---|
$($detail -join "`n")
"@ | Set-Content -Path (Join-Path $OutDir 'informe.md') -Encoding UTF8

Write-Host ''
Write-Host "Informe escrito en $(Join-Path $OutDir 'informe.md')" -ForegroundColor Green
$stats | Format-Table -AutoSize
