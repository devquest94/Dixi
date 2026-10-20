<#
  verify-release-protocol.ps1 - comprueba que el binario de PRODUCCION lleva
  su frontend incrustado (caracteristica `tauri/custom-protocol`).

  POR QUE EXISTE ESTA PRUEBA
  --------------------------
  Sin la caracteristica `custom-protocol`, un binario compilado con
  `cargo build --release` NO lleva el frontend dentro: la WebView2 intenta
  conectarse al servidor de desarrollo de Vite (http://localhost:1420), no
  existe, la interfaz queda en blanco, el frontend nunca ejecuta sus efectos
  y los atajos globales NO se registran. El fallo es silencioso: el binario
  arranca, ocupa la bandeja y no se ve nada.

  La seccion [features] de `src-tauri/Cargo.toml` expone `custom-protocol` con
  un nombre propio para poder pedirlo explicitamente en la linea de comandos.

  QUE COMPRUEBA
  -------------
  1. `src-tauri/Cargo.toml` declara la caracteristica y la reenvia a
     `tauri/custom-protocol`.
  2. `dist/` existe y `dist/index.html` referencia al menos un recurso con
     hash (los nombres con hash son los que se incrustan).
  3. El binario release existe.
  4. El binario CONTIENE el nombre del recurso con hash. Esta es la prueba
     de verdad: solo una compilacion con `custom-protocol` incrusta los
     assets en el binario. Sin la caracteristica, el nombre no aparece y
     esta comprobacion falla.
  5. (Opcional, -SmokeTest) El binario arranca y responde: se le pide
     regenerar los bindings IPC y se comprueba que el archivo cambia.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-protocol.ps1
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-protocol.ps1 -SmokeTest
#>
param(
    [string]$Binary,
    [switch]$SmokeTest
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$tauriDir = Join-Path $repoRoot 'src-tauri'
$distDir = Join-Path $repoRoot 'dist'
$cargoToml = Join-Path $tauriDir 'Cargo.toml'

if (-not $Binary) {
    $Binary = Join-Path $tauriDir 'target\release\dixi.exe'
}

$failures = New-Object System.Collections.Generic.List[string]

function Write-Check {
    param([string]$Name, [bool]$Ok, [string]$Detail = '')

    $mark = 'FAIL'
    if ($Ok) { $mark = 'OK  ' }

    $suffix = ''
    if ($Detail -ne '') { $suffix = ' - ' + $Detail }

    Write-Host ('[{0}] {1}{2}' -f $mark, $Name, $suffix)
    if (-not $Ok) { $script:failures.Add($Name) }
}

Write-Host '--- verify-release-protocol ---' -ForegroundColor Cyan

# --- 1. La caracteristica existe y reenvia a tauri/custom-protocol ---------
if (-not (Test-Path $cargoToml)) {
    Write-Error "No se encuentra $cargoToml"
}
$cargoText = Get-Content $cargoToml -Raw
$featureDeclared = $cargoText -match '(?m)^\s*custom-protocol\s*=\s*\[[^\]]*tauri/custom-protocol[^\]]*\]'
Write-Check 'Cargo.toml declara custom-protocol -> tauri/custom-protocol' $featureDeclared

# --- 2. El frontend compilado existe y usa recursos con hash ---------------
$indexHtml = Join-Path $distDir 'index.html'
$hasDist = Test-Path $indexHtml
Write-Check 'dist/index.html existe' $hasDist

$assetNames = @()
if ($hasDist) {
    $indexText = Get-Content $indexHtml -Raw
    # Los nombres de recursos de Vite llevan hash: main-<hash>.js
    $matches = [regex]::Matches($indexText, '/assets/([A-Za-z0-9_\-.]+-[A-Za-z0-9_-]{6,}\.(?:js|css))')
    $assetNames = @($matches | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
}
Write-Check 'dist/index.html referencia recursos con hash' ($assetNames.Count -gt 0) ("$($assetNames.Count) recurso(s)")

# --- 3. El binario release existe ------------------------------------------
$binaryExists = Test-Path $Binary
Write-Check 'El binario release existe' $binaryExists $Binary

if (-not $binaryExists) {
    Write-Host ''
    Write-Host 'FALLO: no hay binario que verificar. Compila primero:' -ForegroundColor Red
    Write-Host '  npm run build'
    Write-Host '  cargo build --release --features custom-protocol'
    exit 1
}

# --- 4. El binario incrusta el frontend (la prueba de verdad) ---------------
$binaryText = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($Binary))
$embedded = @($assetNames | Where-Object { $binaryText.Contains($_) })
Write-Check 'El binario incrusta los recursos del frontend (custom-protocol activo)' `
    ($embedded.Count -eq $assetNames.Count -and $assetNames.Count -gt 0) `
    ("incrustados: $($embedded.Count)/$($assetNames.Count)")

if ($embedded.Count -lt $assetNames.Count) {
    Write-Host ''
    Write-Host 'El binario NO lleva el frontend dentro: se abriria en blanco.' -ForegroundColor Red
    Write-Host 'Compila con la caracteristica: cargo build --release --features custom-protocol' -ForegroundColor Red
}

# --- 5. Prueba de humo opcional: el binario arranca y responde -------------
if ($SmokeTest) {
    $bindings = Join-Path $repoRoot 'src\bindings.ts'
    $before = [datetime]::MinValue
    if (Test-Path $bindings) { $before = (Get-Item $bindings).LastWriteTimeUtc }

    $previous = $env:PATH
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$previous"
    $pushed = $false
    try {
        Push-Location $tauriDir
        $pushed = $true
        & $Binary --export-bindings | Out-Null
    } catch {
        Write-Check 'El binario arranca y regenera los bindings' $false $_.Exception.Message
    } finally {
        if ($pushed) { Pop-Location }
        $env:PATH = $previous
    }

    $after = [datetime]::MinValue
    if (Test-Path $bindings) { $after = (Get-Item $bindings).LastWriteTimeUtc }
    Write-Check 'El binario arranca y regenera los bindings' ($after -gt $before)
}

Write-Host ''
if ($failures.Count -gt 0) {
    Write-Host ('FALLO: {0} comprobacion(es) fallaron: {1}' -f $failures.Count, ($failures -join ', ')) -ForegroundColor Red
    exit 1
}
Write-Host 'OK: el binario de produccion lleva el frontend incrustado.' -ForegroundColor Green
exit 0
