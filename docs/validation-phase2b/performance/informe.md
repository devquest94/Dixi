# Consumo de memoria y CPU de Dixi (Fase 2B)

Medido con `scripts/measure-dixi-tree.ps1` sobre una instancia **aislada**
(carpeta portable propia, con los modelos compartidos por junction). La
instalación real del usuario no se tocó.

- Duración de cada muestra: 12 s, cada 250 ms.
- **RAM privada**: lo que el proceso reserva de verdad.
- **Working set**: lo que ocupa en RAM, incluyendo páginas compartidas con el
  runtime de WebView2, que es un componente del sistema compartido.
- **CPU**: porcentaje medio del árbol de procesos durante la muestra.

## Resultados

| Condición | RAM privada (MB) | Working set (MB) | CPU (%) | Procesos |
|---|---|---|---|---|
| Ventana abierta (binario final) | 249,6 | 424,5 | 0,40 | 8 |
| Ventana abierta | 256,0 | 428,6 | 0,25 | 8 |
| Aplicación únicamente en bandeja | 293,6 | 463,7 | 0,26 | 9 |
| Modelo ASR descargado de memoria | 286,6 | 469,1 | 1,15 | 9 |
| Modelo cargado | 281,5 | 465,6 | 0,12 | 9 |
| Modelo cargado (sin descarga automática) | 243,4 | 422,5 | 1,01 | 8 |
| Transcripción activa (GUI) | 310,6 | 549,7 | 71,72 | 17 |
| Transcripción activa (sin descarga automática) | 268,5 | 500,6 | 67,03 | 16 |
| Transcripción por fichero (sin interfaz) | 100,1 | 38,0 | 129,03 | 3 |

El desglose por proceso (app principal, worker de ASR, runtime de WebView2)
está en los ficheros `.txt` de este mismo directorio.

## Lectura

1. **El runtime de WebView2 cuesta más que el motor de ASR.** La aplicación
   con la ventana abierta reserva unos 250 MB repartidos entre 8 procesos (la
   app y siete del runtime de WebView2). La misma transcripción **sin
   interfaz** —la ruta por lotes de `--transcribe-file`— reserva 100,1 MB y
   38,0 MB de working set con solo 3 procesos. El motor de reconocimiento no
   es lo que pesa: lo que pesa es el navegador incrustado.

2. **La transcripción activa multiplica la CPU y los procesos.** De 8 a 17
   procesos y ~72 % de CPU con la interfaz; el headless llega a 129 % de CPU
   porque el motor usa varios núcleos sin la carga de la interfaz.

3. **La ventana y la bandeja apenas se distinguen.** ~250 MB con la ventana
   abierta frente a 293,6 MB solo en la bandeja: la diferencia está dentro
   del ruido de la muestra, y en ambos casos el proceso de WebView2 está
   vivo. Ocultar la ventana **no** libera memoria de forma apreciable.

4. **No se pudo aislar de forma fiable el peso del modelo en reposo.** Con
   la descarga automática a 15 s y con la descarga desactivada, los números
   de "modelo cargado" y "modelo descargado" se solapan (281,5 / 286,6 MB y
   243,4 MB con el modelo retenido). La causa es que el micrófono de esta
   máquina no capta señal utilizable, así que el dictado no llega a
   transcribir y el modelo no se carga; la única condición en la que el
   motor carga de verdad es la transcripción por fichero, y ahí el delta
   respecto al reposo de la interfaz es de unos 100 MB de RAM privada.

## Limitación

Las condiciones "modelo cargado" y "modelo descargado" se midieron sobre
un dictado real, pero **sin voz**: el micrófono de la máquina no capta señal
utilizable y la transcripción no llega a completarse, de modo que el modelo
no residía en memoria. Para medirlas con aislamiento haría falta grabar
audio y procesarlo, que es exactamente lo que hace la condición de
transcripción por fichero.

## Repetición

```
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-phase2b.ps1
```

El script deja la instancia aislada en `%TEMP%\dixi-phase2b-instance` y los
ficheros de muestra en este directorio. Con `-Conditions` se mide un
subconjunto, y con `-ModelUnloadTimeout` se cambia la descarga del modelo.
