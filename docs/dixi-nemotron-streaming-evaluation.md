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

## 5. FASE D — Benchmark corregido: 7 grabaciones, 3 motores

Las siete grabaciones reales del usuario, en **copias** con
identificadores anonimos (`V1`..`V7`); los originales no se tocaron.
La **ruta real de la app**: `--live-file` alimenta el audio al ritmo de
una grabacion (100 ms cada 100 ms) por el mismo worker, el mismo router
y la misma `transcribe_recording`. 5 repeticiones por grabacion: **35
ejecuciones por motor**. CPU unicamente, sin Vulkan. Idioma forzado a
`es` (que el motor resuelve a `es-US`).

**Aviso sobre el corpus**: siete grabaciones son un corpus pequeno, y
medir cinco veces los mismos audios no son cinco muestras
independientes: son cinco observaciones de la misma muestra. Los
percentiles describen la dispersion de la maquina durante la medicion,
no la de la poblacion de dictaciones.

### Metodologia: tres estadisticas que no son lo mismo

El banco por lotes y el de la ruta en vivo guardan ahora **una fila
por ejecucion** (`ejecuciones*.json`). Sobre esa lista se calculan tres
cosas distintas, y antes se confundian:

1. **POR GRABACION**: la distribucion de las N repeticiones de cada
   grabacion. Es lo que describe una grabacion concreta.
2. **AGRUPADA**: la distribucion de TODAS las ejecuciones del motor
   (7 grabaciones x 5 repeticiones = 35). Es lo que describe el motor.
3. **POR SUBCONJUNTO**: las ejecuciones de las grabaciones cortas
   frente a las largas.

> El error corregido: los percentiles "globales" se calculaban sobre
> una fila por grabacion que ya era el MINIMO de sus repeticiones. El
> "p50 global" era la mediana de 7 minimos, y el "p95 global" era un
> percentil sobre 7 valores. Ninguno de los dos era el p50 o el p95 de
> las 35 ejecuciones.


### Resultados agrupados (35 ejecuciones por motor)

| Motor | N | min | p50 | p95 | max |
|---|---|---|---|---|---|
| Whisper Base, espera tras soltar (ms) | 35 | 980 | 1864 | 7206 | 7640 |
| Nemotron streaming, espera tras soltar (ms) | 35 | 164 | 389 | 1749 | 1973 |
| Nemotron por lotes, inferencia ASR (ms) | 35 | 792 | 3834 | 7457 | 7928 |

### Por subconjunto (corte en 16 s de audio)

| Motor | Subconjunto | N | min | p50 | p95 | max |
|---|---|---|---|---|---|---|
| Whisper Base | <= 16 s | 15 | 980 | 1199 | 3798 | 3798 |
| Whisper Base | > 16 s | 20 | 1346 | 2552 | 7206 | 7640 |
| Nemotron streaming | <= 16 s | 15 | 223 | 424 | 1973 | 1973 |
| Nemotron streaming | > 16 s | 20 | 164 | 264 | 782 | 1312 |

### Whisper Base, por grabacion

| Grabacion | Audio (s) | N | p50 | p95 | min | max |
|---|---|---|---|---|---|---|
| V1.wav | 3.30 | 5 | 994 | 1625 | 980 | 1625 |
| V2.wav | 6.02 | 5 | 1199 | 1958 | 1082 | 1958 |
| V3.wav | 14.40 | 5 | 1514 | 3798 | 1149 | 3798 |
| V4.wav | 17.82 | 5 | 1556 | 3073 | 1346 | 3073 |
| V5.wav | 22.34 | 5 | 1864 | 3357 | 1420 | 3357 |
| V6.wav | 28.03 | 5 | 3189 | 7206 | 2057 | 7206 |
| V7.wav | 37.44 | 5 | 4267 | 7640 | 3257 | 7640 |

### Nemotron streaming, por grabacion

| Grabacion | Audio (s) | N | p50 | p95 | min | max |
|---|---|---|---|---|---|---|
| V1.wav | 3.30 | 5 | 424 | 1166 | 356 | 1166 |
| V2.wav | 6.02 | 5 | 1044 | 1973 | 223 | 1973 |
| V3.wav | 14.40 | 5 | 399 | 593 | 289 | 593 |
| V4.wav | 17.82 | 5 | 185 | 546 | 164 | 546 |
| V5.wav | 22.34 | 5 | 389 | 452 | 244 | 452 |
| V6.wav | 28.03 | 5 | 430 | 1312 | 264 | 1312 |
| V7.wav | 37.44 | 5 | 213 | 782 | 183 | 782 |

