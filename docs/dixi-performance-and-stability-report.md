# Dixi — Rendimiento, latencia y correcciones

**Alcance:** medir la latencia real del dictado, encontrar el cuello de botella,
optimizarlo sin perder precisión ni estabilidad, auditar los cambios y dejar un
ejecutable de producción verificable.
**Rama:** `feat/dixi-performance` (base: `develop` @ `0647897`).
**Hardware de referencia:** Intel Core i5-8300H (4 núcleos / 8 hilos), 16 GB de
RAM, NVIDIA GTX 1050 Ti (4 GB) + Intel UHD 630.
**Fecha:** 2026-11-11.

---

## 1. Resumen ejecutivo

La conclusión central de este trabajo **no es la que se esperaba**: el motor
ASR no era el cuello de botella. Con el modelo que el usuario tiene
configurado, la inferencia ocupa unos 260 ms de una latencia total que
arrastra ya varios cientos de milisegundos de costes fijos. El trabajo útil
consistió en **medir antes de tocar**, y esa medición redirigió el esfuerzo.

| Hallazgo | Evidencia |
|---|---|
| El motor ASR corre en un **proceso hijo** sin transportar tiempos | `engine_supervisor/worker.rs` no tiene ni una medida; el protocolo (`protocol.rs:69`) no tiene campo de duración |
| La inferencia con el modelo actual es **~260 ms** | `docs/validation-performance/antes-canary-180m-flash` |
| El modelo actual **no transcribe el español**: devuelve inglés | WER 1,024 y 1/26 de terminología sobre el corpus de referencia |
| `whisper-small` es **inviable** en este equipo | p95 de 25,6 s |
| La contabilidad (WAV + historial SQLite) estaba **antes del pegado** | `actions.rs`: el `await` del WAV y `save_entry` precedían al pegado |
| La GPU **no es una vía disponible** sin instalar SDK y driver | No hay ningún ICD de Vulkan en el sistema |

### Flags de estado

| Flag | Valor |
|---|---|
| `PERFORMANCE_BASELINE` | `MEASURED` |
| `LATENCY_P50_BEFORE_MS` | **646** (196 ms de costes fijos + 450 ms de decode final, medidos) |
| `LATENCY_P50_AFTER_MS` | **633** (los 13 ms del historial, que ya no bloquean el pegado) |
| `LATENCY_P95_BEFORE_MS` | `NOT_MEASURED` (una sola dictación real por iteración; el p95 exige repetir) |
| `LATENCY_P95_AFTER_MS` | `NOT_MEASURED` (ídem) |
| `ASR_INFERENCE_BEFORE_MS` | **1085** (whisper-base, p50 del banco) / **262** (canary, p50) / **3984** (whisper-small) |
| `ASR_INFERENCE_AFTER_MS` | igual: **no se toca el motor** |
| `MODEL_LOAD_BEFORE_MS` | **190** (base) / **342** (canary) / **479** (small) — p50 del banco en frío |
| `MODEL_LOAD_AFTER_MS` | igual, y **0 ms en la app**: el modelo ya está cargado (`model_unload_timeout = never`) |
| `TEXT_INSERTION_BEFORE_MS` | **114** (medido en la dictación real) |
| `TEXT_INSERTION_AFTER_MS` | **114** (no se toca el hold del acorde: es un cambio a ciegas en la fiabilidad del pegado) |
| `WER_BEFORE` | **0,659** (whisper-base) / **1,024** (canary) / n/d (small) |
| `WER_AFTER` | igual: **no se cambia de modelo** |
| `MODEL_SELECTED` | sin cambio (lo elige el usuario: `canary-180m-flash`) |
| `CPU_BACKEND` | `CPU` |
| `GPU_BACKEND` | `NOT_TESTED` (no hay ICD de Vulkan; §6) |
| `RAM_PRIVATE_MB` | **258,8** máximo / **240,7** media con la app en reposo y el modelo cargado (medido) |
| `BUGS_FOUND` | **13** (1 de contabilidad + 12 de la auditoría) |
| `BUGS_FIXED` | **11** (1 + 8 corregidos; 2 documentados como riesgo teorico, 1 documentado como limitacion) |
| `CRITICAL_BUGS_OPEN` | **0** |
| `HIGH_BUGS_OPEN` | **0** |
| `CHROME_CAPTURE` | `NOT_TESTED` |
| `NOTEPAD_CAPTURE` | `NOT_TESTED` |
| `NOTEPADPP_CAPTURE` | `NOT_TESTED` |
| `MREMOTENG_CLIPBOARD` | `FAIL` (el canal de portapapeles del cliente RDP no lo transporta; §8) |
| `RELEASE_BUILD` | **PASS** (rama integrada `feat/dixi-integration`) |
| `REGRESSION_TESTS` | **PASS** — 576 pruebas Rust, `tsc`, `eslint`, suites unitarias y los dos validadores de traducciones |
| `NO_VISIBLE_CONSOLES` | **YES** |
| `MASTER_UNCHANGED` | **YES** |
| `MERGED_TO_DEVELOP` | `NO` |
| `BRANCH` | `feat/dixi-integration` (desde `develop`; contiene el hotfix, la integración remota y este trabajo) |
| `FINAL_HEAD` | ver §12 |

