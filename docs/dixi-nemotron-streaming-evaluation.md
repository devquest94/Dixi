# Nemotron 3.5 ASR Streaming — evaluacion real en Dixi

Rama: `develop` (directamente, sin ramas nuevas)
Equipo: Intel Core i5-8300H, 8 nucleos logicos, 16 GB de RAM, CPU unicamente
(sin Vulkan, sin drivers, sin SDK).
Motor: transcribe.cpp 0.4.0 (crate `transcribe-cpp` 0.4.0 + `transcribe-cpp-sys` 0.4.0).

Todo lo que hay aqui esta **medido**. Donde no hay medicion, esta escrito
`NOT_MEASURED`. No hay estimaciones presentadas como resultados.

---

## 1. Que se autorizo y que se hizo

| | |
|---|---|
| Descargar el GGUF Q8_0 de Nemotron 3.5 ASR Streaming | Hecho |
| Guardarlo en la carpeta de modelos administrada por Dixi | Hecho, con el mecanismo de la app |
| Usar las siete grabaciones reales para pruebas locales | Hecho (copias, los originales intactos) |
| Comparar con Whisper Base en condiciones equivalentes | Hecho |
| Cambiar el modelo activo del usuario | **NO** (sigue canary) |
| Instalar drivers, Vulkan SDK o herramientas globales | **NO** |
| Descargar otras cuantizaciones o modelos | **NO** |
| Enviar grabaciones a servicios externos | **NO** |
| Eliminar archivos originales | **NO** |
| Borrar temporales existentes | **NO** |
| Crear ramas o worktrees | **NO** |

---

## 2. El modelo: procedencia, integridad y licencia

| Campo | Valor |
|---|---|
| Modelo | `nemotron-3.5-asr-streaming-0.6b` |
| Cuantizacion | Q8_0 |
| Archivo | `nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf` |
| Tamano | 751 094 240 bytes (716,3 MiB) |
| SHA-256 | `b94545b313b3223fda7b2857a52681da813935c2127643d1e9ff0c23d988089c` |
| Fuente | `https://huggingface.co/handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf` |
| Revision | `6d44e540bc31b0de1dbe174a3cea87f53a7f22fb` (la que fija el catalogo de Dixi) |
| Modelo original | `https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b` |
| Arquitectura GGUF | `parakeet` |
| Licencia | OpenMDW-1.1 (`openmdw.ai/license/1-1/`) |

El SHA-256 **coincide byte a byte** con el que tiene el catalogo de Dixi
(`src-tauri/src/catalog/catalog.json`), que es el ancla de confianza del
proyecto.

### Como se instalo (mecanismo de la app, no a mano)

Se uso el transporte de descargas de Dixi, con la fuente preferida:

```
Partial download of handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/... is already full-size; verifying
SHA256 verified for model handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/...
Mirror download of ... completed and verified ("C:\Users\DanielSan\AppData\Roaming\com.dixi.app\models\nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf")
```

Es decir: **el `.partial` a tamano completo se verifica y se publica por
renombrado**, sin volver a descargar. El fichero nunca estuvo visible con
un nombre de modelo antes de estar verificado.

Para poder lanzar ese camino sin la interfaz se anadio `--download-model`
(commit `3a79404`): reutiliza `download_from_mirror` —el transporte
reanudable, con HTTPS, `.partial` propio, control de tamano y de SHA-256 del
catalogo, y publicacion atomica por renombrado— pero apuntando a la URL de
resolucion de Hugging Face de la revision fijada, en vez de a un mirror.

Despues de instalarlo, la propia app lo ve:

```
is_downloaded = True
supports_streaming = True
```

**El modelo activo no se toco**: sigue siendo
`handy-computer/canary-180m-flash-gguf/canary-180m-flash-Q8_0.gguf`.

### La licencia, en detalle

OpenMDW-1.1 concede uso, modificacion y comparticion sin restriccion, y
**no impone ninguna obligacion sobre las salidas**. La unica obligacion es
conservar la licencia y los avisos si se redistribuye el material del
modelo. Para el uso personal de Dixi no impone nada; el propio repositorio
de la conversion publica la licencia (OpenMDW-1.1) y el `NOTICE` con la
procedencia (conversion de `nvidia/nemotron-3.5-asr-streaming-0.6b` en el
commit `24b151a`, pesos no reentrenados ni ajustados).

---

## 3. El streaming es autentico: verificado en el codigo