### Nemotron por lotes, por grabacion (inferencia ASR)

| Grabacion | Audio (s) | N | p50 | p95 | min | max |
|---|---|---|---|---|---|---|
| V1.wav | 3.30 | 5 | 958 | 1312 | 792 | 1312 |
| V2.wav | 6.02 | 5 | 1219 | 3026 | 1044 | 3026 |
| V3.wav | 14.40 | 5 | 3022 | 5472 | 2945 | 5472 |
| V4.wav | 17.82 | 5 | 3395 | 4560 | 2672 | 4560 |
| V5.wav | 22.34 | 5 | 4978 | 6565 | 4716 | 6565 |
| V6.wav | 28.03 | 5 | 4853 | 5078 | 4528 | 5078 |
| V7.wav | 37.44 | 5 | 7073 | 7928 | 6366 | 7928 |

## 6. El comportamiento entre 14 y 18 s: lo que se investigo

La evaluacion anterior afirmaba un **umbral** entre 14,40 s y 17,82 s de
audio: por debajo, Nemotron tardaba ~5 s tras soltar la tecla; por encima,
325 ms. **Esa afirmacion era erroneous: era un artefacto de la medicion, no
una propiedad de la duracion.** Aqui esta el desmontaje, con los tres
experimentos que lo desmienten.

### 6.1 El barrido de duraciones

Se corto la misma grabacion en prefijos de 10 a 22 s. El contenido de voz es
identico (es un prefijo); lo unico que cambia es la duracion
(`scripts/make-length-sweep.py`).

| Audio (s) | min | p50 | p95 |
|---|---|---|---|
| 10 | 294 | 515 | 9 979 |
| 12 | 216 | 378 | 379 |
| 13 | 406 | 541 | 1 205 |
| 14 | 700 | 710 | 1 766 |
| 15 | 171 | 255 | 316 |
| 16 | 169 | 200 | 597 |
| 17 | 151 | 2 850 | 3 132 |
| 18 | 176 | 366 | 4 227 |
| 20 | 673 | 1 384 | 5 123 |
| 22 | 242 | 467 | 2 242 |

**No hay umbral.** El minimo esta entre 151 y 700 ms en todas las
duraciones, incluidas las de 10 y 13 s que antes se median como lentas. Y
la dispersion es enorme: los 15 s dan 255 ms de mediana y los 17 s dan
2 850 ms.

### 6.2 El mismo audio, con y sin carga

La misma grabacion de 22,34 s, el mismo modelo, la misma sesion, cinco
repeticiones cada vez, ocupando 0, 2 o 4 nucleos con un proceso de carga
controlado:

| Carga de la maquina | min | p50 | p95 | max |
|---|---|---|---|---|
| 0 nucleos | 273 | 429 | 436 | 436 |
| 2 nucleos | 298 | 393 | **18 891** | 18 891 |
| 4 nucleos | 319 | 469 | **68 806** | 68 806 |

**La mediana no se mueve; la cola se dispara.** Y el "model compute" que
registra la app explica por que:

| Carga | Compute del modelo (ratio sobre tiempo real) |
|---|---|
| 0 nucleos | 5,90-8,73 s (2,56x-3,79x) |
| 2 nucleos | 39,51 s y 40,30 s (0,55x-0,57x) en dos de las cinco |
| 4 nucleos | 90,21 s (0,25x) en una de las cinco |

Cuando el ratio baja de 1, el modelo **no llega a seguir a la voz** y el
retraso se descarga de golpe al soltar.

### 6.3 El mecanismo, verificado en el codigo nativo

Un subagente leyo la ruta cache-aware de `transcribe-cpp-sys-0.4.0`:

1. El motor decodifica en fragmentos de ~1,1 s de audio. Su coste por
   fragmento es **constante** y el numero de fragmentos es proporcional al
   audio: **el compute es lineal** (`encoder.cpp:830-836`,
   `model.cpp:3579-3641`).
2. El unico minimo del camino es `chunk_advance + right_edge_margin`
   (`model.cpp:3599-3603`): con los parametros del GGUF (chunk de 105
   frames mel, margen de 2), **1 070 ms** el primer corte y 1 140 ms los
   siguientes. **No existe ningun umbral de 15, 17 o 30 s en el codigo.**