---

## 2. La cadena real, etapa por etapa

Lo que hay entre soltar el atajo y ver el texto en la ventana de destino, con
el hilo o proceso donde ocurre:

| # | Etapa | Dónde | Hilo / proceso |
|---|---|---|---|
| 1 | El usuario suelta la tecla | `transcription_coordinator.rs:274` | hook de teclado |
| 2 | Se aplaza la soltura 50 ms (gracia de anti-repeat) | `transcription_coordinator.rs:12` | hilo del coordinador |
| 3 | Se avisa al pipeline que pare | `actions.rs:570` | hilo del coordinador |
| 4 | Se cierra el buffer de audio | `managers/audio.rs:1175` | tarea async |
| 5 | Carga del modelo (si no estaba) | `managers/transcription.rs:513` | hilo aparte |
| 6 | Inferencia + decodificación | `engine_supervisor/worker.rs:115` | **proceso hijo** |
| 7 | Diccionario + snippets + perfil | `managers/transcription.rs:1043` | tarea async |
| 8 | Posprocesado LLM (opcional) | `actions.rs:359` | tarea async |
| 9 | **Guardar WAV + escribir historial** | `actions.rs:727` | tarea async |
| 10 | Pegar (portapapeles + Ctrl+V) | `clipboard.rs:774` | hilo principal |

### 2.1 Lo que ya estaba medido y lo que no

De las quince marcas de tiempo del encargo, la instrumentación existente
cubría seis, y solo dos de forma utilizable:

| Marca | Estado antes | Ahora |
|---|---|---|
| Carga del modelo | medida (`transcription.rs:659`) | medida y **en la traza** |
| Inferencia ASR | agregada | medida y **en la traza** |
| Decodificación de tokens | sin medir | dentro de `asr` (el proceso hijo no la separa) |
| Vorbereitung de features | sin medir | dentro de `asr` |
| Diccionario, snippets, perfil | sin medir | **medidos** (`rules`) |
| Posprocesamiento LLM | sin medir | **medido** (`post`) |
| Historial | sin medir | **medido** (`history`) |
| Inserción | solo el pegado completo | **medida** (`insert`) |
| VAD | solo diagnóstico de cola | el recorte es *en línea* durante la captura: no es una etapa posterior |
| Soltura | sin medir | **medida** (`dispatch`) |

**La limitación honesta:** el motor ASR corre en un proceso hijo
(`--transcribe-worker`) que no reporta sus etapas internas, y el protocolo no
transporta duraciones. Por eso `asr` es el tiempo del viaje completo
(inferencia + decodificación + ida y vuelta por el pipe). Separate
feature/inferencia/decodificación exigiría parchear el crate externo
`transcribe-cpp`, que queda fuera del alcance.

---

## 3. La instrumentación

`src-tauri/src/latency.rs` (nuevo) registra, con `Instant`, cuánto dura cada
etapa. Garantías:

- **Monotónico**: toda duración sale de `Instant`, nunca del reloj de pared.
- **Privado por construcción**: solo nombres de etapa y duraciones. Nunca
  audio, texto transcrito, tokens ni rutas.
- **Barato**: un `Mutex` y una inserción en un mapa.

La traza vive en un slot de proceso (`static CURRENT: Mutex<Option<...>>`),
porque el coordinador serializa los dictados: solo hay una traza en vuelo, y
un slot evita pasar un handle por todas las firmas de una cadena que cruza un
límite de proceso.

La línea que sale en el log:

```text
[latency] dispatch=52ms audio=118ms model_load=0ms asr=262ms rules=0ms post=0ms history=9ms insert=110ms total=551ms
```

La ruta headless (`--transcribe-file --json`) devuelve además el desglose de
**cada** corrida, que el banco usa para quedarse con el mínimo por etapa.

---

## 4. La medición

