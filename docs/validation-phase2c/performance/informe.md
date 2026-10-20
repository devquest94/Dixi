# Consumo de memoria y CPU de Dixi (Fase 2C)

Medido con `scripts/measure-dixi-tree.ps1` sobre una instancia **aislada**
(carpeta portable propia, con los modelos compartidos por junction). La
instalación real del usuario no se tocó.

- Duración de cada muestra: 15 s, cada 250 ms.
- **RAM privada**: lo que el proceso reserva de verdad, con la media de la
  muestra.
- **Working set**: lo que ocupa en RAM, incluyendo páginas compartidas con
  el runtime de WebView2, que es un componente del sistema compartido.
- **CPU**: porcentaje medio del árbol de procesos durante la muestra
  (100 % = un núcleo saturado).

## Las cinco condiciones controladas

| Condición | 2B privada | 2C privada | Δ | 2B ws | 2C ws | 2B CPU | 2C CPU | Procesos |
|---|---|---|---|---|---|---|---|---|
| Ventana abierta | 249,6 MB | 263,3 MB | +13,7 MB | 424,5 MB | 432,0 MB | 0,40 % | 0,10 % | 8 |
| Aplicación únicamente en bandeja | 293,6 MB | 258,4 MB | -35,2 MB | 463,7 MB | 424,3 MB | 0,26 % | 2,62 % | 8 |
| Modelo ASR descargado de memoria | 286,6 MB | 244,9 MB | -41,7 MB | 469,1 MB | 424,6 MB | 1,15 % | 0,00 % | 8 |
| Modelo cargado | 243,4 MB | 248,8 MB | +5,4 MB | 422,5 MB | 430,5 MB | 1,01 % | 0,31 % | 8 |
| Transcripción activa (GUI) | 268,5 MB | 260,4 MB | -8,2 MB | 500,6 MB | 471,2 MB | 67,03 % | 33,89 % | 14 |

## El motor, con el modelo cargado

La condición que de verdad aísla el motor es la transcripción por fichero,
sin interfaz. El micrófono de esta máquina no capta señal utilizable, así que
el dictado no llega a transcribir y el modelo no se carga: en las cinco
condiciones de arriba el motor nunca llega a estar en memoria.

En esta condición la media depende de dónde cae la ventana de medida, así
que el dato comparable es el **pico**, que es el estado con el modelo
cargado y transcribiendo:

| Magnitud | 2B | 2C | Δ |
|---|---|---|---|
| RAM privada (pico) | 506,3 MB | 505,7 MB | -0,6 MB |
| Working set (pico) | 176,9 MB | 176,6 MB | -0,3 MB |
| CPU (pico) | 730,45 % | 615,87 % | — |
| Procesos en el pico | 3 | 2 | — |

## Lectura

1. **Lo que se añadió en la 2C no se ve en el consumo.** El pico del motor
   con el modelo cargado es **506,3 MB en 2B y 505,7 MB en 2C**: 0,6 MB de
   diferencia, por debajo del ruido de la muestra. El motor de ASR no se
   tocó en esta fase, y la almacén compartido, los perfiles, los fragmentos,
   los transforms, el historial y los cuatro paneles nuevos no mueven la
   aguja: lo que pesa sigue siendo el runtime de WebView2.

2. **Las diferencias de las cinco condiciones son ruido, no una regresión.**
   Los signos se alternan: ventana abierta +13,7 MB, solo bandeja −35,2 MB,
   modelo descargado −41,7 MB, modelo cargado +5,4 MB, transcripción activa
   −8,2 MB. Un coste real de las funcionalidades nuevas se vería como una
   diferencia positiva y constante en todas. El informe de la 2B ya había
   medido que la variación entre ejecuciones de esta máquina es mayor que
   estas diferencias.

3. **La CPU no es una mejora.** Durante la transcripción, la 2C midió la
   mitad de CPU que la 2B (33,89 % frente a 67,03 %). Eso no es una mejora
   del producto: el porcentaje de CPU depende de la carga de la máquina en
   ese momento y de cuánto de la ventana de medida coincidió con la
   transcripción. El pico del motor también sale menor en la 2C, y el
   código del motor es idéntico. Medirlo otra vez daría otros números.

## Limitaciones

- El micrófono de esta máquina no capta señal utilizable, así que las
  condiciones «modelo cargado» y «modelo descargado» de la tabla no llegan
  a cargar el modelo: el dictado no se transcribe. Solo la transcripción
  por fichero aísla el motor de verdad.
- Para esa condición hubo que **remuestrear el audio de referencia** de
  22,05 kHz a 16 kHz (el motor exige WAV PCM 16 bits mono a 16 kHz) y pasar
  el modelo explícitamente con `--model`, porque la ruta headless no lee
  el modelo seleccionado de los ajustes de la instancia aislada.
- El binario de la 2B ya no está en disco, así que no se pudo volver a
  medir con él: la comparación usa las muestras que la fase 2B dejó
  committeadas, producidas con el mismo script y los mismos parámetros.

## Repetición

```
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-phase2b.ps1 `
    -OutDir docs/validation-phase2c/performance `
    -InstanceDir "$env:TEMP/dixi-phase2c-instance"
```

El script deja la instancia aislada en la carpeta temporal y las muestras en
este mismo directorio. El desglose por proceso (app principal, worker de
ASR, runtime de WebView2) está en los ficheros `.txt`.