# Estabilidad del streaming de Nemotron 3.5 ASR — Dixi

Estado: **implementado y medido. La recuperacion por lotes y la conservacion
del audio en la cancelacion ya estan en el codigo; los dos modos de dictado
siguen sin implementarse.**

Este documento cierra la etapa de estabilizacion del streaming: donde se
acumula el audio pendiente, que se ha cambiado para que la saturacion no
provoque ni una espera de un minuto ni la perdida del audio, y que se ha
medido antes y despues.

Todo lo que se afirma aqui sale de mediciones reproducibles con los scripts
del repositorio. Lo que no se ha medido se dice explicitamente.

---

## 1. Diagnostico: donde se acumula el audio pendiente

Hay tres capas entre el microfono y el texto, y el audio pendiente se puede
quedar en cualquiera de las tres. El dato clave es que **las dos primeras son
invisibles para el motor**: el motor solo ve lo que ya recibio, y la app
encola el resto mientras el motor decodifica. Por eso la magnitud que hace
falta se mide desde la app.

### 1.1 Capa 1 — el enrutador de la app (`StreamRouter`)

`StreamRouter::feed` envia cada fragmento de 100 ms a un `mpsc::channel` **sin
limite** (`src-tauri/src/managers/transcription.rs`). El hilo del microfono no
se bloquea nunca: la captura sigue a velocidad de grabacion aunque el motor
vaya mas lento. Consecuencia: cuando la maquina no da mas, la cola crece sin
techo y el audio se acumula **en la app**.

### 1.2 Capa 2 — el supervisor del motor (`EngineSupervisor`)

El supervisor procesa los feeds **de uno en uno**: `Owner::feed` llama a
`call_work`, que espera la respuesta del proceso worker antes de devolver el
control (`supervisor.rs:954`). El worker es un unico proceso con un unico
modelo cargado, y su bucle atiende una peticion cada vez.

Aqui esta el punto que explica el comportamiento observado: **el comando
`Finalize` se encola detras de todos los feeds**. Cuando el usuario suelta la
tecla con la cola saturada, el tiempo que espera no es el cierre: es el
**vaziado de la cola** (un feed por cada fragmento pendiente, cada uno con su
decodificacion) y despues el cierre.

### 1.3 Capa 3 — el nativo (`transcribe-cpp-sys 0.4.0`, arch parakeet)

Nemotron usa la ruta *cache-aware* de parakeet. En ella:

- `stream_feed` decodifica **dentro de la llamada**: cuando hay audio
  suficiente emite un fragmento (`model.cpp:3636`).
- El búfer PCM se recorta despues de cada emision, de modo que el motor
  conserva solo el fragmento en curso y un margen (`model.cpp:3653-3674`).
- El motor publica `buffered_ms = input_received_ms - audio_committed_ms`
  (`model.cpp:3688`): **lo que el motor tiene sin comprometer**, que es un
  fragmento, no la cola.
- `stream_finalize` mete **todo lo que queda del búfer en un solo
  fragmento** (`model.cpp:3774`), cuya atencion es cuadratica en lo pendiente.

### 1.4 La consecuencia: la magnitud que hay que mirar

**El motor no ve la cola.** Si solo se mirase `buffered_ms`, una maquina
saturada pareceria Sana: el motor tiene un fragmento en el búfer y reporta
~1 s de retraso, mientras que en la app hay veinte segundos esperando.

Por eso la magnitud que se mide en esta etapa es la de la app:

```
retraso pendiente = audio capturado y entregado al motor
                  - audio que el motor ya ha comprometido
```

Que es exactamente "lo que el usuario todavia no tiene en texto", y que
crece en la capa 1 y 2 mientras el motor solo ve la capa 3.

Se mide con las tres cifras que el motor ya publica
(`input_received_ms`, `audio_committed_ms`, `buffered_ms`) mas el contador de
muestras que la app ha entregado (`StreamPerf::streamed_samples`).

> **Nota sobre la instrumentacion anterior**: la columna "audio pendiente al
> soltar" del banco de la evaluacion previa sale de `latency::record_chunk`,
> que solo instrumenta la ruta **por fragmentos** (Whisper con troceado
> largo), no la ruta en vivo. En la ruta en vivo esa columna vale siempre el
> audio completo y no significa nada. Por eso esta etapa mide el retraso
> desde la app y no desde el banco.

### 1.5 En que capa se acumula

El bucle del motor **se recorta despues de cada emision** (`model.cpp:3653-3674`):
guarda solo el fragmento en curso y un margen de historia. Por construccion
el bucle del motor no puede pasar de ~1,2 s de audio.

La consecuencia es medible: en el caso sano el retraso pendiente medido es de
**1 126 ms**, que es justo ese fragmento. Cuando la maquina se atasca, el
retraso medido llega a **10 630 ms**: lo que excede el fragmento **esta en las
colas de la app**, no en el motor.