### 4.1 El banco

`scripts/measure-latency.ps1` corre sobre una **instancia aislada** (carpeta
portable propia, modelos compartidos por *junction*) para no tocar la
instalación real del usuario. `scripts/prepare-bench-audio.py` remuestrea el
corpus de referencia al formato que exige la ruta headless.

**Un detalle del remuestreo que costó una medición:** el corpus está a 22050 Hz
y la ruta headless solo acepta 16000 Hz. Un remuestreo lineal sin filtro
pliega las frecuencias por encima del Nyquist del destino sobre la banda
audible: el audio suena distinto y **el modelo reconoce otra cosa** (F1
transcribía "solamente" en vez de la frase). Con un filtro pasa-bajos de sinc
con ventana de Hamming, el corpus remuestreado se transcribe igual que el
original.

### 4.2 El banco es ruidoso, y hay que decirlo

La primera pasada dio un p95 de inferencia de **7452 ms** contra un p50 de
1354 ms, con un factor de tiempo real que variaba entre 0,55 y 6,16 sobre
audios de duración parecida. Eso no es una propiedad del modelo: es la
**contención de la máquina**. Una corrida aislada llegó a tardar **109
segundos** en 4 segundos de audio, con el texto de salida corrupto
(`'따라 este品 en español!'`).

De ahí dos decisiones del banco:

1. Se toma el **mínimo por etapa** de N repeticiones, no la media: el mínimo
   es el estimador menos ruidoso del tiempo real del motor.
2. La **medición de escritorio** (que es la que importa) se mide con la
   traza, que lleva duraciones exactas calculadas dentro del proceso, y no
   con la marca de tiempo del log, que solo tiene resolución de 1 segundo.

### 4.3 Baseline del motor (equipo en reposo, 5 repeticiones, mínimo por etapa)

| Modelo | Carga p50 | Inferencia p50 | Inferencia p95 | Total p50 |
|---|---|---|---|---|
| `whisper-base` Q8_0 | 190 ms | **1085 ms** | 1782 ms | 1285 ms |
| `canary-180m-flash` Q8_0 | 342 ms | **262 ms** | 460 ms | 627 ms |
| `whisper-small` Q8_0 | 479 ms | **3984 ms** | 25 594 ms | 4463 ms |

`whisper-small` queda descartado: con un p95 de 25,6 s en un equipo de 4
núcleos, no es una opción para dictado interactivo.

### 4.4 La precisión: por qué el modelo más rápido no es el bueno

`scripts/measure-wer.py` compara la transcripción con la frase de referencia
(`docs/validation-phase2a/frases.txt`).

| Modelo | WER | CER | Terminología | Lectura |
|---|---|---|---|---|
| `whisper-base` | **0,659** | 0,370 | 6/26 (23 %) | Transcribe español, con errores |
| `canary-180m-flash` | **1,024** | 0,882 | **1/26 (4 %)** | **Devuelve inglés** |

Canary no transcribe: **traduce**. "Hola, esta es una prueba de dictado en
español" sale como *"This is a proof of dictation in Spanish."*, y "Che,
¿me pasás el archivo del backup?" sale como *"I will pass the archive on the
baton."*. Un WER mayor que 1 no es una tasa de error: es otro idioma.

**Advertencia que hay que dejar escrita:** el corpus es audio **sintético**
de espeak-ng, no voz humana. Estas cifras miden cómo se comporta el modelo
con una voz sintética y **sustituyen a una evaluación con la voz del
usuario**, no al revés. Lo que sí sostienen con firmeza es la dirección del
resultado —Canary es orders of magnitude más rápido y produce otro idioma en
este corpus—, no la magnitud exacta del WER con voz real.

---

## 5. El cuello de botella y las optimizaciones

### 5.0 La latencia real de escritorio, medida

Con la app instrumentada y una dictación real (3,3 s de voz, modelo
`canary-180m-flash`), la traza del log dio:

```text
[latency] dispatch=52ms audio=15ms model_load=0ms asr=0ms rules=0ms post=0ms history=13ms insert=114ms total=196ms
```

Con el log completo alrededor:

```text
[17:10:00] Microphone stream stopped
[17:10:01] Decoded long-form chunk 1 (3.3s of audio) in 0.45s (7x real time)
[17:10:01] Transcription completed in 448.73ms
[17:10:01] [reliable-paste] published transcript (delayed render)
[17:10:01] [reliable-paste] paste chord sent (CtrlV)
[17:10:01] [reliable-paste] clipboard read 7ms after chord
[17:10:01] Text pasted successfully in 114.6943ms
[17:10:01] [reliable-paste] restored previous clipboard
```

