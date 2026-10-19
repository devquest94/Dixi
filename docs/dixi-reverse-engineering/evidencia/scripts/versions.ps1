$ErrorActionPreference = 'Continue'
$app = 'C:\Users\DanielSan\AppData\Local\WisprFlow\app-1.6.1174'
$files = @(
  "$app\Wispr Flow.exe",
  "$app\resources\Release\Wispr Flow Helper.exe",
  'C:\Users\DanielSan\AppData\Local\WisprFlow\Wispr Flow.exe',
  'C:\Users\DanielSan\AppData\Local\WisprFlow\Update.exe'
)
foreach ($f in $files) {
  if (Test-Path -LiteralPath $f) {
    $vi = (Get-Item -LiteralPath $f).VersionInfo
    Write-Output "--- $f"
    Write-Output ("    ProductName    = {0}" -f $vi.ProductName)
    Write-Output ("    ProductVersion = {0}" -f $vi.ProductVersion)
    Write-Output ("    FileVersion    = {0}" -f $vi.FileVersion)
    Write-Output ("    CompanyName    = {0}" -f $vi.CompanyName)
    Write-Output ("    FileDescription= {0}" -f $vi.FileDescription)
  }
}

Write-Output ""
Write-Output "=== locales .pak count (i18n coverage) ==="
$n = (Get-ChildItem -LiteralPath "$app\locales" -Filter *.pak -ErrorAction SilentlyContinue | Measure-Object).Count
Write-Output "  $n locale packs"

Write-Output ""
Write-Output "=== media / GPU stack ==="
foreach ($n2 in @('ffmpeg.dll','dxcompiler.dll','libGLESv2.dll','vk_swiftshader.dll','icudtl.dat')) {
  $p = Join-Path $app $n2
  if (Test-Path -LiteralPath $p) {
    $it = Get-Item -LiteralPath $p
    $vi = $it.VersionInfo
    Write-Output ("  {0,-22} {1,8} KB  ver={2}" -f $n2, [math]::Round($it.Length/1KB,0), $vi.FileVersion)
  }
}

Write-Output ""
Write-Output "=== .sentry-native ==="
Get-ChildItem -LiteralPath "$app\.sentry-native" -Force -ErrorAction SilentlyContinue |
  ForEach-Object { Write-Output ("  {0,-34} {1,8} KB" -f $_.Name, [math]::Round($_.Length/1KB,1)) }

Write-Output ""
Write-Output "=== roaming sentry ==="
Get-ChildItem -LiteralPath 'C:\Users\DanielSan\AppData\Roaming\Wispr Flow\sentry' -Recurse -Force -ErrorAction SilentlyContinue |
  Select-Object -First 12 | ForEach-Object { Write-Output ("  {0,-46} {1,8} KB" -f $_.Name, [math]::Round($_.Length/1KB,1)) }

Write-Output ""
Write-Output "=== ISOLATED INSTANCE STATE ==="
$w = Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like '*WisprFlow*' }
Write-Output ("  total wispr processes: {0}" -f ($w | Measure-Object).Count)
foreach ($p in $w) {
  $type = 'main'
  if ($p.CommandLine -match '--type=([a-z-]+)') { $type = $matches[1] }
  $ud = ''
  if ($p.CommandLine -match '--user-data-dir="?([^"\s]+)"?') { $ud = $matches[1] }
  Write-Output ("  PID {0,-6} {1,-22} {2,-8} ud={3}" -f $p.ProcessId, $p.Name, $type, $ud)
}
Write-Output ""
Write-Output "  --- port 9333 ---"
$c = Get-NetTCPConnection -State Listen -LocalPort 9333 -ErrorAction SilentlyContinue
if ($c) {
  foreach ($x in $c) { Write-Output ("  LISTEN {0}:{1} pid={2}" -f $x.LocalAddress, $x.LocalPort, $x.OwningProcess) }
} else {
  Write-Output "  NOT LISTENING"
}
Write-Output ""
Write-Output "  --- isolated.log ---"
if (Test-Path 'D:\Codigo\Dixi\.tmp-audit\isolated.log') {
  Get-Content 'D:\Codigo\Dixi\.tmp-audit\isolated.log' -TotalCount 25 | ForEach-Object { Write-Output "  $_" }
} else { Write-Output "  (none)" }