3. La app encola los tramos de audio **sin limite** (`transcription.rs:181`)
   y el `Finalize` se encola **detras** de todos (`transcription.rs:848`):
   la espera tras soltar es el vaciado del retraso pendiente.
4. Al finalizar, el motor mete **todo** el audio pendiente en **un solo
   fragmento** (`model.cpp:3774`), cuya atencion es cuadratica en lo
   pendiente. Ahi esta el super-lineal que castiga al que se retraso.

**El mecanismo completo**: si el equipo llega, el retraso es cero y la
espera son ~400 ms; si no llega, el retraso crece y al final se paga de
forma super-lineal. El disparador es **la capacidad de la maquina en ese
momento**, no la duracion del dictado.

### 6.4 Lo que queda sin medir

El numero de pasos del encoder (`chunk_step`) existe en el nativo
(`parakeet.h:225`) pero **no se exporta** por la API publica
(`transcribe.h:2119-2129`). Para medirlo habria que tocar el motor.

Lo que si exporta el motor y Dixi no lee en la ruta live: los tiempos de
mel, codificador y decodificador (`transcribe_get_timings`,
`transcribe-asr.cpp:1827-1828`), y el `buffered_ms` /
`audio_committed_ms` que la app ya registra en el log.

## 7. FASE E — Exactitud

**WER = NOT_MEASURED. CER = NOT_MEASURED.**

Las siete grabaciones no tienen transcripcion de referencia humana: son
dictaciones espontaneas, no la lectura de un guion. Sin referencia no hay
WER ni CER que no sean inventados, y **no se usa la salida de Whisper como
referencia verdadera para puntuar Nemotron**: eso compararia un modelo
contra si mismo con distinto reconocedor de idioma.

El colector (`scripts/collect-voice-validation.py`) ahora distingue los dos
casos: si una dictacion se parece a alguna frase del guion con una
desviacion menor del 45 %, se puntua; si no se parece a ninguna, se queda
en `NOT_MEASURED` en vez desacarle un numero. Con las siete grabaciones
actuales, las dos quedan sin puntuar, que es lo correcto.

Lo que si se puede comparar, sobre las mismas siete grabaciones:

| Criterio | Whisper Base | Nemotron 3.5 |
|---|---|---|
| Salida en espanol (no traduccion) | Si | Si (el motor no traduce: `supports_translate=false`) |
| Cola de texto inventado al final | Si en la muestra de 22 s (cola en griego) | No |
| Alucinaciones del tipo "Thank you" que nadie dijo | Si (muestra de 37 s) | No |
| Mayusculas y puntuacion | Irregular | Regular |
| Terminos tecnicos en ingles | "clavufla R2", "Clouf R2" | "Clawflow R2", "Cloud R2" |
| Es panlatino (voseo, lunfardo) | Parcialmente ("pushea" -> "puse a") | Parcialmente ("pushea" -> "pushea") |

La comparacion honesta es de **calidad subjetiva sobre siete muestras**, no
de exactitud estadistica.

**La sesion guiada esta preparada** (`docs/validation-voice/README.md` y
`docs/validation-voice/frases-voz.txt`): diez frases rioplatenses con
terminologia tecnica en ingles y su texto de referencia. El colector une la
traza de latencia, la entrada del historial y el WAV, y calcula el WER y el
CER de cada diccion. **No se ha grabado audio nuevo**: la sesion la hace la
persona, con el guion delante.

El WER publicado por el propio modelo (FLEURS, Q8_0) es **es = 6,3** para
Nemotron, frente al 65,9 medido con Whisper Base en el corpus sintetico.
Ese 65,9 no es comparable: como se mostro en la seccion 4, el corpus
sintetico hace que Whisper alucine, y con el idioma forzado la salida sale
en japones. Sobre voz real Whisper Base transcribe bien. **Ninguna de las
dos cifras dice cual es mejor para tu voz: eso lo decide la sesion guiada.**

---

## 8. FASE F — Estabilidad y regresiones

Descargar y medir Nemotron no puede cambiar el uso cotidiano. Comprobado:

