<#
  sample-engine-usage.ps1 - muestrea el uso del arbol de procesos de dixi.exe
  mientras corre el banco.

  QUE REGISTRA
  ------------
  Por cada muestra (cada 500 ms):
    - la RAM resident summed de todos los dixi.exe (la app, el worker del
      motor y sus hijos),
    - el tiempo de CPU acumulado de esos mismos procesos, para poder
      calcular el consumo por intervalo.

  POR QUE HAY QUE MIRAR EL ARBOL Y NO EL PROCESO VISIBLE
  -------------------------------------------------------
  El ASR corre en un proceso hijo (el worker del motor). Medir solo la app
  daria una foto de la memoria sin el modelo cargado.

  Uso:
    .\sample-engine-usage.ps1 -Seconds 900 -OutFile <csv>
#>
param(
    [int]$Seconds = 900,
    [string]$OutFile = "D:\Codigo\Dixi\.logs\engine-usage.csv",
    [int]$IntervalMs = 500
)

$ErrorActionPreference = 'Stop'

$rows = @()
$prev = $null
$fin = (Get-Date).AddSeconds($Seconds)

while ((Get-Date) -lt $fin) {
    $procs = @(Get-CimInstance Win32_Process -Filter "Name='dixi.exe'" -ErrorAction SilentlyContinue)
    if ($procs.Count -gt 0) {
        $ram = 0.0
        $cpu100ns = [uint64]0
        foreach ($p in $procs) {
            $ram += [double]$p.WorkingSetSize / 1MB
            # KernelModeTime y UserModeTime vienen en unidades de 100 ns.
            $cpu100ns += [uint64]$p.KernelModeTime + [uint64]$p.UserModeTime
        }
        $cpuPct = $null
        if ($null -ne $prev) {
            $delta = $cpu100ns - $prev
            if ($delta -ge 0) {
                # delta en 100 ns -> segundos de CPU; sobre el intervalo real.
                $cpuSeconds = $delta / 10000000
                $cpuPct = [math]::Round(($cpuSeconds / ($IntervalMs / 1000.0)) * 100, 1)
            }
        }
        $prev = $cpu100ns
        $rows += [pscustomobject]@{
            t           = (Get-Date).ToString('HH:mm:ss')
            procesos    = $procs.Count
            ram_mb      = [math]::Round($ram, 1)
            cpu_pct     = $cpuPct
            cpu_acum_s  = [math]::Round($cpu100ns / 10000000, 2)
        }
    } else {
        # Sin procesos no hay delta: el contador se reinicia para que el
        # siguiente intervalo no arrastre el hueco.
        $prev = $null
    }
    Start-Sleep -Milliseconds $IntervalMs
}

if ($rows.Count -gt 0) {
    $rows | Export-Csv -Path $OutFile -NoTypeInformation -Encoding UTF8
    $ram = @($rows | ForEach-Object { [double]$_.ram_mb })
    $cpu = @($rows | Where-Object { $null -ne $_.cpu_pct } | ForEach-Object { [double]$_.cpu_pct })
    Write-Host ("Muestreo: {0} muestras" -f $rows.Count)
    Write-Host ("  RAM  min={0} MB  mediana={1} MB  max={2} MB" -f ($ram | Measure-Object -Minimum).Minimum, ($ram | Sort-Object)[[int]($ram.Count/2)], ($ram | Measure-Object -Maximum).Maximum)
    if ($cpu.Count -gt 0) {
        Write-Host ("  CPU  media={0} %  max={1} %  (sobre {2} nucleos logicos)" -f [math]::Round((($cpu | Measure-Object -Average).Average),1), ($cpu | Measure-Object -Maximum).Maximum, (Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty NumberOfLogicalProcessors))
    }
} else {
    Write-Host "Muestreo: no se vio ningun dixi.exe"
}