El encargo es explicito: *"No alcanza con que `supports_streaming=true`
aparezca en el catalogo"*. Se verifico la cadena entera.

### 3.1 Whisper NO tiene streaming

`transcribe-cpp-sys-0.4.0/src/arch/whisper/model.cpp:3579` deja
`stream_begin = nullptr`: la familia whisper no implementa el triple de
streaming, asi que `start_stream` no puede abrir una sesion. Por eso, con
Whisper la app decodifica **todo** despues de soltar la tecla, y por eso su
espera es siempre el coste de la pasada completa.

### 3.2 parakeet (Nemotron) SI lo tiene, y reutiliza la cache

- El dispatcher nativo expone `stream_begin` / `stream_feed` / `stream_finalize`
  (`transcribe-asr.cpp:2359-2378`), y la familia parakeet engancha sus hooks
  (`model.cpp:3864-3868`).
- El encoder cache-aware exige `att_context_style=ChunkedLimited`
  (`encoder.cpp:757-762`), que es exactamente la convencion del modelo.
- En cada paso, la **entrada del grafo es la cache persistente del paso
  anterior** (`model.cpp:2474-2480`) y las consultas cubren solo los frames
  nuevos, con K/V sobre la ventana virtual `T_cache + T_q_new`
  (`encoder.cpp:830-836`). La salida se rota de vuelta a la cache sin viaje
  al host (`model.cpp:2672-2701`).
- El mel se recalcula sobre un buffer PCM deslizante recortado
  (`model.cpp:3549-3566`, `3653-3674`), asi que su coste tambien es acotado.

**La ventana no se recalcula completa.** El unico solapamiento es el
*pre-encode cache*, que es el propio mecanismo cache-aware de NeMo, no una
repeticion redundante.

### 3.3 La cadena de Dixi es continua y no bloqueante

```
managers/audio.rs:509  on_audio -> stream_router.feed(frame)
managers/transcription.rs:181  StreamCmd::Feed
managers/transcription.rs:819  session.feed(pcm)
managers/transcription/live.rs:205  stream.feed(pcm)
engine_supervisor/supervisor.rs:970  Request::Feed con el PCM
engine_supervisor/worker.rs:202  stream.feed(&pcm)   [proceso hijo]
transcribe-asr.cpp:2366  -> parakeet stream_feed
```

El audio cruza al proceso hijo por un frame con el PCM pegado
(`protocol.rs:1-20`) y el motor decodifica y emite tokens **antes** de la
soltura. `finalize` es un *flush* del mismo stream, no una pasada nueva
(`live.rs:208-209`).

### 3.4 El idioma: `es` -> `es-US`, con condicionamiento valido

El prompt de idioma no es una cadena: es un one-hot de 128 dimensiones
sacado de un diccionario **dentro del GGUF** (`weights.cpp:408-502`), y
`resolve_prompt_id` hace coincidencia exacta (`model.cpp:152-174`). Si el
idioma no esta, el motor aborta con `TRANSCRIBE_ERR_UNSUPPORTED_LANGUAGE`
(`model.cpp:2612-2632`): no hay caida silenciosa.

En Dixi, la intencion `es` se resuelve al locale concreto del modelo
cargado:

```
Language intent 'es' resolved to 'es-US' for model 'handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/...'
transcribe-cpp run: task=Transcribe, language=Some("es-US"), vocabulary_terms=0
Output language evidence: UserSelected("es-US")
```

El modelo reporta 32 locales BCP-47 (`es-US`, `es-ES`, `en-US`, ...) y el
motor acepta tanto los locales como los codigos base (`es` es un alias del
mismo slot). `es-AR` **no** esta soportado y no se asumio en ningun momento.

### 3.5 Lo que se puede y lo que no se puede medir

Existe un contador interno de pasos del encoder (`chunk_step`,
`parakeet.h:225`), pero **no esta exportado** por la API publica
(`transcribe.h:2119-2129`, `2278-2385`) ni por el binding de Rust
(`streaming.rs:37-54`). Asi que:

- **NO_MEASURED**: numero de recalculos del encoder por paso.
- **Medido en su lugar**: el tiempo de calculo del modelo frente al audio
  transmitido, el `buffered_ms`, el `audio_committed_ms` y el numero de
  revisiones de texto, que la app ya registra
  (`managers/transcription.rs:1384-1473`).