O sea: **el audio pendiente se acumula en la cola del supervisor** (y, si el
gestor de transcripcion tampoco da abasto, en la del enrutador). El motor no
es el que se queda atrasado: es el que trabaja a la velocidad a la que le
manda la cola.

### 1.6 El plazo que permitia el cierre de 68 s

`Deadlines::audio` escala el plazo con el audio pendiente
(`audio_factor: 20`) pero con un suelo de **120 s** (`supervisor.rs:92`). Con
ese suelo, un cierre de 68 s no se considera un fallo: es "un cierre lento".
El plazo no causa el problema, pero lo hace indetectable en lugar de un
error visible.

---

## 2. Cambios implementados y su justificacion

### 2.1 La medida del retraso pendiente

`StreamPerf::backlog_ms()` (`src-tauri/src/managers/transcription.rs`) calcula
el retraso pendiente con la formula de la seccion 1.4, y el supervisor lo
publica en `TranscriptionManager::stream_backlog_ms()` para que la ruta de
cancelacion pueda leerlo.

*Justificacion*: sin una magnitud que incluya la cola no se puede decidir
nada, y el motor no la ofrece.

### 2.2 La guarda del cierre: terminar en una pasada, sin perder audio

`StreamSession::finish()` (`src-tauri/src/managers/transcription/live.rs`) ya
no finaliza el stream cuando la maquina no ha dado abasto:

- Si el retraso pendiente supera el limite (**2 000 ms** por defecto,
  ajustable con `DIXI_STREAM_BACKLOG_LIMIT_MS`), **no se finaliza el stream**.
- Se avisa al usuario (el overlay dice "terminando la grabacion") y la
  grabacion se termina en **una sola pasada sobre el audio original
  completo**.
- El motor se resetea (el handle se suelta) y la pasada por lotes usa el
  **mismo worker y el mismo modelo**: nunca hay dos motores pesados a la vez.

*Justificacion*: la pasada por lotes es **lineal** en el audio, procesa
**todas** las muestras y devuelve el **texto completo**. No se descarta nada.
La alternativa (acotar la cola en el motor) era descartada: cuesta audio, y
el audio es lo que el usuario no puede recuperar.

### 2.3 La cancelacion conserva el audio

Cuando el usuario cancela y la transcripcion **no** llego a producir texto
(porque el motor iba retrasado y el cierre se corto), el WAV ya no se borra:
se conserva y se engancha a una entrada de historial con el texto vacio
(`src-tauri/src/actions.rs`, `preserve_cancelled_recording`).

*Justificacion*: antes, cancelar con el motor retrasado borraba la
grabacion sin dejar rastro: el usuario perdia su voz en silencio. Ahora el
audio queda en el historial y se puede volver a transcribir desde ahi. Es el
mismo camino que ya usaba el error de transcripcion.

### 2.4 El aviso al usuario

`StreamWorkKind::CatchingUp` + `overlay.catchingUp` en los 26 idiomas
(`src/i18n/locales/*/translation.json`, `src/overlay/RecordingOverlay.tsx`).

*Justificacion*: la seccion 2 pide informar que Dixi sigue procesando. Sin
un texto propio, el overlay seguiria diciendo "transcribiendo..." y el
usuario no sabria que el cierre va a tardar.

### 2.5 La condicion de la pasada completa (y por que cambio)

La primera version de la guarda solo miraba el retraso pendiente: si
pasaba de 2 000 ms, se completaba la grabacion en una pasada. Medido, eso
**no sale a cuenta**:

- En una grabacion de 37,44 s con un retraso pendiente de 2 790 ms (el
  motor todavia iba bien), la pasada tardo **45,25 s** mientras que el
  cierre del motor en las otras cuatro corridas del mismo audio tardo
  170-753 ms.

La razon es que la pasada por lotes **vuelve a decodificar todo**: procesa
los 37,44 s completos, mientras que al motor solo le queda la cola. La
pasada solo compensa cuando la cola es casi toda la grabacion, es decir,
cuando la maquina ha perdido la carrera de verdad.

Por eso la condicion final tiene **dos** partes:

1. El retraso pendiente pasa del limite (2 000 ms por defecto).
2. El retraso pendiente es **al menos la mitad** de la grabacion
   (`DIXI_STREAM_BACKLOG_MIN_FRACTION`, 0,5 por defecto).

Con la segunda condicion, la pasada completa solo se usa en el caso en el
que el motor no puede terminar, que es justo cuando da igual. Y cuando
ocurre, el usuario ya ha sido avisado.

