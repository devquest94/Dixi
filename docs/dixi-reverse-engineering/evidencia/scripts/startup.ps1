$ErrorActionPreference = 'Continue'
$app = 'C:\Users\DanielSan\AppData\Local\WisprFlow\app-1.6.1174\Wispr Flow.exe'
$prof = 'D:\Codigo\Dixi\.tmp-audit\isolated-profile2'
$port = 9334

if (Test-Path $prof) { Remove-Item -LiteralPath $prof -Recurse -Force }
New-Item -ItemType Directory -Path $prof | Out-Null

$before = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }).ProcessId

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath $app -ArgumentList @("--user-data-dir=$prof", "--remote-debugging-port=$port") -PassThru

$t_port = $null
$t_page = $null
$t_list = $null
for ($i = 0; $i -lt 120; $i++) {
  Start-Sleep -Milliseconds 250
  $el = $sw.ElapsedMilliseconds
  if ($null -eq $t_port) {
    $c = Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue
    if ($c) { $t_port = $el }
  }
  if ($null -eq $t_list) {
    try {
      $r = Invoke-WebRequest -Uri "http://127.0.0.1:$port/json/list" -UseBasicParsing -TimeoutSec 2
      if ($r.StatusCode -eq 200 -and $r.Content -match 'title') { $t_list = $el }
    } catch { }
  }
  if ($null -ne $t_list) { break }
}

Write-Output "=== ARRANQUE EN FRIO (instancia aislada, perfil nuevo) ==="
if ($null -ne $t_port) { Write-Output ("  puerto CDP escuchando        : {0} ms" -f $t_port) } else { Write-Output "  puerto CDP: NO" }
if ($null -ne $t_list) { Write-Output ("  /json/list respondio (listo)  : {0} ms" -f $t_list) } else { Write-Output "  /json/list: NO responde" }
Write-Output ("  total elapsed               : {0} ms" -f $sw.ElapsedMilliseconds)

$after = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' })
$new = @($after | Where-Object { $before -notcontains $_.ProcessId })
Write-Output ("  procesos nuevos: {0} -> {1}" -f $new.Count, (($new | ForEach-Object { $_.ProcessId }) -join ','))

# top targets
try {
  $r = Invoke-WebRequest -Uri "http://127.0.0.1:$port/json/list" -UseBasicParsing -TimeoutSec 5
  $j = $r.Content | ConvertFrom-Json
  Write-Output ""
  Write-Output "=== TARGETS CDP (renderers listos) ==="
  foreach ($t in $j) { Write-Output ("  {0,-16} {1}" -f $t.title, ($t.url -replace '.*renderer\\', '')) }
} catch { Write-Output "  (no se pudo leer /json/list)" }

Start-Sleep -Seconds 3
Write-Output ""
Write-Output "=== MEMORIA TRAS 3 s DE ARRANQUE ==="
$tot = 0; $pmt = 0
foreach ($x in $after) {
  try {
    $pr = Get-Process -Id $x.ProcessId -ErrorAction Stop
    $tot = $tot + $pr.WorkingSet64
    $pmt = $pmt + $pr.PrivateMemorySize64
  } catch { }
}
Write-Output ("  working set total (todas las instancias): {0} MB" -f [math]::Round($tot/1MB,1))
Write-Output ("  private memory total: {0} MB" -f [math]::Round($pmt/1MB,1))

# cleanup
$newIds = @($new | ForEach-Object { $_.ProcessId })
$newHelper = @($after | Where-Object { $_.Name -like '*Helper*' -and $newIds -contains $_.ParentProcessId })
foreach ($x in @($new + $newHelper)) {
  try { Stop-Process -Id $x.ProcessId -Force -ErrorAction Stop } catch { }
}
Start-Sleep -Seconds 2
$alive = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' })
Write-Output ""
Write-Output "=== VERIFICACION POST-LIMPIEZA ==="
Write-Output ("  instancias vivas: {0} -> PIDs {1}" -f $alive.Count, (($alive | ForEach-Object { $_.ProcessId }) -join ','))
Write-Output ("  original 4484 vivo: {0}" -f ([bool](@($alive.ProcessId) -contains 4484)))
Write-Output ("  puerto {0} escuchando: {1}" -f $port, [bool](Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue))
Remove-Item -LiteralPath $prof -Recurse -Force -ErrorAction SilentlyContinue
Write-Output "  perfil temporal eliminado: $(-not (Test-Path $prof))"