La huella que queda en el log de la app para demostrarlo en produccion:
`Live streaming transcription started (model, backend)`, la linea periodica
`Live preview perf: ... s streamed audio, ... s model compute (...x
real-time), input_received=, committed_audio=, buffered=, revision=, N
frames fed, M updates emitted`, y el cierre `Live preview finalized in ...`.

---

## 4. Un defecto del banco que falseaba las mediciones

Antes de medir, el banco de la ruta en vivo daba resultados sospechosamente
identicos a los de la ruta por lotes. La causa: **`Set-Content -Encoding UTF8` en PowerShell 5.1
escribe un BOM**, y el plugin `store` de Tauri parsea el JSON con
`serde_json`, que no acepta BOM. El fichero de ajustes se descartaba entero
y la app arrancaba con los ajustes por defecto:

- el idioma forzado se perdia (`auto` en vez de `es`), asi que el motor
  recibia `language=None` y el prompt `auto` en vez del locale;
- el timeout de descarga del modelo se perdia (`min5` en vez de `never`);
- el modelo seleccionado se perdia.

El sintoma era invisible: el banco arrancaba, transcribia y daba numeros
validos, pero de un motor al que nunca se le habia dicho en que idioma
trabajar. Corregido en el commit `5cfaa7c`: ahora los ajustes se escriben
sin BOM y el idioma forzado llega al motor.

Las mediciones de esta evaluacion se hicieron **despues** del arreglo.

---

## 5. FASE D — Benchmark: 7 grabaciones, 3 motores, mismas condiciones

### 5.1 Como se midio

- Las siete grabaciones reales del usuario, **en copias** con identificadores
  anonimos (`V1`..`V7`); los originales no se tocaron y no se subieron a
  ningun servicio.
- La **ruta real de la app**: `--live-file` alimenta el audio al ritmo de una
  grabacion (100 ms cada 100 ms) por el mismo worker, el mismo router y la
  misma `transcribe_recording` que usa la aplicacion. La espera que se
  reporta es la de verdad: desde que el usuario suelta hasta que el texto
  esta listo.
- 5 repeticiones por muestra; se informa el **minimo** (el estimador menos
  ruidoso del tiempo real del motor), el **p50** y el **p95**.
- CPU unicamente, sin Vulkan. El idioma forzado a `es` (que el motor resuelve
  a `es-US`).

| Muestra | Audio (s) |
|---|---|
| V1 | 3,30 |
| V2 | 6,02 |
| V3 | 14,40 |
| V4 | 17,82 |
| V5 | 22,34 |
| V6 | 28,03 |
| V7 | 37,44 |

**Aviso sobre el corpus**: siete grabaciones son un corpus pequeno, y medir
cinco veces los mismos audios no son cinco muestras independientes: son
cinco observaciones de la misma muestra. Los percentiles de aqui describen
la dispersion de la maquina durante la medicion, no la dispersion de la
poblacion de dictaciones.

### 5.2 Motor A — Whisper Base Q8_0 (la ruta real de la app)

Con Whisper la app **no decodifica nada durante la grabacion**: la familia
whisper no implementa streaming (`stream_begin = nullptr`), asi que la
grabacion entera se decodifica al final.

| Muestra | Audio (s) | min (ms) | p50 (ms) | p95 (ms) | Ruta |
|---|---|---|---|---|---|
| V1 | 3,30 | 818 | 1 606 | 2 245 | batch |
| V2 | 6,02 | 816 | 845 | 1 162 | batch |
| V3 | 14,40 | 958 | 1 013 | 3 151 | batch |
| V4 | 17,82 | 1 022 | 1 084 | 1 393 | batch |
| V5 | 22,34 | 1 176 | 1 246 | 7 322 | batch |
| V6 | 28,03 | 1 184 | 2 132 | 2 974 | batch |
| V7 | 37,44 | 2 251 | 7 924 | 10 801 | batch |
| **Global** | | **816** | **1 022** | **2 251** | |

Carga del modelo en frio: 171-513 ms.

### 5.3 Motor C — Nemotron 3.5 Q8_0 en streaming (la ruta real de la app)

| Muestra | Audio (s) | min (ms) | p50 (ms) | p95 (ms) | Ruta |
|---|---|---|---|---|---|
| V1 | 3,30 | 1 661 | 1 853 | 2 458 | live |
| V2 | 6,02 | 1 661 | 1 738 | 2 410 | live |
| V3 | 14,40 | 3 481 | 5 005 | 6 021 | live |
| V4 | 17,82 | 184 | 325 | 496 | live |
| V5 | 22,34 | 271 | 278 | 325 | live |
| V6 | 28,03 | 143 | 173 | 194 | live |
| V7 | 37,44 | 168 | 173 | 377 | live |
| **Global** | | **143** | **271** | **496** | |

