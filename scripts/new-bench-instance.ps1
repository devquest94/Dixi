<#
  new-bench-instance.ps1 - prepara la instancia aislada que usa el banco.

  QUE HACE
  --------
  Una carpeta propia para la corrida, con el binario copiado y los modelos
  ENLAZADOS POR ENLACE DURO (archivo por archivo): no se duplican los cientos
  de megabytes y la instalacion real del usuario no se toca.

  Un junction de carpeta NO sirve aqui: la app puede borrar un `.partial`
  residual en `models_dir`, y con un junction ese borrado cae en la carpeta
  real del usuario. Con un enlace duro, borrar el nombre de la instancia no
  afecta al original, que sigue teniendo su propio enlace.

  Los ajustes de la instancia dejan el modelo en memoria durante la medicion
  (interesa la inferencia, no la politica de descarga) y fijan el idioma de
  reconocimiento cuando se pide uno.

  Uso:
    .\new-bench-instance.ps1 -InstanceDir <dir> -Exe <dixi.exe> -Model <id> [-Language es]
#>
param(
    [Parameter(Mandatory = $true)][string]$InstanceDir,
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$Language = "",
    [string]$ModelsDir = "$env:APPDATA\com.dixi.app\models",
    # Carpetas adicionales de donde enlazar modelos (por ejemplo una
    # descarga aislada que todavia no esta en la carpeta del usuario).
    [string[]]$ExtraModelsDir = @()
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Exe)) {
    Write-Error "No se encuentra el binario: $Exe"
}

New-Item -ItemType Directory -Force -Path $InstanceDir | Out-Null

Copy-Item $Exe (Join-Path $InstanceDir 'dixi.exe') -Force
Set-Content -Path (Join-Path $InstanceDir 'portable') -Value 'Dixi Portable Mode' -Encoding UTF8
New-Item -ItemType Directory -Force -Path (Join-Path $InstanceDir 'Data') | Out-Null

# Los recursos (iconos de la bandeja, sonidos) se resuelven con respecto al
# ejecutable cuando la app no esta empaquetada.
$resourcesSource = Join-Path (Split-Path -Parent $PSScriptRoot) 'src-tauri\resources'
if (Test-Path $resourcesSource) {
    Copy-Item $resourcesSource (Join-Path $InstanceDir 'resources') -Recurse -Force
}

$instanceModels = Join-Path $InstanceDir 'Data\models'
# De donde se enlazan los modelos: la carpeta del usuario y las extra.
$sources = @($ModelsDir) + @($ExtraModelsDir) | Where-Object { $_ -and (Test-Path $_) }
if ($sources.Count -gt 0) {
    New-Item -ItemType Directory -Force -Path $instanceModels | Out-Null
    $linked = 0
    $allModels = @()
    foreach ($source in $sources) {
        $allModels += @(Get-ChildItem $source -File -Filter '*.gguf' -ErrorAction SilentlyContinue)
    }
    foreach ($modelFile in ($allModels | Sort-Object Name)) {
        $target = Join-Path $instanceModels $modelFile.Name
        if (Test-Path $target) {
            # Ya enlazado en una corrida anterior: cuenta igual, el enlace
            # sigue apuntando al modelo real y no ocupa datos extra.
            $linked++
            continue
        }
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

$instanceSettings = @{
    selected_model        = $Model
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
    # Sin BOM: `Set-Content -Encoding UTF8` en PowerShell 5.1 escribe un BOM, y
    # el plugin `store` parsea el JSON con serde_json, que no acepta un BOM:
    # el fichero se descartaba y la app arrancaba con los ajustes por defecto
    # (idioma `auto` en vez del forzado, y sin el timeout de descarga).
    [System.IO.File]::WriteAllText(
        (Join-Path $InstanceDir 'Data\settings_store.json'),
        $_,
        (New-Object System.Text.UTF8Encoding($false))
    )
}

Write-Host "[setup] instancia lista en $InstanceDir"