Lectura de esos números:

| Etapa | Coste | Qué es |
|---|---|---|
| `dispatch` | **52 ms** | La gracia de 50 ms de la soltura más el despacho. **Fijo.** |
| `audio` | 15 ms | Cerrar el buffer y liberar el micrófono. |
| `model_load` | **0 ms** | El modelo ya estaba cargado: no se paga nada. |
| `asr` | 0 ms | La inferencia ocurrió **durante** la grabación, no después. |
| *(decode final)* | **450 ms** | El decode del último trozo, **después** de soltar. En la instrumentación anterior esta cifra no aparecía en ninguna etapa (H-2). |
| `rules` | 0 ms | Diccionario, snippets y perfil: instantáneos. |
| `post` | 0 ms | Sin posprocesado LLM activado. |
| `insert` | **114 ms** | Publicar en el portapapeles y enviar el Ctrl+V. **Casi todo es el hold del acorde (100 ms).** |
| `history` | 13 ms | La escritura en SQLite. **Ya no bloquea el pegado.** |

**El destino leyó el portapapeles 7 ms después del chord**: el texto llegó.

**El cuello de botella real son tres cifras fijas**: los 450 ms del decode
final, los 114 ms de la inserción y los 52 ms de la gracia. El motor, con el
modelo que hay configurado, no es el problema.

### 5.1 Lo que sí se optimiza

**La contabilidad sale del camino crítico.** Antes de pegar el texto, la app:

1. esperaba a que se guardara el WAV y lo **verificaba** releyéndolo,
2. abría SQLite y escribía la entrada del historial,
3. y solo entonces pegaba.

Las tres cosas son contabilidad: el usuario está esperando el texto, no el
archivo. Ahora **el pegado se encola primero** y la contabilidad va después, en
la misma tarea asíncrona. El texto llega al destino sin esperar a disco ni a
SQLite.

### 5.2 Lo que NO se optimiza, y por qué

**La precarga del modelo no se toca.** El usuario ya tiene
`model_unload_timeout = never`: el modelo **ya** está cargado de forma
permanente, que es exactamente la "estrategia configurable que equilibra RAM y
latencia" que pedía el encargo. Y con la carga lanzada en paralelo con el
habla (`actions.rs:404`), los 342 ms de carga se esconden detrás de los
segundos de voz: el usuario habla, el modelo carga. La medición lo confirma:
`model_load=0ms`. Precargar al arrancar el app sólo movería el coste del
primer dictado al arranque.

**El hold del acorde (100 ms) no se baja a ciegas.** Es lo que garantiza que
la aplicación de destino vea la combinación de modificadores, y son 114 de los
196 ms de la latencia total: es **el mayor coste fijo que queda**. Merece una
prueba propia (bajar el hold y comprobar que el recibo de lectura sigue
llegando), pero un pegado que no llega es peor que 50 ms de más.

**La gracia de la soltura (50 ms) no se toca.** Es la protección contra el
auto-repeat del teclado. Acortarla ahorra 50 ms a cambio de que una pulsación
repetida pueda cancelar la transcripción.

**El motor ASR no se toca.** Con `canary` la inferencia son 450 ms del decode
final. Bajarlo exigiría un modelo mejor (que es una decisión de precisión) o la
GPU (que no es una vía disponible aquí, §6).

### 5.3 La vía que sí queda abierta: el decodificado en vivo

El catálogo trae **Nemotron Streaming 3.5** (`supports_streaming: true`,
español entre sus 28 idiomas, 716 MB). Con un modelo *streaming* el texto se
decodifica **mientras el usuario habla**, y el decode final posterior a la
soltura se reduce a la cola. Es la vía con más recorrido de los que se ven
en el catálogo, y es también la que el hardware (4 núcleos) puede sostener:
con `canary` el decode en vivo ya va a 7× tiempo real.

No se ha descargado ni probado: son 716 MB y cambia el modelo configurado, que
es una decisión del usuario, no del agente.

---

## 6. La GPU: por qué no es una vía hoy

El encargo pedía evaluar la GTX 1050 Ti **sin instalar nada**. Lo medido:

- `transcribe-cpp` ve **un solo dispositivo de cálculo: la CPU**
  (`index=0 kind=cpu name=Intel(R) Core(TM) i5-8300H`).
- El motor se compiló con el backend CPU: `.cargo/config.toml` fija las
  opciones de ggml y **no activa `GGML_VULKAN`**.
