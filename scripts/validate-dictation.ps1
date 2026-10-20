<#
  validate-dictation.ps1 — arnes de validacion del dictado en tiempo real.

  Se ejecuta EN PARALELO con la prueba manual del usuario. Mide, sin
  interferir con la app:

    1. Latencia: de la soltura de la tecla de dictado hasta que se inyecta
       el texto (y hasta que el portapapeles queda restaurado).
       - La soltura de tecla se detecta con GetAsyncKeyState (10 ms).
       - Los eventos de la app se detectan TAILANDO el fichero de log: se
         marca el instante de LLEGADA de cada linea, porque el log solo
         tiene resolucion de 1 segundo.
    2. RAM y CPU del arbol completo de procesos (app + WebView2 + worker).
    3. Recuperacion del portapapeles: se deja un centinela antes de la
       prueba y se comprueba despues. NO se lee el portapapeles durante la
       prueba: leerlo podria robarle la lectura al portapapeles y hacer que
       la app restaure antes de tiempo.
    4. Conservacion del foco: se registra la ventana en primer plano antes
       y despues.
    5. Errores y avisos del log durante la prueba.

  Uso:
    powershell -File scripts/validate-dictation.ps1 [-TimeoutSeconds 120] [-Out <json>]
#>
param(
    [int]$TimeoutSeconds = 120,
    [string]$Out = "$env:TEMP\dixi-validacion.json",
    [string]$LogPath = "$env:LOCALAPPDATA\com.dixi.app\logs\dixi.log"
)

