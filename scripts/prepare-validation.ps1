<#
  prepare-validation.ps1 — deja Dixi en un estado conocido y determinista para
  la validacion manual del dictado.

  Por que hace falta:
   - `selected_model` vacio + onboarding pendiente => la app no carga NINGUN
     modelo ("Skipping model auto-selection until onboarding is complete") y
     el dictado no puede funcionar.
   - `selected_language = "auto"` => deteccion automatica de idioma. Es
     exactamente el punto donde canary-180m-flash fallo (traducio el
     espanol al ingles), asi que para la prueba se fija "es".
   - `reliable_paste = false` => se usa el pegado heredado, que NO restaura el
     portapapeles. Con `true` se activa la via fiable, que instantanea el
     portapapeles y lo restaura cuando el destino ha leido el texto.
   - `model_unload_timeout = "min5"` => el modelo se descarga a los 5 min de
     inactividad, asi que la primera transiccion tras una pausa paga la
     carga. Con "never" la latencia es estable y comparable.

  El store se parchea con Node (scripts/patch-settings.mjs) porque en Windows
  PowerShell 5.1 el round-trip en JSON de UTF-8 anade BOM y deforma las
  cadenas no ASCII; ver el comentario de ese script.

  Todo lo que toca es reversible: hay una copia del fichero de ajustes.

  Uso:
    powershell -File scripts/prepare-validation.ps1 [-Launch] [-Restore]
#>
param(
    [switch]$Launch,
    [switch]$Restore
)

$ErrorActionPreference = 'Continue'

$appData = "$env:APPDATA\com.dixi.app"
$store = Join-Path $appData 'settings_store.json'
$modelsDir = Join-Path $appData 'models'
$exe = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\src-tauri\target\release\dixi.exe'))
$patcher = Join-Path $PSScriptRoot 'patch-settings.mjs'
$MODEL_FILE = 'whisper-base-Q8_0.gguf'

# --- parar la app (los ajustes se guardan al salir) --------------------------
$running = Get-Process -Name dixi -ErrorAction SilentlyContinue
if ($running) {
    Write-Output "[prep] parando Dixi ($($running.Count) proceso/s) para editar los ajustes"
    $running | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 2
}

if (-not (Test-Path $store)) { Write-Output "[prep] ERROR: no existe $store"; exit 1 }

if ($Restore) {
    & node $patcher $store --restore
    exit $LASTEXITCODE
}

# --- comprobar que el modelo esta descargado --------------------------------
if (-not (Test-Path (Join-Path $modelsDir $MODEL_FILE))) {
    Write-Output "[prep] ERROR: falta el modelo $MODEL_FILE en $modelsDir"
    Write-Output "[prep] descargalo con:"
    Write-Output "[prep]   python scripts/fetch_models.py src-tauri/src/catalog/catalog.json `"$modelsDir`" handy-computer/whisper-base-gguf"
    exit 1
}
Write-Output "[prep] modelo presente: $MODEL_FILE"

# --- aplicar los ajustes (UTF-8 seguro) -------------------------------------
& node $patcher $store
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# --- relanzar ---------------------------------------------------------------
if ($Launch) {
    if (-not (Test-Path $exe)) { Write-Output "[prep] ERROR: no existe el binario $exe"; exit 1 }
    Write-Output "[prep] lanzando $exe (GUI visible, sin consola)"
    $p = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds 8
    $p.Refresh()
    if ($p.HasExited) { Write-Output "[prep] ERROR: la app termino al arrancar"; exit 1 }
    Write-Output "[prep] en marcha pid=$($p.Id) ventana='$($p.MainWindowTitle)'"

    $log = "$env:LOCALAPPDATA\com.dixi.app\logs\dixi.log"
    if (Test-Path $log) {
        Write-Output ""
        Write-Output "[prep] ultimas lineas del log:"
        Get-Content $log -Tail 10 | ForEach-Object { Write-Output "  $_" }
    }
}
