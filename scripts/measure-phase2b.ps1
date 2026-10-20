<#
  measure-phase2b.ps1 - mide el consumo de Dixi en cinco condiciones
  controladas, sobre una instancia AISLADA (portable) que no toca la
  instalacion real del usuario.

  CONDICIONES
  -----------
  1. ventana-abierta     : la app corriendo con la ventana visible.
  2. solo-bandeja         : la app corriendo con --start-hidden.
  3. modelo-descargado    : la app en reposo, con el modelo descargado de
                            memoria (la instancia aislada usa un tiempo de
                            descarga de 15 s).
  4. modelo-cargado       : la app en reposo, con el modelo en memoria.
  5. transcripcion-activa : la app transcribiendo.

  Se reportan RAM privada, working set y CPU, con el desglose por proceso
  (app principal, worker de ASR y runtime de WebView2).

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-phase2b.ps1
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-phase2b.ps1 -Seconds 20
#>
param(
    [int]$Seconds = 15,
    [int]$IntervalMs = 250,
    [string]$InstanceDir = "$env:TEMP\dixi-phase2b-instance",
    [string]$OutDir = "",
    [string]$ModelsDir = "$env:APPDATA\com.dixi.app\models"
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$measureScript = Join-Path $PSScriptRoot 'measure-dixi-tree.ps1'
$releaseBinary = Join-Path $repoRoot 'src-tauri\target\release\dixi.exe'

if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot 'docs\validation-phase2b\performance'
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if (-not (Test-Path $releaseBinary)) {
    Write-Error "No se encuentra el binario de release: $releaseBinary"
}

# ---------------------------------------------------------------------------
# Instancia aislada
# ---------------------------------------------------------------------------
Write-Host "[setup] preparando la instancia aislada en $InstanceDir" -ForegroundColor Cyan

if (Test-Path $InstanceDir) {
    Remove-Item -Recurse -Force $InstanceDir
}
New-Item -ItemType Directory -Force -Path $InstanceDir | Out-Null

Copy-Item $releaseBinary (Join-Path $InstanceDir 'dixi.exe') -Force
Set-Content -Path (Join-Path $InstanceDir 'portable') -Value 'Dixi Portable Mode' -Encoding UTF8
New-Item -ItemType Directory -Force -Path (Join-Path $InstanceDir 'Data') | Out-Null

# Los modelos se comparten por junction: no se duplican los cientos de
# megabytes, y la instalacion real no se modifica.
$instanceModels = Join-Path $InstanceDir 'Data\models'
if (Test-Path $ModelsDir) {
    cmd /c "mklink /J `"$instanceModels`" `"$ModelsDir`"" | Out-Null
    Write-Host "[setup] modelos compartidos por junction desde $ModelsDir"
} else {
    Write-Warning "No hay modelos en $ModelsDir; la transcripcion no podra cargar ninguno."
}

# Ajustes de la instancia: modelo conocido, descarga rapida del modelo y
# modo debug (que habilita la descarga a los 15 s).
$settings = @{
    settings = @{
        selected_model        = 'handy-computer/whisper-base-gguf/whisper-base-Q8_0.gguf'
        model_unload_timeout  = 'sec15'
        debug_mode            = $true
        start_hidden          = $false
        show_tray_icon        = $true
        transcribe_accelerator = 'auto'
    }
}
$settings | ConvertTo-Json -Depth 5 | Set-Content -Path (Join-Path $InstanceDir 'Data\settings_store.json') -Encoding UTF8

$instanceBinary = Join-Path $InstanceDir 'dixi.exe'

# ---------------------------------------------------------------------------
# Utilidades
# ---------------------------------------------------------------------------
$results = @()

function Stop-Instance {
    Get-Process -Name dixi -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $instanceBinary } |
        ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Milliseconds 800
}

function Start-Instance {
    param([switch]$Hidden)
    # PowerShell 5.1 rejects an empty -ArgumentList, so the flag is only
    # passed when there is one.
    if ($Hidden) {
        Start-Process -FilePath $instanceBinary -ArgumentList '--start-hidden' `
            -WorkingDirectory $InstanceDir -WindowStyle Hidden | Out-Null
    } else {
        Start-Process -FilePath $instanceBinary `
            -WorkingDirectory $InstanceDir -WindowStyle Hidden | Out-Null
    }
    Start-Sleep -Seconds 5
}