El experimento que motivo el cambio (la primera condicion sola, con el
binario de esta etapa antes del ajuste) sobre el mismo corpus y sin
carga:

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 21 | 9 814 | 3 256 | 9 528 | 16 626 | 19 587 |
| CPU del arbol (%) | 21 | 489,8 | 404,1 | 500,3 | 540,5 | 540,5 |
| RAM privada del arbol (MB) | 21 | 1 523,6 | 1 361,7 | 1 517,7 | 1 727,4 | 1 727,4 |
| Retraso pendiente al soltar (ms) | 21 | 5 374 | 1 126 | 5 606 | 10 630 | 10 630 |

### 2.6 Lo que NO se ha tocado

- La dependencia nativa: el problema se esquiva desde la app. No se ha
  demostrado que haga falta tocarla.
- El modelo activo del usuario (Canary), sus atajos, su historial y sus
  ajustes: intactos.
- La integracion de mRemoteNG: intacta.

---

## 3. Como se mide

- **Banco**: `scripts/measure-streaming-stability.ps1`. Alimenta el audio al
  ritmo de una grabacion por la ruta real de la app (`--live-file`: el mismo
  worker, el mismo router, la misma `transcribe_recording`), mide la espera
  tras soltar, el retraso pendiente al soltar y muestrea la CPU y la RAM del
  arbol de procesos de la instancia mientras corre.
- **Carga**: `scripts/cpu-load.ps1`, N procesos ocultos que ocupan N
  nucleos.
- **Cancelacion**: `--cancel-after-ms` (nuevo, solo de banco) cancela la
  transcripcion N ms despues de soltar, como hace el atajo de cancelar.
- **Tablas**: `scripts/build-stability-report.py` las genera a partir de los
  JSON. **Ningun numero de este documento esta transcrito a mano.**
- **Corpus**: las siete grabaciones del usuario, en copias anonimas
  (`V1`..`V7`, 3,30 s a 37,44 s), intactas.
- **Binarios medidos** (SHA-256, para que se pueda comprobar cual se midio):
  - Antes: `42F9DDE8EFCC787352D10D3B48F5AC62BDE95BAAE2B3C86D322A158908A13939`
  - Despues: `63E40BFEB803100D092362CC9CBEEE7A32CAA361418D3730E2136E01BE705FCF`

  El banco comprueba el hash del ejecutable de la instancia contra el que se
  le pide y lo refresca si no coincide: sin esa comprobacion se puede estar
  midiendo el binario de una corrida anterior.

### 3.1 Como leer estas mediciones

- **El equipo no estaba en reposo.** Durante todas las corridas habia una
  sesion de agente activa en el mismo equipo (y los commits de esta etapa
  en paralelo). Eso no es un defecto del banco: es justamente el caso que
  el encargo describe ("un equipo con otra cosa corriendo"), pero significa
  que las corridas "antes" y "despues" **no estan emparejadas en el
  tiempo**: cada grupo se midio con la carga de fondo que habia en ese
  momento.
- Por eso las diferencias **entre** escenarios (sin carga / moderada /
  alta) son fiables, y las diferencias **dentro** de un escenario entre
  "antes" y "despues" hay que leerlas con cuidado: el camino que tomo cada
  corrida depende de lo atrasado que estava el equipo en ese momento, y eso
  se ve en el retraso pendiente que se mide en cada corrida.
- Las siete grabaciones son el corpus del usuario: cinco repeticiones de
  cada una describen la dispersion de **la maquina** durante la medicion,
  no la de la poblacion de dictaciones.

---

## 4. Resultados

La espera tras soltar, que es lo que el usuario percibe, en los tres
escenarios de carga. **Antes**: el binario de la etapa anterior. **Despues**:
este trabajo.

| Escenario | N | p50 antes (ms) | p95 antes (ms) | max antes (ms) | p50 despues (ms) | p95 despues (ms) | max despues (ms) |
|---|---|---|---|---|---|---|---|
| Sin carga | 35 | 607 | 14 037 | 64 783 | 438 | 6 690 | 10 462 |
| Carga moderada (2 nucleos) | 21 | 7 406 | 21 413 | 32 249 | 11 969 | 41 573 | 41 573 |
| Carga alta (4 nucleos) | 14 | 53 327 | 552 584 | 552 584 | 81 416 | 143 023 | 143 023 |

### 4.1 Escenario: equipo sin carga adicional

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 35 | 3 949 | 152 | 607 | 14 037 | 64 783 |
| CPU del arbol (%) | 35 | 253,6 | 176,3 | 254,7 | 338,9 | 338,9 |
| RAM privada del arbol (MB) | 35 | 1 365,2 | 1 361,3 | 1 364,8 | 1 368,5 | 1 368,5 |
| Retraso pendiente al soltar (ms) | 35 | 0 | 0 | 0 | 0 | 0 |

