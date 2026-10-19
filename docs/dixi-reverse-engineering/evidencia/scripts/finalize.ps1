$ErrorActionPreference = 'Stop'
$docs = 'D:\Codigo\Dixi\docs\dixi-reverse-engineering'
$ev   = Join-Path $docs 'evidencia'
$tmp  = 'D:\Codigo\Dixi\.tmp-audit'
$utf8 = New-Object System.Text.UTF8Encoding($false)

# 1) preservar el toolkit de auditoria como evidencia reproducible
$sc = Join-Path $ev 'scripts'
New-Item -ItemType Directory -Path $sc -Force | Out-Null
Get-ChildItem -LiteralPath $tmp -File -Include '*.ps1','*.py','*.bat' -ErrorAction SilentlyContinue | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $sc $_.Name) -Force
}
Write-Output ("scripts preservados: {0}" -f (Get-ChildItem -LiteralPath $sc -File).Count)

# 2) reescribir referencias a la ruta de evidencia
$repl = @(
  @('D:\Codigo\Dixi\.tmp-audit\research-handy-openwhispr.md', 'docs/dixi-reverse-engineering/evidencia/research-handy-openwhispr.md'),
  @('D:\Codigo\Dixi\.tmp-audit\research-minimax.md',          'docs/dixi-reverse-engineering/evidencia/research-minimax.md'),
  @('D:\Codigo\Dixi\.tmp-audit\research-wispr-docs.md',        'docs/dixi-reverse-engineering/evidencia/research-wispr-docs.md'),
  @('.tmp-audit\research-handy-openwhispr.md', 'evidencia/research-handy-openwhispr.md'),
  @('.tmp-audit\research-minimax.md',          'evidencia/research-minimax.md'),
  @('.tmp-audit\research-wispr-docs.md',        'evidencia/research-wispr-docs.md'),
  @('`D:\Codigo\Dixi\.tmp-audit\research-handy-openwhispr.md` (83 KB', '`evidencia/research-handy-openwhispr.md` (81 KB'),
  @('copiando la BD a `.tmp-audit/db/` y leyendo el esquema con `python3.13` + `sqlite3` en modo lectura.',
    'copiando la BD a un directorio temporal **fuera de la instalación** (copia eliminada al terminar) y leyendo el esquema con `python3.13` + `sqlite3` en modo lectura.'),
  @('el `ax-inspect` se ejecutó **sólo en modo de lectura estructural** sobre una **copia** de la BD en `.tmp-audit/db/`.',
    'el `ax-inspect` se ejecutó **sólo en modo de lectura estructural** sobre una **copia temporal** de la BD (eliminada al terminar).')
)
$changed = 0
Get-ChildItem -LiteralPath $docs -Filter '*.md' -File | ForEach-Object {
  $p = $_.FullName
  $t = [System.IO.File]::ReadAllText($p, [System.Text.Encoding]::UTF8)
  $o = $t
  foreach ($r in $repl) { $t = $t.Replace($r[0], $r[1]) }
  if ($t -ne $o) {
    [System.IO.File]::WriteAllText($p, $t, $utf8)
    $changed++
    Write-Output ("  actualizado: {0}" -f $_.Name)
  }
}
Write-Output ("documentos con referencias corregidas: {0}" -f $changed)

# 3) eliminar el directorio temporal
$restante = Get-ChildItem -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
Write-Output ("restos en .tmp-audit: {0}" -f $restante.Count)