| Que se comprueba | Resultado |
|---|---|
| Modelo activo sin cambios | `canary-180m-flash` sigue seleccionado |
| Preferencias del usuario sin cambios | `selected_language = es`, `remote_desktop_*` sin tocar |
| Historial sin tocar | Sin entradas anadidas ni modificadas por la evaluacion |
| Atajos sin tocar | Los mismos atajos; el modo propuesto no esta implementado |
| Suite Rust completa | **576 PASS, 0 FAIL**, 20 ignorados |
| TypeScript (`tsc --noEmit`) | PASS |
| ESLint | PASS |
| Pruebas de frontend (3 suites) | PASS |
| Validador de traducciones | PASS, 25 idiomas, 806 claves |
| Pruebas de descarga de modelos | PASS (reanudacion, SHA-256 y descarte de parciales) |
| Catalogo coherente con el binario | 69 modelos sembrados |
| Recuperacion ante error del motor | El motor aborta con `TRANSCRIBE_ERR_UNSUPPORTED_LANGUAGE` si el idioma no esta en su tabla: no sigue en silencio |
| Integracion de mRemoteNG | Sin tocar: `remote_desktop_enabled=false`, `remote_desktop_auto_mode=false` |

Lo que **no** se toco: la carpeta de modelos del usuario (solo se anadio un
fichero), el historial, los atajos, el VAD, el diccionario, los snippets,
los perfiles, el posprocesado con MiniMax, la insercion de texto, el
portapapeles y la interfaz de configuracion.

El unico cambio de comportamiento en toda la evaluacion es el nuevo
`--download-model`, que es una herramienta de linea de comandos y no altera
la aplicacion.

---

## 9. FASE G — Temporales: inventario, sin borrar nada

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

## 10. Veredicto corregido

**PROMISING**, con una correccion importante respecto a la evaluacion
anterior: **no hay un umbral de duracion**. Lo que hay es un margen de
capacidad de la maquina.

### Lo que se demuestra

1. **Nemotron hace streaming de verdad en Dixi** (seccion 3): Whisper no
   tiene `stream_begin`, parakeet si, con reutilizacion de la cache entre
   pasos.
2. **En las 35 ejecuciones, Nemotron streaming gana en todos los cortes**:
   p50 de 389 ms contra 1 864 ms de Whisper (4,8x menos), y tambien en el
   subconjunto corto (424 ms contra 1 199 ms). El p95 tambien: 1 749 ms
   contra 7 206 ms.
3. **El texto es igual o mejor** en las mismas siete grabaciones (seccion 7).
4. **Por lotes Nemotron es mas lento** (p50 de 3 834 ms de inferencia): la
   ganancia esta en el momento de decodificar, no en el motor.

### Lo que lo matiza

1. **La cola adversa es brutal si el equipo no llega**: con 2 o 4 nucleos
   ocupados, una repeticion de 22,34 s tardo 18,9 s y 68,8 s en entregar el
   texto. Whisper no tiene ese modo de fallo: su espera es el coste de
   decodificar y crece de forma proporcional.
2. **No hay un umbral fijo, pero si un punto de inflexion en la capacidad**:
   el motor tiene que ir a mas de 1x tiempo real para que el retraso no
   crezca. En este equipo el margen es de 1,6x a 5,1x segun la carga.
3. **La exactitud no esta medida**: WER y CER son `NOT_MEASURED` porque las
   siete grabaciones no tienen referencia humana.
4. **Cuesta 1,2 GB mas de RAM** y **1,2 s mas de carga en frio**; cubre 28
   idiomas en vez de 99 y **no traduce a ingles**.

### La conclusion operativa

El modo prolongado es mejor **en la mediana y en el p95, a cualquier
duracion**, pero su peor caso es mucho peor. Eso hace que la decision no
sea "cual modelo" sino **"que se hace cuando el equipo no llega"**, y la
respuesta esta en la seccion 6: acotar el retraso pendiente o, al menos,
hacer visible que el texto ya esta escrito mientras se habla (la vista
previa en vivo ya existe).

La propuesta de los dos modos, con los riesgos de mantener los dos motores
cargados y lo que hay que decidir, esta en
`docs/dixi-asr-modes-proposal.md`. **No esta implementada.**

### Lo que sigue pendiente

1. **La sesion guiada** (`docs/validation-voice/README.md`): es lo que
   convierte esto en una recomendacion con numeros de exactitud.
2. **Acotar el retraso pendiente** en el motor, o medir cuanto audio se
   pierde al hacerlo.
3. **Exponer el `chunk_step`** del motor para poder medir los recalculos.
4. **La limpieza de los 605 MB** de temporales, con autorizacion.