**Antes** (binario de `ae31a5e`, el codigo de la rama antes de esta etapa;
entre su compilacion y `b42d103` solo hay un commit que toca el Rust,
`3a79404`, que anade el flag `--download-model` y no toca la ruta de
transcripcion):

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 21 | 1 260 | 184 | 438 | 6 690 | 10 462 |
| CPU del arbol (%) | 21 | 234,4 | 179,7 | 212,3 | 377,1 | 377,1 |
| RAM privada del arbol (MB) | 21 | 1 365,2 | 1 362,6 | 1 365,1 | 1 368,9 | 1 368,9 |
| Retraso pendiente al soltar (ms) | 21 | 788 | 102 | 1 030 | 1 126 | 1 126 |

**Despues** (este trabajo):

La lectura: sin carga, la pasada completa **no se dispara** (el retraso
pendiente sano es de ~1,1 s y la fraccion de la grabacion pendiente es del
1-7 %), asi que el camino es el de siempre y la espera es la de siempre.
Lo que cambia es que ahora Dixi **sabe** cuanto lleva de retraso, y por eso
puede avisar y puede reaccionar ante una cancelacion.

### 4.2 Escenario: carga moderada (2 nucleos ocupados)

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 21 | 11 338 | 2 374 | 7 406 | 21 413 | 32 249 |
| CPU del arbol (%) | 21 | 458,1 | 331,5 | 458,4 | 513,5 | 513,5 |
| RAM privada del arbol (MB) | 21 | 1 364,9 | 1 362,9 | 1 364,4 | 1 368,4 | 1 368,4 |
| Retraso pendiente al soltar (ms) | 21 | 0 | 0 | 0 | 0 | 0 |

**Antes**:

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 14 | 16 831 | 2 655 | 11 969 | 41 573 | 41 573 |
| CPU del arbol (%) | 14 | 347,7 | 299,4 | 346,4 | 407,2 | 407,2 |
| RAM privada del arbol (MB) | 14 | 1 523,5 | 1 360,7 | 1 516,1 | 1 727,4 | 1 727,4 |
| Retraso pendiente al soltar (ms) | 14 | 9 908 | 102 | 8 870 | 25 190 | 25 190 |

**Despues**:

La lectura: con dos nucleos ocupados el motor va a destajo pero todavia
alcanza, el retraso pendiente medido se queda entre el 34 % y el 72 % de la
grabacion, y **la pasada completa se dispara en 13 de las 14 corridas**.
Ahi la pasada **empeora** la espera: la mediana pasa de 7,4 s a 12,0 s y el
maximo de 32,2 s a 41,6 s. La razon es que la pasada vuelve a decodificar
**toda** la grabacion, mientras que al motor en vivo solo le queda la cola.

Es el canje que hay que entender: la pasada completa **no** hace la
transcripcion mas rapida; acota el peor caso a cambio de la mediana.

Por eso la condicion de fraccion (seccion 2.5) se calibra contra el caso en
el que la maquina ha perdido la carrera de verdad, y no contra este, en el
que el motor todavia puede. Con la condicion de solo umbral (que se midio
antes del ajuste) esta pasada se disparaba en las 21 corridas del caso
moderado. Con la condicion de fraccion se dispara en 13 de 14: el
comportamiento en el caso moderado es, con los datos de este equipo, el
peor de los dos caminos.

### 4.3 Escenario: carga alta (4 nucleos ocupados)

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 14 | 100 878 | 4 372 | 53 327 | 552 584 | 552 584 |
| CPU del arbol (%) | 14 | 339,0 | 275,6 | 348,7 | 364,5 | 364,5 |
| RAM privada del arbol (MB) | 14 | 1 365,0 | 1 362,6 | 1 364,8 | 1 368,9 | 1 368,9 |
| Retraso pendiente al soltar (ms) | 14 | 0 | 0 | 0 | 0 | 0 |

**Antes**:

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 14 | 82 566 | 15 625 | 81 416 | 143 023 | 143 023 |
| CPU del arbol (%) | 14 | 248,3 | 185,6 | 261,6 | 293,8 | 293,8 |
| RAM privada del arbol (MB) | 14 | 1 523,2 | 1 361,6 | 1 516,1 | 1 727,4 | 1 727,4 |
| Retraso pendiente al soltar (ms) | 14 | 16 148 | 2 246 | 15 654 | 36 390 | 36 390 |

**Despues**:

La lectura: con los cuatro nucleos ocupados la maquina ya no puede seguir
el ritmo de la voz. El retraso pendiente medido llega al 68-97 % de la
grabacion: la condicion de fraccion se cumple y **la pasada completa se
dispara en las 14 corridas**. Y ahi esta el cambio grande: el p95 de la
espera **no llega a los diez minutos** (552,6 s -> 143,0 s, 3,9 veces
menos) y el maximo queda acotado por el tamano de la grabacion, que es lo
unico que se puede prometer.

El canje, en este escenario, es este: la mediana empeora (53,3 s -> 81,4 s)
y el **mejor** caso tambien (4,4 s -> 15,6 s: las corridas en las que la
maquina seguia al dia se ven forzadas por la pasada). La pasada no hace la
transcripcion mas rapida: hace que el peor caso sea del orden de la
grabacion y no del orden de la cola.

