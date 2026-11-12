<#
  cpu-load.ps1 - carga controlada para las mediciones de estabilidad.

  Ocupa N nucleos con un bucle acotado. Cada proceso ocupa UN nucleo: el
  consumo de CPU se mide por tiempo de proceso, no por numero de hilos.

  Se lanza en segundo plano y SIN ventana visible (CreateNoWindow), para no
  interrumpir al usuario ni robarle el foco.

  Uso:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/cpu-load.ps1 `
        -Nucleos 1 -Segundos 90
#>
param(
    # Nucleos a ocupar. Cada instancia ocupa uno.
    [int]$Nucleos = 1,
    [int]$Segundos = 90
)

$ErrorActionPreference = 'Stop'
$fin = (Get-Date).AddSeconds($Segundos)
$h = 0
while ((Get-Date) -lt $fin) {
    $h = ($h * 31 + 7) % 2147483647
}
