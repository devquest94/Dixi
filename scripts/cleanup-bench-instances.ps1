<#
  cleanup-bench-instances.ps1 - limpia las carpetas que el banco deja en %TEMP%.

  QUE HACE
  --------
  Las instancias aisladas del banco se crean con un enlace a la carpeta de
  modelos del usuario (junction en las instancias antiguas, enlaces duros en
  las nuevas). Borrar la carpeta sin antes cortar el enlace puede borrar los
  modelos de verdad: en Windows PowerShell 5.1, `Remove-Item -Recurse` sigue
  los junctions y VACIA el destino.

  Por eso el orden es el que importa:
    1. se enumeran los junctions y se corta el ENLACE (con la API de .NET,
       que no sigue el enlace),
    2. se comprueba que ningun destino de junction cae dentro de la carpeta
       que se va a borrar,
    3. y solo entonces se borra la carpeta.

  NADA SE BORRA SIN AUTORIZACION: sin -Execute el script solo informa.

  USO
  ---
    .\cleanup-bench-instances.ps1                      # solo informa
    .\cleanup-bench-instances.ps1 -Execute             # borra lo seguro
    .\cleanup-bench-instances.ps1 -Execute -IncludeMine # incluye las de esta sesion
#>
param(
    # Borra de verdad. Sin este parametro el script solo informa.
    [switch]$Execute,
    # Incluye tambien las instancias creadas en esta sesion (dixi-bench-*).
    [switch]$IncludeMine,
    # Prefijos de las carpetas a considerar.
    [string[]]$Prefix = @('dixi-latency-'),
    [string]$TempDir = $env:TEMP
)

$ErrorActionPreference = 'Stop'

if (-not $IncludeMine) {
    $Prefix = @('dixi-latency-')
} else {
    # Las de esta sesion: las instancias del banco, los clips y las COPIAS
    # de las grabaciones del usuario. Estas ultimas son contenido privado:
    # no deberian quedarse en una carpeta temporal.
    $Prefix = @('dixi-latency-', 'dixi-bench-', 'dixi-voice', 'dixi-clips')
}

Write-Host "Inventario de las carpetas del banco en $TempDir" -ForegroundColor Cyan
Write-Host ''

$candidates = @()
foreach ($p in $Prefix) {
    $candidates += @(Get-ChildItem $TempDir -Directory -Filter "${p}*" -ErrorAction SilentlyContinue)
}
$candidates = @($candidates | Sort-Object FullName -Unique)

if ($candidates.Count -eq 0) {
    Write-Host "No hay carpetas que limpiar." -ForegroundColor Green
    return
}

$total = 0
$plan = @()

foreach ($dir in $candidates) {
    $files = @(Get-ChildItem $dir.FullName -Recurse -File -ErrorAction SilentlyContinue)
    $logical = ($files | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $logical) { $logical = 0 }
    # Los enlaces duros no ocupan datos propios: solo cuentan si la carpeta
    # que se borra es la unica que los tiene.
    $shared = ($files | Where-Object { $_.LinkType -eq 'HardLink' } | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $shared) { $shared = 0 }
    $real = $logical - $shared
    $total += $real

    $junctions = @(Get-ChildItem $dir.FullName -Recurse -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.LinkType -eq 'Junction' })

    # SAFETY: ningun destino de junction puede caer dentro de la carpeta.
    $unsafe = $false
    foreach ($j in $junctions) {
        foreach ($target in @($j.Target)) {
            if ($target -and $target.StartsWith($dir.FullName, [System.StringComparison]::OrdinalIgnoreCase)) {
                Write-Host "  INSEGURO: $($j.FullName) apunta dentro de la carpeta" -ForegroundColor Red
                $unsafe = $true
            }
        }
    }

    $plan += [pscustomobject]@{
        Carpeta  = $dir.FullName
        RealMB   = [math]::Round($real / 1MB, 1)
        Archivos = $files.Count
        Junctions = $junctions.Count
        Destinos = ($junctions | ForEach-Object { $_.Target }) -join '; '
        Seguro   = (-not $unsafe)
    }
}

$plan | Format-Table -AutoSize | Out-String | Write-Host
Write-Host ("Total real a liberar: {0:N1} MB" -f ($total / 1MB)) -ForegroundColor Cyan
Write-Host ''

if (-not $Execute) {
    Write-Host "Informe: no se ha borrado nada. Usa -Execute para limpiar lo seguro." -ForegroundColor Yellow
    return
}

$freed = 0
foreach ($item in $plan) {
    if (-not $item.Seguro) {
        Write-Host "Se omite $($item.Carpeta): el junction apunta dentro de la carpeta" -ForegroundColor Red
        continue
    }
    # 1) Cortar los enlaces ANTES de borrar: Remove-Item -Recurse sigue los
    #    junctions en PowerShell 5.1 y vaciaria la carpeta de modelos.
    $junctions = @(Get-ChildItem $item.Carpeta -Recurse -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.LinkType -eq 'Junction' })
    foreach ($j in $junctions) {
        try {
            # Directory::Delete sobre un reparse point quita el enlace, no el
            # destino.
            [System.IO.Directory]::Delete($j.FullName)
            Write-Host "  junction cortado: $($j.FullName)" -ForegroundColor DarkGray
        } catch {
            Write-Warning "No se pudo cortar el junction $($j.FullName): $_"
        }
    }
    # 2) Ahora si, la carpeta.
    try {
        Remove-Item -LiteralPath $item.Carpeta -Recurse -Force
        Write-Host "Borrada: $($item.Carpeta) ($($item.RealMB) MB)" -ForegroundColor Green
        $freed += $item.RealMB
    } catch {
        Write-Warning "No se pudo borrar $($item.Carpeta): $_"
    }
}
Write-Host ''
Write-Host ("Liberados {0:N1} MB" -f $freed) -ForegroundColor Green