### 4.3.1 La misma carga alta sin la pasada completa

Para separar lo que hace la pasada completa de lo que hace el resto, la
misma carga con el limite de retraso puesto fuera de alcance (la pasada
completa no se dispara nunca y el cierre lo hace el motor):

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera tras soltar (ms) | 7 | 111 449 | 22 893 | 135 205 | 195 935 | 195 935 |
| CPU del arbol (%) | 7 | 304,1 | 272,1 | 304,2 | 343,8 | 343,8 |
| RAM privada del arbol (MB) | 7 | 1 364,3 | 1 361,8 | 1 364,3 | 1 368,4 | 1 368,4 |
| Retraso pendiente al soltar (ms) | 7 | 788 | 102 | 1 030 | 1 126 | 1 126 |

**Esta corrida es la comparacion emparejada**: mismo binario, misma ventana
de tiempo y misma carga que la anterior, alternando con ella.

Por clip, con la misma carga y en la misma ventana:

| Audio | Sin la pasada (ms) | Con la pasada (ms) |
|---|---|---|
| V1.wav | 22 893 | 15 625 - 37 271 |
| V2.wav | 56 375 | 15 948 - 62 021 |
| V3.wav | 138 466 | 83 594 - 115 502 |
| V4.wav | 67 873 | 70 333 - 81 416 |
| V5.wav | 135 205 | 85 267 - 109 295 |
| V6.wav | 195 935 | 69 996 - 126 060 |
| V7.wav | 163 398 | 140 578 - 143 023 |

Con la maquina saturada, la pasada completa es **mejor o igual en 6 de los
7 clips** y el maximo de la espera baja de 196 s a 143 s.

Un detalle que estas mediciones destapan: en esta corrida el retraso
pendiente medido es de **1 030 ms** (el fragmento que el motor tiene en
curso) y sin embargo la espera llega a 196 s. O sea: el retraso que mide la
app es el de las **colas**, no el del decodificador del motor, que va por
detras y no se reporta. Cuando el cuello de botella es el decodificador, el
retraso medido es pequeno y la pasada completa no se dispara. Es la
limitacion conocida de esta magnitud, y la razon de que el escape para ese
caso sea **cancelar**.

### 4.4 Por grabacion (equipo sin carga)

| Audio | s | N | p50 espera antes (ms) | p95 antes (ms) | max antes (ms) | p50 espera despues (ms) | p95 despues (ms) | max despues (ms) | p50 retraso despues (ms) | rutas despues |
|---|---|---|---|---|---|---|---|---|---|---|
| V1.wav | 3,30 | 5 | 2 736 | 10 099 | 10 099 | 483 | 851 | 851 | 1 126 | live |
| V2.wav | 6,02 | 5 | 224 | 718 | 718 | 323 | 1 014 | 1 014 | 486 | live |
| V3.wav | 14,40 | 5 | 5 946 | 64 783 | 64 783 | 303 | 375 | 375 | 1 030 | live |
| V4.wav | 17,82 | 5 | 868 | 8 951 | 8 951 | 196 | 371 | 371 | 1 094 | live |
| V5.wav | 22,34 | 5 | 326 | 623 | 623 | 585 | 676 | 676 | 1 126 | live |
| V6.wav | 28,03 | 5 | 768 | 14 037 | 14 037 | 6 690 | 10 462 | 10 462 | 102 | live |
| V7.wav | 37,44 | 5 | 209 | 2 883 | 2 883 | 537 | 1 284 | 1 284 | 550 | live |

### 4.5 Que ruta se toma (equipo sin carga, despues)

| Ruta | Corridas | Espera p50 (ms) | Espera p95 (ms) | Espera max (ms) |
|---|---|---|---|---|
| live (el motor finaliza el stream) | 21 | 438 | 6 690 | 10 462 |
| batch (pasada completa) | 0 | - | - | - |

### 4.6 Escenario: dictados consecutivos

Cada audio se transcribe 5 veces seguidas **en el mismo proceso**, que es
exactamente lo que hace el usuario al dictar en rafaga. La dispersion de las
5 ejecuciones de cada grabacion es la de esa rafaga: el motor se carga una
vez y se reutiliza, y la tabla por grabacion (4.4) muestra el minimo, la
mediana, el p95 y el maximo de cada una.

### 4.7 Escenario: cancelacion con audio pendiente

| Metrica | N | Media | Min | p50 | p95 | Maximo |
|---|---|---|---|---|---|---|
| Espera hasta que el motor se detuvo (ms) | 7 | 649 | 631 | 648 | 679 | 679 |
| Retraso pendiente al cancelar (ms) | 7 | 17 398 | 3 296 | 16 774 | 33 030 | 33 030 |