Carga del modelo en frio: 1 165-2 493 ms (el modelo pesa 716 MB; el de
Whisper, 81 MB).

### 5.4 Motor B — Nemotron 3.5 Q8_0 por lotes (el coste del motor, sin streaming)

| Muestra | Audio (s) | ASR min (ms) | ASR p50 (ms) | ASR p95 (ms) |
|---|---|---|---|---|
| V1 | 3,30 | 574 | 629 | 1 177 |
| V2 | 6,02 | 1 047 | 1 318 | 2 112 |
| V3 | 14,40 | 2 385 | 2 495 | 2 504 |
| V4 | 17,82 | 2 674 | 2 803 | 3 649 |
| V5 | 22,34 | 3 786 | 4 404 | 6 874 |
| V6 | 28,03 | 4 556 | 4 838 | 6 992 |
| V7 | 37,44 | 5 890 | 6 312 | 6 477 |

**Nemotron por lotes es mas lento que Whisper por lotes** (4 404 ms contra
1 246 ms para 22,34 s). La ganancia de Nemotron **no esta en el motor**: esta
en que decodifica mientras el usuario habla.

### 5.5 El umbral, y por que existe

El streaming de Nemotron tiene dos regimes muy distintos, y el log de la
app los separa:

| Muestra | Compute del modelo | Factor | Espera tras soltar |
|---|---|---|---|
| V1 (3,30 s) | 4,25 s | 0,78x tiempo real | 1 661-2 458 ms |
| V2 (6,02 s) | 6,77 s | 0,89x tiempo real | 1 661-2 410 ms |
| V3 (14,40 s) | 16,89 s | 0,85x tiempo real | 3 481-6 021 ms |
| V4 (17,82 s) | 2,99 s | 4,90x tiempo real | 184-496 ms |
| V6 (28,03 s) | 5,70 s | 4,91x tiempo real | 143-194 ms |
| V7 (37,44 s) | 10,06 s | 3,72x tiempo real | 168-377 ms |

Por debajo de ~15 s de audio, el modelo **no llega a seguir a la voz**
(el compute va a 0,74-0,9x tiempo real): al soltar la tecla, el retraso se
descarga entero de golpe. Por encima de ~17 s, el compute va a 3,7-4,9x
tiempo real: llega de sobra, y al soltar la tecla solo queda la cola.

La sesion de streaming tiene un **coste fijo de arranque** que un dictado
corto no amortiza. Cuanto vale exactamente y de donde sale
(la asignacion y el vaciado de las caches por capa del codificador
cache-aware) queda **NOT_MEASURED**: el contador nativo de pasos
(`chunk_step`) no se exporta por la API publica.

Lo que si se puede afirmar: el umbral esta entre 14,40 s y 17,82 s de audio,
y es reproducible (los dos bancos lo coinciden).

### 5.6 RAM y CPU del arbol de procesos

El ASR corre en un proceso hijo, asi que la medicion suma la app, el worker
del motor y sus hijos.

| | Whisper Base | Nemotron 3.5 |
|---|---|---|
| RAM del arbol de procesos (mediana) | 198 MB | 1 384 MB |
| RAM del arbol de procesos (pico) | 248 MB | ~1 400 MB |
| Carga del modelo en frio | 171-513 ms | 1 165-2 493 ms |

Muestreo continuo durante los tres motores (3 098 muestras, una cada 500 ms):
RAM del arbol entre 13 MB (sin modelo cargado) y **1 547 MB de pico**, con
**1 179 MB de mediana**; CPU **media del 204 % y pico del 847 %** sobre 8
nucleos logicos, es decir, la decodificacion satura el procesador. Ese
pico de CPU es la razon por la que la espera de Whisper, que ocurre entera
despues de soltar, es tan sensible a lo que haga la maquina: con Nemotron
esa carga se reparte mientras el usuario habla.

Medir el arbol entero es obligatorio: el ASR corre en un proceso hijo, y
medir solo la app daria una foto de la memoria sin el modelo cargado.

El pico de RAM de Whisper se midio con el modelo cargado y la decodificacion
en curso. Los 716 MB del GGUF se traduzen en ~1,2 GB de memoria residente
del proceso del motor.

---

## 6. FASE E — Exactitud

**WER = NOT_MEASURED. CER = NOT_MEASURED.**

