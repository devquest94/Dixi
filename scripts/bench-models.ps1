<#
  bench-models.ps1 — mide los modelos ASR locales sobre audio sintetico.

  Usa la ruta de transaccion por lotes de la propia aplicacion
  (`--transcribe-file`), que es el MISMO camino que usa el dictado: no es un
  banco de pruebas aparte.

  El binario escribe logs por stdout junto al JSON, asi que se extrae el
  bloque JSON antes de interpretarlo.

  Uso:
    bench-models.ps1 -Exe <dixi.exe> -AppDataDir <dir> -ModelsDir <dir> `
                     -FixturesDir <dir> -Out <json> [-Repeat 3]
#>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$AppDataDir,
    [Parameter(Mandatory = $true)][string]$ModelsDir,
    [Parameter(Mandatory = $true)][string]$FixturesDir,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$Repeat = 3
)

$ErrorActionPreference = 'Continue'

function Get-Json($lines) {
    # Descarta los logs (el binario escribe en stdout) y devuelve el primer
    # bloque JSON que se pueda interpretar. El JSON de --json es una unica
    # linea; los logs llevan la marca de tiempo al principio.
    foreach ($line in $lines) {
        $t = ([string]$line).Trim()
        if (($t.StartsWith('{') -and $t.EndsWith('}')) -or ($t.StartsWith('[') -and $t.EndsWith(']'))) {
            try { return ($t | ConvertFrom-Json) } catch { }
        }
    }
    # Array multilinea: se reintenta con el bloque completo.
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $t = ([string]$lines[$i]).Trim()
        if ($t -eq '[' -or $t -eq '{') { $start = $i; break }
    }
    if ($start -lt 0) { return $null }
    try { return (($lines[$start..($lines.Count - 1)] -join "`n") | ConvertFrom-Json) } catch { return $null }
}

$targetModels = Join-Path $AppDataDir 'models'
New-Item -ItemType Directory -Force -Path $targetModels | Out-Null

Write-Output "[bench] copiando modelos a $targetModels"
Get-ChildItem $ModelsDir -Filter '*.gguf' | ForEach-Object {
    $dest = Join-Path $targetModels $_.Name
    if (-not (Test-Path $dest) -or (Get-Item $dest).Length -ne $_.Length) {
        Copy-Item $_.FullName $dest -Force
        Write-Output "[bench]   $($_.Name) ($([math]::Round($_.Length/1MB,1)) MB)"
    }
}

# Los ids reales los decide el registro de modelos: no se adivinan a partir
# del nombre de fichero.
$listLines = & $Exe --list-models --json 2>&1 | ForEach-Object { [string]$_ }
$registry = Get-Json $listLines
$modelIds = @()
foreach ($f in Get-ChildItem $targetModels -Filter '*.gguf') {
    $hit = $registry | Where-Object { $_.filename -eq $f.Name } | Select-Object -First 1
    if ($hit) { $modelIds += $hit.id }
    else { Write-Output "[bench] AVISO: $($f.Name) no aparece en el registro" }
}
Write-Output "[bench] ids de modelo: $($modelIds -join ', ')"

$fixtures = Get-ChildItem $FixturesDir -Filter '*.wav' | Sort-Object Name
$results = @()

foreach ($modelId in $modelIds) {
    foreach ($fx in $fixtures) {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $raw = & $Exe --transcribe-file $fx.FullName --model $modelId --repeat $Repeat --json 2>&1
        $sw.Stop()
        $exit = $LASTEXITCODE

        $obj = Get-Json $raw
        if ($obj -and $obj.text) {
            $results += [pscustomobject]@{
                model      = $modelId
                fixture    = $fx.BaseName
                audio_secs = $obj.audio_secs
                load_ms    = $obj.load_ms
                best_ms    = $obj.best_ms
                all_ms     = ($obj.transcribe_ms -join ',')
                rtf        = [math]::Round($obj.rtf, 3)
                backend    = $obj.bound_backend
                text       = $obj.text
                wall_ms    = $sw.ElapsedMilliseconds
                exit       = $exit
            }
            Write-Output ("[bench] {0,-26} {1,-14} best={2,6}ms rtf={3,6} backend={4,-6} -> {5}" -f `
                $modelId, $fx.BaseName, $obj.best_ms, ([math]::Round($obj.rtf,2)), $obj.bound_backend, $obj.text)
        } else {
            $tail = (($raw | Select-Object -Last 3) -join ' | ').Trim()
            $results += [pscustomobject]@{
                model = $modelId; fixture = $fx.BaseName
                text = "ERROR: $tail"; exit = $exit
            }
            Write-Output "[bench] FALLO $modelId / $($fx.Name): $tail"
        }
    }
}

$results | ConvertTo-Json -Depth 5 | Set-Content -Path $Out -Encoding UTF8
Write-Output "[bench] resultados en $Out"