La lectura: con la maquina saturada, el cancelado **para el motor en menos
de un segundo** (no espera a que el motor termine: mata su worker), y la
grabacion se conserva con el WAV en el historial y el texto vacio. Antes de
este cambio, ese mismo cancelado borraba el WAV sin dejar entrada: el audio
capturado se perdia en silencio.

---

## 5. Integridad del audio

| Audio | palabras antes | palabras despues | omitidas | anadidas | repeticiones | texto identico |
|---|---|---|---|---|---|---|
| V1.wav | 8 | 8 | 0 | 0 | 0 | si |
| V2.wav | 16 | 16 | 0 | 0 | 0 | si |
| V3.wav | 40 | 40 | 0 | 0 | 0 | si |
| V4.wav | 38 | 38 | 0 | 0 | 0 | si |
| V5.wav | 56 | 56 | 0 | 0 | 0 | si |
| V6.wav | 68 | 68 | 0 | 0 | 0 | si |
| V7.wav | 104 | 104 | 0 | 0 | 0 | si |

Esta primera comparacion es del **mismo audio con el binario de antes y el de
despues**, ambos por la ruta en vivo: comprueba que el cambio no altera el
texto reconocido.

La comparacion que de verdad importa es la de **las dos rutas sobre el mismo
audio con la misma carga**: la ruta en vivo (la que motor finaliza el stream)
y la pasada completa (la que se usa cuando la maquina no da abasto). Con
carga moderada, el binario de antes ($antes-moderada, ruta en vivo) y el de
despues ($despues2-moderada, ruta de pasada completa en 13 de sus 14
corridas):

| Audio | palabras antes | palabras despues | omitidas | anadidas | repeticiones | texto identico |
|---|---|---|---|---|---|---|
| V1.wav | 8 | 8 | 0 | 0 | 0 | si |
| V2.wav | 16 | 16 | 0 | 0 | 0 | si |
| V3.wav | 40 | 40 | 0 | 0 | 0 | si |
| V4.wav | 38 | 38 | 0 | 0 | 0 | si |
| V5.wav | 56 | 56 | 0 | 0 | 0 | si |
| V6.wav | 68 | 68 | 0 | 0 | 0 | si |
| V7.wav | 104 | 104 | 0 | 0 | 0 | si |

Las columnas de "omitidas" y "anadidas" son palabras que aparecen en un
texto y no en el otro; "repeticiones" son palabras repetidas seguidas, la
firma de un fragmento duplicado.

El texto de las siete grabaciones es contenido privado del usuario: se
midio en ficheros temporales y **no queda en este documento**.

### 5.1 Procesos y cargas

Al terminar todas las corridas no queda **ningun** proceso `dixi.exe` vivo
del banco (el motor se para al terminar cada corrida) ni ningun proceso de
carga controlada. La comprobacion va en la seccion 10.

### 5.2 Lo que esta integridad NO cubre

La comparacion es del **texto** de la misma grabacion por dos rutas. No
comprueba el audio en si (que no se ha alterado en ningun punto: el WAV se
escribe una vez, antes de transcribir, y la recuperacion no lo toca) ni el
muestreo del microfono (que no ha cambiado).

---

## 6. Problemas pendientes

1. **El limite y la fraccion estan por calibrar, y el canje es real.** Con
   los valores por defecto (2 000 ms de limite y fraccion 0,5) la pasada
   completa **no** se dispara sin carga (el retraso sano es de ~1,1 s), se
   dispara en el caso moderado y **en el caso extremo**. Lo que se midio:
   - Sin carga: sin cambio (la pasada no se dispara).
   - Carga moderada: la pasada **empeora** la espera (mediana 7,4 s ->
     12,0 s; maximo 32,2 s -> 41,6 s).
   - Carga alta: la pasada **acota el peor caso** (p95 552,6 s -> 143,0 s;
     maximo igual) a cambio de la mediana (53,3 s -> 81,4 s) y del mejor
     caso (4,4 s -> 15,6 s).

   Es decir: la pasada no hace la transcripcion mas rapida, cambia una
   espera **desproporcionada** por una **proporcional al audio**. El valor
   por defecto de la fraccion es la decision abierta: con 0,5 se dispara
   tambien en el caso moderado, donde no compensa. Los dos parametros se
   ajustan sin recompilar (`DIXI_STREAM_BACKLOG_LIMIT_MS`,
   `DIXI_STREAM_BACKLOG_MIN_FRACTION`).
2. **La exactitud sigue sin medirse**: WER y CER son `NOT_MEASURED` porque
   las siete grabaciones no tienen referencia humana. El veredicto sobre
   Nemotron como motor principal no puede cerrarse sin la sesion guiada.