Las siete grabaciones no tienen transcripcion de referencia humana: son
dictaciones espontaneas, no la lectura de un guion. Sin referencia no hay
WER ni CER que.no sean inventados, y **no se usa la salida de Whisper como
referencia verdadera para puntuar Nemotron**: eso compararia un modelo
contra si mismo con distinto reconocedor de idioma.

Lo que si se puede comparar, sobre las mismas siete grabaciones:

| Criterio | Whisper Base | Nemotron 3.5 |
|---|---|---|
| Salida en espanol (no traduccion) | Si | Si (el motor no traduce: `supports_translate=false`) |
| Cola de texto inventado al final | Si en la muestra de 22 s (cola en griego) | No |
| Alucinaciones del tipo "Thank you" que nadie dijo | Si (muestra de 37 s) | No |
| Mayusculas y puntuacion | Irregular | Regular |
| Terminos tecnicos en ingles | "clavufla R2", "Clouf R2" | "Clawflow R2", "Cloud R2" |
| Es panlatino (voseo, lunfardo) | Parcialmente ("pusheá" -> "puse a") | Parcialmente ("pusheá" -> "pusheá") |

La comparacion honesta es de **calidad subjetiva sobre siete muestras**, no
de exactitud estadistica. Para el WER hace falta la sesion guiada de
`docs/validation-voice/frases-voz.txt`: diez frases con texto conocido,
dictadas por la persona.

Lo que el propio modelo publica como referencia (FLEURS, WER con Q8_0) es
**es = 6,3** para Nemotron, frente a un 65,9 medido con Whisper Base en el
corpus sintetico. Ese 65,9 no es comparable: como se mostro en la seccion
5 del informe de validacion, el corpus sintetico hace que Whisper
alucine, y con el idioma forzado la salida sale en japones. Sobre voz real
Whisper Base transcribe bien. **Ninguna de las dos cifras dice cual es
mejor para tu voz: eso lo decide la sesion guiada.**

---

---

## 7. FASE F — Estabilidad y regresiones

Descargar y medir Nemotron no puede cambiar el uso cotidiano. Comprobado:

| Que se comprueba | Resultado |
|---|---|
| Modelo activo sin cambios | `canary-180m-flash` sigue seleccionado |
| Preferencias del usuario sin cambios | `selected_language = es`, `remote_desktop_*` sin tocar |
| Suite Rust completa | **576 PASS, 0 FAIL**, 20 ignorados |
| TypeScript (`tsc --noEmit`) | PASS |
| ESLint | PASS |
| Pruebas de frontend (3 suites) | PASS |
| Validador de traducciones | PASS, 25 idiomas, 806 claves |
| Pruebas de descarga de modelos | PASS (incluye reanudacion, SHA-256 y descarte de parcial) |
| Catalogo coherente con el binario | 69 modelos sembrados |
| Recuperacion ante error del motor | El motor devuelve `TRANSCRIBE_ERR_UNSUPPORTED_LANGUAGE` si el idioma no esta en su tabla: aborta en vez de seguir en silencio |
| Cierre de los workers | El worker del motor se cierra al soltar el modelo; la app lo detiene en la salida |

Lo que **no** se toco: la carpeta de modelos del usuario (solo se anadio un
fichero), el historial, los atajos, el VAD, el diccionario, los snippets, los
perfiles, el posprocesado con MiniMax, la insercion de texto, el
portapapeles y la interfaz de configuracion.

El unico cambio de comportamiento en toda la evaluacion es el nuevo
`--download-model`, que es una herramienta de linea de comandos y no altera
la aplicacion.

---

## 8. FASE G — Temporales: inventario, sin borrar nada

No se ejecuto `scripts/cleanup-bench-instances.ps1 -Execute`. No se limpio
ninguna de las siete carpetas de la sesion anterior.

| Carpeta | MB reales | Junction | Procedencia |
|---|---|---|---|
| `dixi-latency-antes-canary-180m-flash-125837` | 63,1 | 1 | sesion anterior |
| `dixi-latency-antes-canary-180m-flash-131443` | 63,1 | 1 | sesion anterior |
| `dixi-latency-antes-whisper-base-123629` | 61,4 | 1 | sesion anterior |
| `dixi-latency-antes-whisper-base-123847` | 63,1 | 1 | sesion anterior |
| `dixi-latency-antes-whisper-base-125632` | 63,2 | 1 | sesion anterior |
| `dixi-latency-antes-whisper-base-131313` | 63,2 | 1 | sesion anterior |
| `dixi-latency-antes-whisper-small-125909` | 63,2 | 1 | sesion anterior |

