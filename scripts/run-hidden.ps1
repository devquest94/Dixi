<#
  run-hidden.ps1 - ejecuta un comando SIN abrir ninguna ventana de consola.

  Por que existe
  --------------
  Los comandos lanzados desde una sesion de agente abren su propia consola
  de Windows y le roban el foco a la persona que esta trabajando. Esta
  regla es obligatoria: una tarea que compila bien pero interrumpe no
  cumple.

  Que garantiza
  -------------
  1. `-WindowStyle Hidden` en `Start-Process`: el proceso no crea ventana.
  2. `stdout` y `stderr` van a ficheros de log, para no perderlos y para
     poder diagnosticar sin abrir una consola.
  3. El codigo de salida real se devuelve, no el del script.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-hidden.ps1 `
        -FilePath cargo -ArgumentList @("build","--release") `
        -WorkingDirectory src-tauri -LogFile .logs\build.log

  O, mas comodo, con el wrapper de PowerShell:
    .\scripts\run-hidden.ps1 -FilePath "cargo" -ArgumentList "build","--release" -LogFile "D:\logs\build.log"

  El proceso espera a que termine y devuelve su codigo de salida.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [string[]]$ArgumentList = @(),

    [string]$WorkingDirectory = "",

    [string]$LogFile = "",

    # Espera a que termine el proceso. Desactivalo para tareas largas.
    [switch]$NoWait
)

$ErrorActionPreference = 'Stop'

if (-not $LogFile) {
    $logDir = Join-Path $env:TEMP 'dixi-run-logs'
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $safe = ($FilePath -replace '[\\/:*?"<>|]', '_')
    $LogFile = Join-Path $logDir "$safe-$stamp.log"
}

$logDir = Split-Path -Parent $LogFile
if ($logDir -and -not (Test-Path $logDir)) {
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
}

$stdoutLog = "$LogFile.out"
$stderrLog = "$LogFile.err"

$startArgs = @{
    FilePath               = $FilePath
    WindowStyle            = 'Hidden'
    RedirectStandardOutput = $stdoutLog
    RedirectStandardError  = $stderrLog
    PassThru               = $true
    NoNewWindow            = $false
}

if ($ArgumentList.Count -gt 0) {
    $startArgs['ArgumentList'] = $ArgumentList
}
if ($WorkingDirectory -and (Test-Path $WorkingDirectory)) {
    $startArgs['WorkingDirectory'] = (Resolve-Path $WorkingDirectory).Path
}

$process = Start-Process @startArgs

# Guardar el Handle antes de esperar obliga a .NET a cachear el proceso y
# garantiza que ExitCode tenga valor despues de WaitForExit().
$processHandle = $process.Handle

if ($NoWait) {
    Write-Host "Iniciado en segundo plano (PID $($process.Id)). Log: $stdoutLog"
    exit 0
}

$process.WaitForExit()

# Se unen las dos salidas en un solo log, que es lo que se quiere leer.
$merged = @()
foreach ($file in @($stdoutLog, $stderrLog)) {
    if (Test-Path $file) {
        $merged += Get-Content $file
    }
}
$merged | Set-Content -Path $LogFile -Encoding UTF8
Remove-Item $stdoutLog, $stderrLog -ErrorAction SilentlyContinue

Write-Host "--- salida ($FilePath, codigo $($process.ExitCode)) ---"
$merged | Select-Object -Last 40
Write-Host "--- log completo: $LogFile ---"

exit $process.ExitCode