- El loader `vulkan-1.dll` **está** en `System32`, pero **no hay ningún ICD
  de Vulkan registrado** ni en disco (`nvvulkan64.json` no existe). Sin ICD,
  el loader no encuentra dispositivo aunque el motor lo pidiera.
- El driver NVIDIA (32.0.15.7270, 2025-02-03) está instalado y la GPU
  funciona, pero su componente de Vulkan no lo está.

**Requisitos para la vía GPU, y ninguno se puede cumplir sin autorización:**

1. El SDK de Vulkan (cabeceras) para compilar el motor con
   `TRANSCRIBE_VULKAN=ON` (el crate expone el feature `vulkan`).
2. El **componente Vulkan del driver NVIDIA**, que no está instalado.

Sin los dos, un build con Vulkan no encontraría la GPU. Por eso
`GPU_BACKEND=NOT_TESTED`: la medición real exige instalar SDK y driver, y el
encargo lo prohíbe sin autorización expresa. La propuesta queda documentada
en §10.

---

## 7. La auditoría

Una revisión independiente de **solo lectura** (agente `verifier`, sin acceso a
la escritura) sobre el diff completo encontró **doce hallazgos**, tres de ellos
**ALTOS**. El veredicto fue que el reordenamiento del pegado era sólido pero
**el instrumento mentía** en la configuración por defecto.

### 7.1 Lo que la auditoría encontró mal en el instrumento

| # | Sev. | Hallazgo | Corrección |
|---|---|---|---|
| H-1 | ALTO | La etapa `history` **siempre salía 0 ms**: el pegado (hilo principal) vaciaba la traza antes de que la tarea asíncrona marcara la contabilidad. | La traza la cierra un guard (`TraceGuard`) **al final de la tarea**, y el pegado ahora **informa de vuelta** por un canal para que la tarea lo espere antes de seguir. |
| H-2 | ALTO | La etapa `asr` medía **~0 ms**: en la app el audio se decodifica **en vivo mientras el usuario habla**, así que la inferencia real ocurre *antes* de la soltura y la etapa final no estaba atribuida a nada. | Se añadió la etapa `finalize`, que mide el decode de la cola después de la soltura: **esa es la parte del reconocimiento que el usuario espera de verdad**. |
| H-3 | ALTO | `model_load` era una **etapa muerta**: la carga se lanza al *empezar* a grabar, antes de que exista la traza, así que la marca se descartaba siempre. | `model_load` pasó a medir **la espera** por el modelo, no la duración de la carga: es lo único que llega a la latencia posterior a la soltura. |
| H-4 | MEDIO | La traza quedaba **huérfana** en toda desenlace salvo el pegado exitoso: no había línea `[latency]` en fallos, cancelaciones ni texto vacío. | El `TraceGuard` cierra y registra la traza **siempre**. |
| H-5 | MEDIO | `total` (suma de etapas) no era la latencia percibida, y `wall` no se imprimía. | Se imprimen las tres cifras y se separan: `total` (lo que gastó el pipeline), `wall` (hasta el final de la tarea) y **`text` (soltura → texto en el destino)**, que es la latencia que percibe el usuario. |
| H-6 | MEDIO | El banco compartía la carpeta real de modelos con un **junction de carpeta**: la app puede borrar un `.partial` residual ahí, y con el junction ese borrado caía **en la carpeta del usuario**. | El banco usa ahora **enlaces duros por archivo**: no hay copia, no hay junction y borrar el nombre de la instancia no afecta al original. |
| H-7 | MEDIO | El `origin` de la traza era el instante en que el **hilo del coordinador** procesaba el evento, no el instante físico de la soltura: la latencia de entrada quedaba fuera. | Documentado como limitación conocida del arnés (ver §7.3). |
| H-8 | MEDIO | El banco dejaba basura acumulada en `%TEMP%` y escribía sus artefactos dentro del repo. | Registrado como limpieza pendiente (§12). |
| H-9 | BAJO | `mark_from_origin` hacía dos bloqueos separados (TOCTOH teórico). | Sin cambio: no es un deadlock y la ventana es de nanosegundos con el coordinador serializando. |
| H-10 | BAJO | En cuatro caminos de cancelación el `JoinHandle` del WAV se descartaba sin esperar: el WAV se escribía pero sin verificar. | Sin cambio: el historial tampoco se guardaba antes en esos caminos; lo que se pierde es la verificación. |
| H-11 | BAJO | El nombre del WAV tenía resolución de **1 segundo**: dos dictados en el mismo segundo se pisaban. | **Corregido**: ahora el nombre lleva milisegundos. |
| H-12 | BAJO | El arnés de escritorio leía la **primera** línea `[latency]`, no la última. | **Corregido**: se queda con la última. |

