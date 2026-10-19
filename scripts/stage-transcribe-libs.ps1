<#
  stage-transcribe-libs.ps1 — prepara src-tauri/transcribe-libs/ con el
  runtime de Visual C++ redistribuible.

  POR QUE EXISTE
  -------------
  `tauri.windows.conf.json` declara el recurso "transcribe-libs" -> ".". Tauri
  exige que la ruta de origen exista, asi que el directorio debe existir
  ANTES de compilar. Upstream (Handy) lo resuelve en su CI (build.yml), que
  copia el runtime de VC++ a app-local para que la aplicacion arranque en
  equipos donde no este instalado el redistribuible (issue #1527).

  En un clon limpio este paso no se hace solo: hay que ejecutarlo.

  Los DLL son ARTEFACTOS DE COMPILACION: no se versionan (ver .gitignore).

  Uso:  powershell -File scripts/stage-transcribe-libs.ps1
#>
[CmdletBinding()]
param(
    [string]$Destination,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

# $PSScriptRoot no esta disponible en los valores por defecto de `param`
# en Windows PowerShell 5.1: se resuelve aqui dentro.
if (-not $Destination) {
    $scriptRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Destination = Join-Path $scriptRoot '..\src-tauri\transcribe-libs'
}

function Find-VcRedistDir {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $roots = @()
    if (Test-Path $vswhere) {
        $json = & $vswhere -products * -prerelease -format json 2>$null | ConvertFrom-Json
        $roots += $json | ForEach-Object { $_.installationPath }
    }
    $roots += @(
        (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio'),
        (Join-Path $env:ProgramFiles 'Microsoft Visual Studio')
    )

    $found = @()
    foreach ($root in $roots) {
        if (-not $root -or -not (Test-Path $root)) { continue }
        $redist = Join-Path $root 'VC\Redist\MSVC'
        if (-not (Test-Path $redist)) { continue }
        Get-ChildItem $redist -Directory -ErrorAction SilentlyContinue |
            Where-Object { Test-Path (Join-Path $_.FullName 'x64') } |
            ForEach-Object {
                Get-ChildItem (Join-Path $_.FullName 'x64') -Directory -ErrorAction SilentlyContinue |
                    Where-Object { $_.Name -like 'Microsoft.VC*CRT*' } |
                    ForEach-Object { $found += $_.FullName }
            }
    }
    # La version mas reciente primero.
    $found | Sort-Object -Descending
}

$dest = [System.IO.Path]::GetFullPath($Destination)

if ((Test-Path $dest) -and -not $Force) {
    $existing = @(Get-ChildItem $dest -Filter '*.dll' -ErrorAction SilentlyContinue)
    if ($existing.Count -gt 0) {
        Write-Output "[stage] $dest ya contiene $($existing.Count) DLL; se conserva (use -Force para regenerar)"
        exit 0
    }
}

$source = Find-VcRedistDir | Select-Object -First 1
if (-not $source) {
    throw "No se encontro el runtime redistribuible de Visual C++. Instale las Build Tools de Visual Studio con el componente 'VC++ redistribuible'."
}
Write-Output "[stage] origen: $source"

New-Item -ItemType Directory -Force -Path $dest | Out-Null

# Solo los DLL de release: los de depuracion (sufijo 'd') no se distribuyen.
$copied = 0
Get-ChildItem $source -Filter '*.dll' | Where-Object { $_.Name -notmatch 'd\.dll$' } | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $dest $_.Name) -Force
    Write-Output "[stage]   $($_.Name)"
    $copied++
}

if ($copied -eq 0) {
    throw "No se copio ningun DLL desde $source"
}
Write-Output "[stage] $copied DLL en $dest"
