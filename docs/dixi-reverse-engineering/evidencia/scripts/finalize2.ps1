$ErrorActionPreference = 'Stop'
$docs = 'D:\Codigo\Dixi\docs\dixi-reverse-engineering'
$ev   = Join-Path $docs 'evidencia'
$sc   = Join-Path $ev 'scripts'
$tmp  = 'D:\Codigo\Dixi\.tmp-audit'
$utf8 = New-Object System.Text.UTF8Encoding($false)

# --- 1)Pn PII en las capturas crudas antes de publicarlas ---
Write-Output "=== chequeo de PII en capturas crudas ==="
$cfg = ([System.IO.File]::ReadAllText('C:\Users\DanielSan\AppData\Roaming\Wispr Flow\config.json', [System.Text.Encoding]::UTF8) | ConvertFrom-Json)
$u = $cfg.prefs.user
$needles = @($u.email, $u.firstName, $u.lastName, $u.uuid, $u.globalProfileSharingUserId, $cfg.prefs.sessionId, $cfg.prefs.anonymousId, $cfg.prefs.deviceId) | Where-Object { $_ }
foreach ($f in @('uia-main.txt','after-esc.uia.txt','uia-tree.txt','isolated.log')) {
  $p = Join-Path $tmp $f
  if (-not (Test-Path -LiteralPath $p)) { continue }
  $t = [System.IO.File]::ReadAllText($p, [System.Text.Encoding]::UTF8)
  $hits = @()
  foreach ($n in $needles) { if ($t -like ('*' + $n + '*')) { $hits += $n } }
  if ($hits.Count -gt 0) {
    Write-Output ("  {0}: {1} coincidencia(s) -> REDACTAR" -f $f, $hits.Count)
    foreach ($n in $hits) { $t = $t.Replace($n, '<REDACTADO>') }
    [System.IO.File]::WriteAllText($p, $t, $utf8)
  } else {
    Write-Output ("  {0}: limpio" -f $f)
  }
}

# --- 2) separar toolkit de capturas ---
Get-ChildItem -LiteralPath $sc -File | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force }
Get-ChildItem -LiteralPath $tmp -File | Where-Object { $_.Extension -in @('.ps1','.py','.bat') } | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $sc $_.Name) -Force
}
foreach ($f in @('uia-main.txt','after-esc.uia.txt','uia-tree.txt','isolated.log','rea-status.json')) {
  $p = Join-Path $tmp $f
  if (Test-Path -LiteralPath $p) { Copy-Item -LiteralPath $p -Destination (Join-Path $ev "captura-$f") -Force }
}
Write-Output ""
Write-Output "=== evidencia/scripts (toolkit) ==="
Get-ChildItem -LiteralPath $sc -File | ForEach-Object { Write-Output ("  {0}" -f $_.Name) }
Write-Output ""
Write-Output "=== evidencia (raiz) ==="
Get-ChildItem -LiteralPath $ev -File | ForEach-Object { Write-Output ("  {0,-40} {1,8} KB" -f $_.Name, [math]::Round($_.Length/1KB,1)) }