$ErrorActionPreference = 'Continue'

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Win {
    [DllImport("user32.dll")] public static extern short GetAsyncKeyState(int vKey);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@

$VK_SPACE = 0x20

function Get-ForegroundInfo {
    $h = [Win]::GetForegroundWindow()
    if ($h -eq [IntPtr]::Zero) { return @{ handle = 0; title = ''; class = ''; pid = 0 } }
    $sb = New-Object System.Text.StringBuilder 512
    [void][Win]::GetWindowTextW($h, $sb, 512)
    $cb = New-Object System.Text.StringBuilder 256
    [void][Win]::GetClassNameW($h, $cb, 256)
    $pidOut = 0
    [void][Win]::GetWindowThreadProcessId($h, [ref]$pidOut)
    $proc = Get-Process -Id $pidOut -ErrorAction SilentlyContinue
    return @{ handle = $h.ToInt64(); title = $sb.ToString(); class = $cb.ToString(); pid = $pidOut; process = $(if ($proc) { $proc.ProcessName } else { '' }) }
}

function Get-DixiTree {
    $all = Get-CimInstance Win32_Process
    $roots = @($all | Where-Object { $_.Name -eq 'dixi.exe' })
    if ($roots.Count -eq 0) { return @() }
    $ids = @($roots | ForEach-Object { $_.ProcessId })
    $tree = @($roots); $ch = $true
    while ($ch) {
        $ch = $false
        foreach ($p in $all) {
            if (($ids -contains $p.ParentProcessId) -and -not ($ids -contains $p.ProcessId)) {
                $ids += $p.ProcessId; $tree += $p; $ch = $true
            }
        }
    }
    return $tree
}

# ------------------------------------------------------------------ inicio ---
Write-Output "=== VALIDACION DE DICTADO (Dixi Fase 2A) ==="
Write-Output "log: $LogPath"
if (-not (Test-Path $LogPath)) { Write-Output "AVISO: no existe el log; se creara al arrancar la app" }

$fgBefore = Get-ForegroundInfo
Write-Output "ventana enfocada antes: '$($fgBefore.title)' [$($fgBefore.class)] ($($fgBefore.process))"

# Centinela de portapapeles: se COMPRUEBA despues, nunca durante la prueba.
$sentinel = 'DIXI-CENTINELA-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
Set-Clipboard -Value $sentinel
Write-Output "centinela de portapapeles: $sentinel"

# Posicion de lectura del log (solo las lineas nuevas).
$logPos = if (Test-Path $LogPath) { (Get-Item $LogPath).Length } else { 0 }

$events = New-Object System.Collections.ArrayList
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$spaceWasDown = $false
$keyReleasedAt = $null
$prevCpu = @{}
$samples = New-Object System.Collections.ArrayList
$fgTimeline = New-Object System.Collections.ArrayList
$lastFg = ''

Write-Output ""
Write-Output ">>> Dicta ahora. Tienes $TimeoutSeconds s. Pulsa ENTER cuando termine."
Write-Output ""

while ($sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    # --- cola de entrada: si se pulso ENTER, terminar -------------------------
    if ([Console]::KeyAvailable) {
        $k = [Console]::ReadKey($true)
        if ($k.Key -eq [ConsoleKey]::Enter) { Write-Output ">>> fin de la prueba (ENTER)"; break }
    }

    # --- deteccion de la soltura de la barra espaciadora --------------------
    $down = ([Win]::GetAsyncKeyState($VK_SPACE) -band 0x8000) -ne 0
    if ($down) { $spaceWasDown = $true }
    elseif ($spaceWasDown) {
        $spaceWasDown = $false
        if (-not $keyReleasedAt) {
            $keyReleasedAt = $sw.Elapsed.TotalSeconds
            [void]$events.Add([pscustomobject]@{ t = [math]::Round($keyReleasedAt, 3); kind = 'key_release'; line = 'soltura de la barra espaciadora' })
        }
    }

    # --- tail del log --------------------------------------------------------
    if (Test-Path $LogPath) {
        $len = (Get-Item $LogPath).Length
        if ($len -gt $logPos) {
            $fs = [System.IO.File]::Open($LogPath, 'Open', 'Read', 'ReadWrite')
            try {
                $fs.Seek($logPos, 'Begin') | Out-Null
                $sr = New-Object System.IO.StreamReader($fs)
                $text = $sr.ReadToEnd()
                $logPos = $fs.Position
            } finally { if ($sr) { $sr.Dispose() } else { $fs.Dispose() } }
            foreach ($line in ($text -split "`r?`n")) {
                if ($line.Trim().Length -eq 0) { continue }
                [void]$events.Add([pscustomobject]@{ t = [math]::Round($sw.Elapsed.TotalSeconds, 3); kind = 'log'; line = $line })
            }
        }
    }

    # --- ventana en primer plano ---------------------------------------------
    $fg = Get-ForegroundInfo
    $key = "$($fg.handle)|$($fg.title)"
    if ($key -ne $lastFg) {
        $lastFg = $key
        [void]$fgTimeline.Add([pscustomobject]@{ t = [math]::Round($sw.Elapsed.TotalSeconds, 3); title = $fg.title; class = $fg.class; process = $fg.process })
    }

    # --- muestreo del arbol de procesos --------------------------------------
    $tree = Get-DixiTree
    $now = $sw.Elapsed.TotalSeconds
    $priv = 0.0; $ws = 0.0; $cpu = 0.0; $n = 0
    $cur = @{}
    foreach ($t in $tree) {
        $gp = Get-Process -Id $t.ProcessId -ErrorAction SilentlyContinue
        if (-not $gp) { continue }
        $n++
        $priv += $gp.PrivateMemorySize64 / 1MB
        $ws += $gp.WorkingSet64 / 1MB
        $cur[$t.ProcessId] = $gp.TotalProcessorTime.TotalSeconds
        if ($prevCpu.ContainsKey($t.ProcessId)) {
            $dt = $now - $prevCpu[$t.ProcessId].T
            if ($dt -gt 0) { $cpu += ($gp.TotalProcessorTime.TotalSeconds - $prevCpu[$t.ProcessId].C) / $dt * 100 }
        }
    }
    $prevCpu = @{}
    foreach ($k in $cur.Keys) { $prevCpu[$k] = @{ C = $cur[$k]; T = $now } }
    [void]$samples.Add([pscustomobject]@{ t = [math]::Round($now, 2); privateMB = [math]::Round($priv, 1); workingMB = [math]::Round($ws, 1); cpuPct = [math]::Round($cpu, 2); processes = $n })

    Start-Sleep -Milliseconds 60
}

$fgAfter = Get-ForegroundInfo

# --- verificacion del portapapeles (FUERA de la ventana de la prueba) -------
$clipAfter = $null
try { $clipAfter = Get-Clipboard -Raw } catch { $clipAfter = "<no se pudo leer>" }
$clipRestored = ($clipAfter -ne $null) -and ($clipAfter.Trim() -eq $sentinel)

# --- metricas desde los eventos ---------------------------------------------
function Find-Event($pattern) {
    foreach ($e in $events) { if ($e.kind -eq 'log' -and $e.line -match $pattern) { return $e } }
    return $null
}

$captureEnd = Find-Event 'Microphone stream stopped'
$asrEnd    = Find-Event 'Transcription completed in'
$publish   = Find-Event '\[reliable-paste\] published transcript'
$chord     = Find-Event '\[reliable-paste\] paste chord sent'
$restore   = Find-Event '\[reliable-paste\] restored previous clipboard'
$chordFail = Find-Event 'paste chord failed|chord injection failed'
$emptyTx   = Find-Event 'Transcription result is empty'
$noRead    = Find-Event 'no read within timeout'
$ownership = Find-Event 'clipboard ownership lost'

$asrSeconds = $null
if ($asrEnd -and $asrEnd.line -match 'completed in ([0-9.]+)s') { $asrSeconds = [double]$matches[1] }

Write-Output ""
Write-Output "================= RESULTADO ================="
Write-Output ""
Write-Output "--- LATENCIA (ms) ---"
function Diff($a, $b) { if ($a -and $b) { [math]::Round(($b.t - $a.t) * 1000) } else { $null } }
$latChord = if ($captureEnd -and $chord) { Diff $captureEnd $chord } else { $null }
$latRest  = if ($captureEnd -and $restore) { Diff $captureEnd $restore } else { $null }
$latKey   = if ($keyReleasedAt -and $chord) { [math]::Round(($chord.t - $keyReleasedAt) * 1000) } else { $null }

"  fin de captura (log)        : {0}" -f $(if ($captureEnd) { "$([math]::Round($captureEnd.t,3)) s" } else { 'NO DETECTADO' })
"  ASR completado (log)        : {0}" -f $(if ($asrEnd) { "$([math]::Round($asrEnd.t,3)) s" } else { 'NO DETECTADO' })
"  texto publicado (log)       : {0}" -f $(if ($publish) { "$([math]::Round($publish.t,3)) s" } else { 'NO DETECTADO' })
"  ctrl+V inyectado (log)      : {0}" -f $(if ($chord) { "$([math]::Round($chord.t,3)) s" } else { 'NO DETECTADO' })
"  portapapeles restaurado     : {0}" -f $(if ($restore) { "$([math]::Round($restore.t,3)) s" } else { 'NO DETECTADO' })
""
"  >> captura_fin -> ctrl+V    : {0} ms" -f $(if ($latChord -ne $null) { $latChord } else { 'n/d' })
"  >> captura_fin -> restaurado: {0} ms" -f $(if ($latRest -ne $null) { $latRest } else { 'n/d' })
"  >> soltura_tecla -> ctrl+V  : {0} ms" -f $(if ($latKey -ne $null) { $latKey } else { 'n/d' })
"  >> duracion del ASR (log)   : {0} ms" -f $(if ($asrSeconds -ne $null) { [math]::Round($asrSeconds * 1000) } else { 'n/d' })

Write-Output ""
Write-Output "--- PORTAPAPELES ---"
"  centinela esperado : $sentinel"
"  valor tras la prueba: '$($clipAfter -replace "`r?`n", ' ')'"
"  >> RECUPERADO: {0}" -f $(if ($clipRestored) { 'SI' } else { 'NO' })

Write-Output ""
Write-Output "--- FOCO ---"
"  antes : '$($fgBefore.title)' [$($fgBefore.class)] ($($fgBefore.process))"
"  despues: '$($fgAfter.title)' [$($fgAfter.class)] ($($fgAfter.process))"
"  >> FOCO CONSERVADO: {0}" -f $(if ($fgAfter.handle -eq $fgBefore.handle) { 'SI' } else { 'NO (cambio de ventana)' })
Write-Output "  linea de tiempo del foco:"
foreach ($f in $fgTimeline) { "    t={0,7:N3}s  {1,-42} [{2}] ({3})" -f $f.t, $f.title, $f.class, $f.process }

Write-Output ""
Write-Output "--- RECURSOS (arbol completo) ---"
if ($samples.Count) {
    $s = $samples
    "  RAM privada : max {0:N1} MB | media {1:N1} MB" -f (($s | Measure-Object privateMB -Maximum).Maximum), (($s | Measure-Object privateMB -Average).Average)
    "  working set : max {0:N1} MB | media {1:N1} MB" -f (($s | Measure-Object workingMB -Maximum).Maximum), (($s | Measure-Object workingMB -Average).Average)
    "  CPU         : max {0:N1} %  | media {1:N1} %" -f (($s | Measure-Object cpuPct -Maximum).Maximum), (($s | Measure-Object cpuPct -Average).Average)
    "  procesos    : max {0}" -f (($s | Measure-Object processes -Maximum).Maximum)
}

Write-Output ""
Write-Output "--- INCIDENCIAS DETECTADAS EN EL LOG ---"
$issues = @()
if ($emptyTx)   { $issues += 'transcripcion vacia' }
if ($chordFail) { $issues += 'fallo al inyectar el atajo de pegado' }
if ($noRead)    { $issues += 'el destino no leyo el portapapeles a tiempo' }
if ($ownership) { $issues += 'el portapapeles cambio de propietario' }
if (-not $restore -and $chord) { $issues += 'el portapapeles no se restauro' }
$errs = $events | Where-Object { $_.kind -eq 'log' -and $_.line -match '\[(ERROR|WARN)\]' }
foreach ($e in $errs) { Write-Output "  [log] $($e.line)" }
if ($issues.Count) { foreach ($i in $issues) { Write-Output "  ! $i" } } else { Write-Output "  ninguna" }

$report = [ordered]@{
    started = (Get-Date).ToString('o')
    clipboard_sentinel = $sentinel
    clipboard_restored = $clipRestored
    clipboard_after = $clipAfter
    focus_before = $fgBefore
    focus_after = $fgAfter
    focus_preserved = ($fgAfter.handle -eq $fgBefore.handle)
    focus_timeline = $fgTimeline
    latency_capture_to_chord_ms = $latChord
    latency_capture_to_restore_ms = $latRest
    latency_keyrelease_to_chord_ms = $latKey
    asr_ms = $(if ($asrSeconds -ne $null) { [math]::Round($asrSeconds * 1000) } else { $null })
    ram_private_max_mb = $(if ($samples.Count) { [math]::Round((($samples | Measure-Object privateMB -Maximum).Maximum), 1) } else { $null })
    ram_private_avg_mb = $(if ($samples.Count) { [math]::Round((($samples | Measure-Object privateMB -Average).Average), 1) } else { $null })
    working_set_max_mb = $(if ($samples.Count) { [math]::Round((($samples | Measure-Object workingMB -Maximum).Maximum), 1) } else { $null })
    cpu_max_pct = $(if ($samples.Count) { [math]::Round((($samples | Measure-Object cpuPct -Maximum).Maximum), 2) } else { $null })
    cpu_avg_pct = $(if ($samples.Count) { [math]::Round((($samples | Measure-Object cpuPct -Average).Average), 2) } else { $null })
    issues = $issues
    log_errors = ($errs | ForEach-Object { $_.line })
    events = $events
    samples = $samples
}
$report | ConvertTo-Json -Depth 8 | Out-File -FilePath $Out -Encoding utf8
Write-Output ""
Write-Output "[validacion] informe en $Out"