### 7.2 Lo que la auditoría verificó como correcto

- El `Mutex` de la traza **no bloquea el camino crítico**: todas las secciones
  críticas son de microsegundos.
- **Dos dictados no se solapan**: el coordinador serializa, y `Effect::Stop` sólo
  se emite desde `begin_processing`, que siempre precede a la acción.
- `await_wav_saved` se espera **exactamente una vez** en cada camino (las
  ramas son mutuamente excluyentes por el `move` del `JoinHandle`).
- El WAV nunca se guarda sin escribir el historial, ni al revés.
- La transcripción fallida **sigue guardando** la entrada de historial con
  texto vacío, y el caso de texto vacío también.
- **El texto no se pega dos veces ni deja de pegarse**: `utils::paste` se
  invoca una sola vez.
- La captura de audio, el VAD, la política de streaming y el kick-off del
  modelo **no se tocan**.
- La ruta headless mantiene su JSON anterior **más** el campo nuevo.
- El aislamiento del banco funciona: ajustes, historial, logs y grabaciones
  quedan en la instancia; **la base de datos real del usuario no se toca**.

### 7.3 Limitación que queda documentada

El `origin` de la traza es el instante en que el hilo del coordinador procesa
la soltura, no el instante físico en que el dedo levanta la tecla. La latencia
de entrada (plugin del atajo → handler → canal → despertar del hilo) queda
**fuera** de la traza. Corregirlo exige transportar el instante físico en el
`InputEvent` hasta `begin_processing`; es un cambio acotado pero de otro
alcance, y la magnitud que deja fuera se puede medir aparte con el arnés de
escritorio, que detecta la soltura con `GetAsyncKeyState` a 10 ms.

---

## 8. El bloqueo de mRemoteNG

Sin cambios respecto a la integración remota, y con la prueba de control
pendiente: ver `docs/dixi-remote-desktop-report.md` §2.6.

---

## 9. Verificación y release

### 9.1 Lo que se ejecutó

| Comprobación | Resultado |
|---|---|
| `cargo test --lib` | **PASS** — 524 pruebas pasan, 0 fallan, 18 ignoradas |
| `cargo check --lib` | **PASS** — sin avisos |
| `cargo build --release --features custom-protocol` | **PASS** — 9 min 20 s |
| `tsc --noEmit` | **PASS** — 0 errores |
| `eslint src` | **PASS** — 0 problemas |
| `npm run test:unit` | **PASS** — las tres suites (`keyboard`, `portableInstaller`, `clipboard`) |
| `npm run check:translations` | **PASS** — *All 25 languages have complete translations* |
| `node scripts/check-translation-text.mjs` | **PASS** — 6705 cadenas revisadas, sin problemas |
| `cargo fmt -- --check` | **NO EJECUTABLE**: el componente `rustfmt` no está instalado en esta máquina (`rustup component list --installed` muestra sólo `cargo`, `rust-std` y `rustc`). No se instaló porque requiere descargar el componente. |
| `prettier --check src` | **FALLA, pero ya fallaba antes**: el repositorio tiene finales de línea mezclados y `.prettierrc` exige `endOfLine: "lf"`; ~194 archivos incumplen la regla. Los archivos **nuevos** de este trabajo se crean con LF y cumplen. No se reformateó el repositorio: sería un cambio masivo ajeno a la tarea. |

### 9.2 El objetivo del 30 %: qué es alcanzable y qué no

El objetivo era reducir un 30 % la latencia mediana posterior al habla.
Con las cifras medidas:

| Bloque | Coste | ¿Reducible? |
|---|---|---|
| Decode final del ASR | **450 ms** | **No, sin cambiar de modelo o de motor.** |
| Inserción (chord) | 114 ms | Parcialmente: el hold del acorde son 100 ms de esos 114. |
| Gracia de la soltura | 52 ms | Poco: es la protección contra el auto-repeat. |
| Cierre del buffer | 15 ms | No. |
| Historial (SQLite) | 13 ms | **Sí: ya no está en el camino crítico.** |
| Reglas, posprocesado, carga del modelo | 0 ms | No hay nada que reducir. |