function Measure-Condition {
    param(
        [string]$Label,
        [string]$Title,
        [scriptblock]$During = $null
    )

    Write-Host "[medir] $Label" -ForegroundColor Cyan
    $out = Join-Path $OutDir "$Label.txt"

    if ($During) {
        $job = Start-Job -ScriptBlock {
            param($script, $seconds, $interval, $outPath)
            & $script -Seconds $seconds -IntervalMs $interval -Label 'x' -Out $outPath
        } -ArgumentList $measureScript, $Seconds, $IntervalMs, $out

        Start-Sleep -Seconds 3
        & $During
        Wait-Job $job | Out-Null
        Receive-Job $job | Out-Null
        Remove-Job $job -Force
    } else {
        & $measureScript -Seconds $Seconds -IntervalMs $IntervalMs -Label $Label -Out $out | Out-Null
    }

    if (-not (Test-Path $out)) { $out = '' }
    $text = if ($out -ne '') { Get-Content $out -Raw } else { '' }

    # Se extraen los agregados que imprime el script de medicion.
    $private = 0.0; $working = 0.0; $cpu = 0.0; $procs = 0
    $m = [regex]::Match($text, 'TOTAL.*?privado=\s*([\d\.]+)\s*MB.*?ws=\s*([\d\.]+)\s*MB')
    if ($m.Success) {
        $private = [double]$m.Groups[1].Value
        $working = [double]$m.Groups[2].Value
    }
    $m2 = [regex]::Match($text, 'CPU\s*=?\s*([\d\.]+)')
    if ($m2.Success) { $cpu = [double]$m2.Groups[1].Value }
    $m3 = [regex]::Match($text, 'procesos\s*=?\s*(\d+)')
    if ($m3.Success) { $procs = [int]$m3.Groups[1].Value }

    $script:results += [pscustomobject]@{
        Condicion = $Title
        PrivadaMB = [math]::Round($private, 1)
        WorkingMB = [math]::Round($working, 1)
        CpuPct    = [math]::Round($cpu, 2)
        Procesos  = $procs
        Detalle   = $out
    }

    Write-Host ("         privada={0} MB  ws={1} MB  cpu={2}%  procesos={3}" -f $private, $working, $cpu, $procs)
}

try {
    # 1. Ventana abierta, en reposo (el modelo todavia no se cargo).
    Stop-Instance
    Start-Instance
    Measure-Condition -Label '1-ventana-abierta' -Title 'Ventana abierta'

    # 2. Solo en la bandeja.
    Stop-Instance
    Start-Instance -Hidden
    Measure-Condition -Label '2-solo-bandeja' -Title 'Aplicacion unicamente en bandeja'

    # 3. Modelo descargado de memoria: se dicta, se espera a que el motor
    #    descargue el modelo (15 s en modo debug) y se mide en reposo.
    Write-Host '[medir] 3-modelo-descargado (se dicta y se espera la descarga)' -ForegroundColor Cyan
    & $instanceBinary --toggle-transcription | Out-Null
    Start-Sleep -Seconds 20
    Measure-Condition -Label '3-modelo-descargado' -Title 'Modelo ASR descargado de memoria'

    # 4. Modelo cargado: se dicta y se mide nada mas terminar, con el modelo
    #    todavia en memoria.
    & $instanceBinary --toggle-transcription | Out-Null
    Start-Sleep -Seconds 4
    Measure-Condition -Label '4-modelo-cargado' -Title 'Modelo cargado'

    # 5. Transcripcion activa: se mide mientras se dicta.
    Measure-Condition -Label '5-transcripcion-activa' -Title 'Transcripcion activa' -During {
        & $instanceBinary --toggle-transcription | Out-Null
    }
} finally {
    Stop-Instance
}

# ---------------------------------------------------------------------------
# Informe
# ---------------------------------------------------------------------------
$report = Join-Path $OutDir 'informe.md'
$rows = foreach ($r in $results) {
    "| $($r.Condicion) | $($r.PrivadaMB) | $($r.WorkingMB) | $($r.CpuPct) | $($r.Procesos) |"
}

@"
# Consumo de memoria de Dixi (Fase 2B)

Medido con ``scripts/measure-dixi-tree.ps1`` sobre una instancia **aislada**
(carpeta portable propia, con los modelos compartidos por junction). La
instalacion real del usuario no se toco.

- Duracion de cada muestra: $Seconds s, cada $IntervalMs ms.
- RAM privada: lo que el proceso reserva de verdad.
- Working set: lo que ocupa en RAM, incluyendo paginas compartidas con el
  runtime de WebView2, que es un componente del sistema compartido.
- CPU: porcentaje medio del arbol de procesos durante la muestra.

| Condicion | RAM privada (MB) | Working set (MB) | CPU (%) | Procesos |
|---|---|---|---|---|
$($rows -join "`n")

El desglose por proceso (app principal, worker de ASR, runtime de WebView2)
esta en los ficheros ``.txt`` de este mismo directorio.
"@ | Set-Content -Path $report -Encoding UTF8

Write-Host ''
Write-Host "Informe escrito en $report" -ForegroundColor Green
$results | Format-Table -AutoSize