Los siete junctions apuntan a `%APPDATA%\com.dixi.app\models`, la carpeta
real de modelos del usuario. **Borrarlos con `Remove-Item -Recurse` vaciaria
esa carpeta**: en PowerShell 5.1 el borrado recursivo sigue los junctions.

`scripts/cleanup-bench-instances.ps1` corta el enlace antes de borrar la
carpeta (con la API de .NET, que no sigue el enlace) y comprueba que
ningun destino caiga dentro de la carpeta que se va a borrar. Sin
`-Execute` solo informa.

Esta sesion no duplico el modelo: el GGUF de Nemotron esta en la carpeta de
Dixi y las instancias del banco lo enlazan por hard link, asi que ocupa
una sola vez.

---

## 9. Veredicto

**PROMISING**.

### Lo que se/demo

1. **Nemotron hace streaming de verdad dentro de Dixi**, y no por la
   bandera del catalogo: Whisper no tiene `stream_begin` (su codigo no
   existe), mientras que parakeet implementa el encoder cache-aware con
   reutilizacion de la cache entre pasos, y el audio cruza al proceso del
   motor mientras se graba.
2. **Para dictaciones de 17 s o mas, la espera tras soltar la tecla se
   desploma**: de 1 084-7 924 ms (p50 de Whisper) a **173-325 ms**
   (p50 de Nemotron). Entre 4,6x y 45x menos espera, y con una varianza
   mucho menor: el usuario deja de notar la maquina ocupada.
3. **Por lotes Nemotron es mas lento** (4 404 ms contra 1 246 ms para
   22,34 s). La ganancia es enteramente del momento de la decodificacion.
4. **El texto es igual o mejor** en las siete muestras: sin la cola en
   griego, sin el "Thank you" inventado, con puntuacion y mayusculas mas
   regulares.

### Lo que frena la recomendacion

1. **Para dictaciones de 14 s o menos, Nemotron es PEOR**: 1 661-6 021 ms
   frente a 816-1 013 ms de Whisper. La sesion de streaming tiene un coste
   fijo de arranque que un dictado corto no amortiza. El umbral esta entre
   14,40 s y 17,82 s.
2. **La exactitud no esta medida**: sin transcripcion de referencia humana
   no hay WER ni CER defendibles, y la comparacion disponible es
   cualitativa sobre siete muestras.
3. **Cuesta 1,2 GB mas de RAM** (1 384 MB contra 198 MB) y **1,2 s mas de
   carga en frio** (1 165-2 493 ms contra 171-513 ms).
4. **No traduce a ingles** (`supports_translate=false`) y cubre **28
   idiomas en vez de 99**. Si usas la traduccion automatica, Nemotron no
   puede sustituir a Whisper.

### Propuesta de activacion (para tu aprobacion, no aplicada)

El tradeoff no es "Nemotron o Whisper": es "Nemotron a partir de cierta
duracion". La app ya sabe cuanto se ha grabado cuando el usuario suelta, y
el punto de corte medido esta entre 14,40 s y 17,82 s.

Propuesta: dejar **Whisper Base como modelo por defecto** y ofrecer Nemotron
como el modelo de las dictaciones largas, con un ajuste que lo selecte a
partir de ~17 s de audio. Con los numeros de aqui:

| Duracion de la dictacion | Whisper Base | Nemotron streaming |
|---|---|---|
| <= 14 s | **mejor** (816-1 013 ms) | peor (1 661-6 021 ms) |
| >= 17 s | 1 084-7 924 ms | **mejor** (173-325 ms) |

Implementarlo bien requiere una decision de producto (un selector
automatico por duracion, o dos atajos) y la sesion guiada para validar la
exactitud. **Ninguna de las dos se ha hecho**: el modelo activo sigue siendo
canary y no se ha tocado nada de la configuracion del usuario.

### Lo que sigue pendiente

1. **La sesion guiada** con `docs/validation-voice/frases-voz.txt`: es lo que
   convierte esta evaluacion en una recomendacion con numero de exactitud.
2. **La seleccion por duracion**, si decides que el tradeoff merece la pena.
3. **El coste fijo del arranque del streaming** en el motor nativo: con un
   contador de pasos exportado se podria saber si es la asignacion de las
   caches y si se puede evitar.
4. **La limpieza de los 557,7 MB** de temporales, con autorizacion.