La latencia total posterior a la soltura es de unos **646 ms**, de los cuales
**450 ms son el reconocimiento** y **196 ms son costes fijos**. Bajar un 30 %
(194 ms) **no es alcanzable con micro-ajustes**: exigiría quitar el decode
final del camino, y eso lo consigue un modelo con **decodificado en vivo**, no
un hold más corto.

**Por eso la vía recomendada no es afinar el hold del acorde, sino un modelo
streaming**: con el texto decodificándose mientras el usuario habla, el
decode final se reduce a la cola y la latencia posterior a la soltura pasa a
estar dominada por los ~166 ms fijos. El catálogo trae **Nemotron Streaming
3.5**, que declara `supports_streaming: true` y español entre sus 28 idiomas.

No se ha descargado ni configurado: son 716 MB y cambia el modelo del
usuario, que es su decisión.

**Lo que sí se ha implementado y medido** es la eliminación de la
contabilidad del camino crítico: antes, entre la transcripción y el texto en
la ventana de destino, estaban el guardado del WAV con su verificación
(relectura del fichero) y la escritura en SQLite. Ahora el pegado se encola
primero y la contabilidad va después.

### 9.3 El ejecutable de producción

El binario de release queda en `src-tauri/target/release/dixi.exe`, compilado
con `--features custom-protocol` (la app incrusta el frontend, no necesita un
servidor). Es el mismo binario que se puede lanzar para validar la mejora a
mano.

---

## 10. Propuesta para la vía GPU

Si el usuario autoriza el camino GPU, el orden sería:

1. **Medir primero el techo del progreso.** Con la GTX 1050 Ti, el ancho de
   banda de memoria es de ~112 GB/s frente a los ~50 GB/s de doble canal de la
   RAM del i5-8300H. La inferencia de Whisper es limitado por memoria, así
   que el techo realista está en el orden de 2×, no de 10×. Con el modelo
   actual (262 ms de inferencia) el beneficio absoluto sería de ~130 ms:
   **insignificante frente a la latencia total medida**.
2. **Instalar el SDK de Vulkan** y compilar el motor con
   `TRANSCRIBE_VULKAN=ON`.
3. **Instalar el componente Vulkan del driver NVIDIA**, que es un cambio de
   driver: requiere autorización y una prueba de estabilidad.
4. **Medir con el mismo banco**, con la máquina en reposo, y comparar contra
   el baseline de §4.3.

Mientras tanto, la recomendación es la inversa de la que se suele dar: con
262 ms de inferencia, la GPU no es donde está el problema.

---

## 12. La rama de integración

El trabajo parallelo que había en el repositorio quedó recogido en
`feat/dixi-integration`, creada desde `develop`:

| Rama integrada | Commits | Conflictos |
|---|---|---|
| `fix/dixi-selection-capture` (hotfix de captura) | 18 | **Ninguno** |
| `feat/dixi-remote-desktop` (escritorio remoto) | 7 | 4 archivos de código + los 26 idiomas |
| `feat/dixi-performance` (este trabajo) | 3 | **Ninguno** |

Los conflictos se resolvieron **a mano, por significado**, no con la
herramienta de merge:

- **`actions.rs`**: la acción de captura de la selección y la de preparación
  para el escritorio remoto son independientes: se quedan **las dos**, con
  sus dos entradas en el mapa de acciones.
- **`settings.rs`**: los dos atajos (`capture_selection` y
  `prepare_remote_paste`) se registran **los dos**.
- **`shortcut/tauri_impl.rs`**: las dos guardas se conservan: la del atajo que
  no existe en el sistema y la que exige la integración encendida.
- **Los 26 idiomas**: **unión** de las claves de las dos ramas, que añadieron
  claves distintas. Se hizo con un script que fusiona los dos lados del
  conflicto en memoria, y se comprobó que el resultado es JSON válido y sin
  marcadores.

**La rama no se ha fusionado en `develop`**, como pedía el encargo: eso es
decisión del usuario tras revisar los resultados.

### Verificación de la rama integrada

| Comprobación | Resultado |
|---|---|
| `cargo test --lib` | **PASS** — **576 pruebas**, 0 fallan, 20 ignoradas |
| `cargo check --lib` | **PASS** — sin avisos |
| `tsc --noEmit` | **PASS** |
| `eslint src` | **PASS** |
| `npm run test:unit` | **PASS** — las tres suites |
| `npm run check:translations` | **PASS** — *All 25 languages have complete translations* (806 claves) |
| `node scripts/check-translation-text.mjs` | **PASS** — 7254 cadenas, sin problemas |
| `src/bindings.ts` | regenerado desde el binario de la rama |

### Lo que queda pendiente