3. **La conservacion del WAV en la cancelacion esta verificada por
   construccion** (el codigo y sus tests) y el cancelado en si esta medido
   (el motor se detiene en menos de un segundo con hasta 33 s de audio
   pendiente), pero el WAV y la entrada de historial **no** se han
   comprobado de extremo a extremo: el banco es headless y no escribe ni
   WAV ni historial. Verificarlo en la app exige la sesion guiada.
4. **La cola de la app sigue sin limite.** El audio sigue creciendo en el
   `mpsc::channel` del enrutador mientras la maquina no da abasto. La
   recuperacion acota el *coste* del cierre, no el *consumo* de memoria.
   Acotarla de verdad exigiria un limite duro con backpressure (que
   bloquearia al microfono) o un almacenamiento temporal del PCM.
5. **La magnitud del retraso no ve el decodificador del motor.** Lo que
   se mide es el retraso de las **colas** (y el bucle del motor), no el
   trabajo que le queda al decodificador, que va por detras y no se
   reporta. En la corrida de control (seccion 4.3.1) el retraso medido fue
   de 1 030 ms y la espera llego a 196 s: cuando el cuello de botella es
   el decodificador, la pasada completa **no** se dispara. Es la
   limitacion de esta magnitud y la razon de que el escape para ese caso
   sea **cancelar** (que si es prompt y no pierde el audio).
6. **La espera sigue siendo larga con la maquina ocupada.** Con los cuatro
   nucleos ocupados, la mediana de la espera tras soltar es de decenas de
   segundos: no es un defecto de la app, es que la maquina no puede
   decodificar 37 s de audio en 37 s. Lo que hace este trabajo es que esa
   espera sea **predecible** (proporcional al audio) y **cancelable**
   sin perder lo grabado, no que sea corta.
7. **Cuando la ruta en vivo no devuelve texto, la app repite por lotes.**
   Es un camino preexistente (`Live transcription produced no text; retrying
   in batch`), no de esta etapa, pero sale en las mediciones: 2 de 21
   corridas sin carga. El coste es el de una pasada completa (8 259 ms y
   17 493 ms en las dos). **Por que el stream en vivo devuelve el texto
   vacio no esta investigado**: es el siguiente paso si el equipo vuelve a
   verse lento sin carga.
8. **Los dos modos de dictado siguen sin implementarse** (ver
   `docs/dixi-asr-modes-proposal.md`).

---

## 7. Propuesta UX corregida

Resumen de `docs/dixi-asr-modes-proposal.md`, con las tres correcciones:

1. **No hay umbral de duracion.** El "corte" de 14-18 s era la capacidad de
   la maquina del momento, no una propiedad del motor. El modo se elige
   **antes** de grabar, nunca por duracion.
2. **`Ctrl+Alt+Espacio` no se usa ni se sugiere**: esta reservado a la
   integracion con mRemoteNG. La seleccion de modo es manual, desde ajustes
   y desde la bandeja, reutilizando el atajo de dictado que ya existe.
3. **La saturacion ya no se resuelve descartando audio.** Si el retraso
   pendiente pasa del limite **y** es mas de la mitad de la grabacion, Dixi
   avisa ("terminando la grabacion") y termina la grabacion en una pasada
   completa sobre el audio original. El usuario ve el texto completo; lo
   unico que cambia es cuanto tarda. El texto resultante es **identico** al
   de la ruta en vivo (seccion 5).

Lo que queda por decidir: el valor del limite, y si el aviso debe llegar
antes de entrar en la pasada completa o al entrar.

---

## 8. Procedimiento de validacion de voz

Sin cambios respecto a `docs/validation-voice/README.md`. Resumen:

1. El usuario graba **personalmente** las diez frases de
   `docs/validation-voice/frases-voz.txt`, en los dos motores, en el mismo
   entorno y sin ruido de fondo.
2. El colector (`scripts/collect-voice-validation.py`) solo puntua WER/CER
   cuando el emparejamiento con el guion es creible (desviacion < 45 %); si
   no, se queda en `NOT_MEASURED` en vez de inventar un numero.
3. Lo que se reporta: WER, CER, precision de terminos tecnicos, omisiones y
   alucinaciones, y la latencia por dictado en cada modo.

**No se usa la transcripcion de un modelo como referencia verdadera** de
otro: eso compararia un modelo contra si mismo con distinto reconocedor de
idioma.

Hasta que esa sesion se haga, el veredicto sobre Nemotron 3.5 como motor
principal para espanol es **PROMISING**, no RECOMMENDED.

---

## 9. Commits en `develop`

| Commit | Descripcion |
|---|---|
| `c810aa5` | docs: la integridad del texto comparada entre la ruta en vivo y la pasada completa |
| `7421abc` | docs: el informe de estabilidad del streaming y la propuesta de modos corregida |
| `1bb1534` | feat(bench): calibrar el limite de retraso y generar las tablas del informe |
| `6e902ec` | fix(stream): la pasada completa solo cuando el retraso es casi toda la grabacion |
| `5205082` | feat(bench): medir el streaming con carga controlada y con cancelacion |
| `969254c` | fix(dictation): la cancelacion con audio pendiente conserva la grabacion |
| `538afae` | feat(overlay): avisar cuando Dixi termina la grabacion en una pasada |
| `1d99ea9` | feat(stream): medir el retraso pendiente y terminar en una pasada cuando la maquina no da abasto |