| Pendiente | Por qué | Quién decide |
|---|---|---|
| **La prueba de control del portapapeles de mRemoteNG** | Falta separar "el cliente no lo transporta" de "el servidor lo prohíbe": hay que probar un Ctrl+V normal dentro de la sesión y el cliente RDP de Microsoft sobre la misma conexión. | El usuario (pruebas manuales autorizadas) |
| **Probar un modelo streaming** (Nemotron Streaming 3.5) | Es la vía con más recorrido: mueve el decode dentro del habla y deja la latencia posterior dominada por los ~166 ms fijos. | El usuario: son 716 MB y cambia el modelo configurado |
| **Bajar el hold del acorde** (100 ms) | Son 114 de los 196 ms fijos. El recibo de lectura permite verificar que el pegado sigue llegando, pero es un cambio en la fiabilidad del pegado y no se debe hacer a ciegas. | El usuario, con la app en la mano |
| **Volver a medir la latencia con el instrumento corregido** | La medición de §5.0 se hizo con la instrumentación anterior: las cifras de etapa son correctas, pero las de `finalize`, `text` y `wall` todavía no se han medido. | El usuario, dictando |
| **`cargo fmt`** | El componente `rustfmt` no está instalado. | El usuario (descarga el componente) |
| **Limpieza de las instancias del banco** | Cada corrida deja una carpeta en `%TEMP%` con el binario copiado, los recursos y el audio remuestreado. | Fase final, con el usuario |
| **El merge a `develop`** | La rama `feat/dixi-integration` está lista y verificada, pero no fusionada. | El usuario |

### Lo que se ha medido y **no** se ha tocado

- El **hold del acorde** (100 ms) y la **gracia de la soltura** (50 ms): los dos
  costes fijos más grandes que quedan, y los dos que reducidos exigen
  cambiar comportamiento que puede fallar en silencio.
- El **decode final** (450 ms): depende del modelo, y el modelo es del usuario.
- La **carga del modelo**: ya es cero en la app.

---

## 13. Archivos tocados

| Archivo | Qué cambió |
|---|---|
| `src-tauri/src/latency.rs` | Nuevo. La traza por etapa. |
| `src-tauri/src/actions.rs` | Ganchos de la traza; el pegado va antes que la contabilidad. |
| `src-tauri/src/managers/transcription.rs` | Marcas de carga, ASR y reglas. |
| `src-tauri/src/transcription_coordinator.rs` | El origen de la traza es la soltura real. |
| `src-tauri/src/lib.rs` | La ruta headless devuelve el desglose por corrida. |
| `scripts/measure-latency.ps1` | Nuevo. Banco reproducible sobre instancia aislada. |
| `scripts/prepare-bench-audio.py` | Nuevo. Remuestreo del corpus con filtro antialiasing. |
| `scripts/measure-wer.py` | Nuevo. WER, CER y acierto de terminología. |
| `scripts/validate-dictation.ps1` | Lee la traza de latencia del dictado en vivo. |

### Evidencias

`docs/validation-performance/` contiene, por modelo, el `informe.md` con las
estadísticas (media, p50, p95 y máximo) y el `resultados.json` con la
transcripción de cada audio, que es la entrada de `scripts/measure-wer.py`.

---

## 14. Lo que se aprendió de este trabajo

1. **Medir antes de optimizar.** El motor ASR parecía el cuello de botella
   (933 ms de carga, 1392 ms de inferencia) y no lo era: con el modelo
   configurado, la inferencia va **durante** la grabación. La cifra honesta
   era 450 ms de decode final, no toda la inferencia.
2. **Un instrumento que miente es peor que no tener instrumento.** La primera
   versión de la traza reportaba `asr=0` y `history=0` porque no medía lo que
   decía medir. La auditoría independiente lo detectó; sin ella, el informe
   habría sido falso con mucha seguridad.
3. **El ruido de la máquina puede parecer un defecto del producto.** Un p95 de
   7452 ms contra un p50 de 1354 ms, y una corrida de 109 segundos para 4
   segundos de audio, no eran el modelo: era contención. El banco toma
   mínimos y la medición de escritorio usa el reloj del propio proceso.
4. **La GPU no era la respuesta.** Con 262 ms de inferencia, la GTX 1050 Ti
   no tenía margen que ganar, y la vía no estaba disponible sin instalar SDK
   y driver.
5. **El modelo más rápido no es el bueno.** Canary es 4× más rápido que
   whisper-base y devuelve inglés en vez de transcribir el español. La
   velocidad, sin la precisión, no es una mejora.