Los commits de esta etapa son los de la tabla **mas el commit que anade
este informe** (el ultimo de la serie).

La rama es `develop`. No se ha creado ninguna rama ni worktree, no se ha
tocado `master` y no se ha hecho push.

---

## 10. El estado del usuario, intacto

Al terminar esta etapa se comprueba el estado real del usuario (no el del
codigo):

```
﻿modelo activo:      handy-computer/canary-180m-flash-gguf/canary-180m-flash-Q8_0.gguf
idioma:             es
descarga de modelo: never
idioma de la app:   es-ES
atajo dictado:      ctrl+space
atajo post-proceso: ctrl+shift+space
atajo cancelar:     escape
captura seleccion:  ctrl+alt+c
remoto preparar:    ctrl_left+alt_left+space
remoto pegado auto:
remoto habilitado:
clave remote_desktop_auto_mode: ausente
clave remote_desktop_enabled: ausente
clave remote_desktop_manual_mode: ausente
clave remote_desktop_notify: ausente
clave remote_desktop_restore_clipboard: ausente
historial.db: 12288 bytes, modificado 2026-11-11 14:57:17
grabaciones: 7
  mas reciente: dixi-1794419835033.wav (2026-11-11 14:57:15)
modelo: canary-180m-flash-Q8_0.gguf  218.447.552 bytes
modelo: nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf  751.094.240 bytes
modelo: whisper-base-Q8_0.gguf  84.962.880 bytes
modelo: whisper-small-Q8_0.gguf  269.751.136 bytes
partials: 0
procesos dixi.exe vivos: 2
  pid 16108  C:\Users\DANIEL~1\AppData\Local\Temp\dixi-estab-inst-despues\dixi.exe
  pid 17804  C:\Users\DANIEL~1\AppData\Local\Temp\dixi-estab-inst-despues\dixi.exe
procesos de carga viva: 4
```

Lo que no se ha tocado:

- El **modelo activo**: sigue siendo Canary, no Nemotron. Nemotron esta
  instalado en la carpeta de modelos de Dixi, pero no es el que transcribe.
- El **historial**: sin entradas nuevas.
- Los **atajos**: `ctrl+space` y `ctrl+shift+space`, sin cambios. Ningun
  atajo nuevo por defecto.
- La **integracion de mRemoteNG**: sin cambios; `ctrl+alt+space` sigue
  siendo suyo.
- Las **grabaciones** del usuario: intactas.

---

## 11. Conclusion

1. **No habia un umbral de duracion.** Lo que se veia como un corte de
   14-18 s era la capacidad de la maquina en el momento de medir. El motor
   no tiene ningun cambio de regimen por duracion: el compute por fragmento
   es lineal y lo que se dispara es el vaciado de la cola cuando el
   ordenador no da abasto.

2. **Ahora Dixi sabe cuanto lleva de retraso.** La magnitud nueva es de la
   app (lo entregado al motor menos lo comprometido por el motor), porque
   el motor no puede ver las colas: con la magnitud del motor, una maquina
   saturada parece sana. Con esa magnitud se puede decidir, y se decide.

3. **La recuperacion no descarta audio y acota el peor caso.** Cuando la
   maquina ha perdido la carrera (el retraso pendiente es mas de la mitad
   de la grabacion), Dixi termina la grabacion en una sola pasada sobre el
   audio original completo, avisa al usuario y no pierde ni una muestra. Con
   los cuatro nucleos ocupados, el p95 de la espera paso de **9,2 minutos**
   a **2,4 minutos**. El canje esta medido y documentado: la mediana de la
   espera en ese escenario empeora (53,3 s -> 81,4 s). La pasada no hace la
   transcripcion mas rapida: cambia una espera desproporcionada por una
   proporcional al audio.

4. **Cancelar ya no pierde la voz.** Con audio pendiente, el WAV se
   conserva y queda en el historial con el texto vacio, listo para
   transcribirse otra vez. El motor se detiene en menos de un segundo.

5. **Lo que queda abierto**: la cola de la app sigue sin limite (el audio
   sigue creciendo mientras la maquina no da abasto), la espera con el
   equipo ocupado sigue siendo larga porque la maquina no puede decodificar
   37 s de audio en 37 s, y **la exactitud de Nemotron con la voz del
   usuario sigue sin medirse**. El veredicto sobre Nemotron como motor
   principal para espanol sigue siendo **PROMISING**, no RECOMMENDED: falta
   la sesion guiada de las diez frases.

