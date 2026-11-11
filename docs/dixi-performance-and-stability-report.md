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
| `LATENCY_P50_BEFORE_MS` | pendiente de la medición de escritorio (ver §5) |
| `LATENCY_P50_AFTER_MS` | pendiente de la medición de escritorio (ver §5) |
| `LATENCY_P95_BEFORE_MS` | pendiente de la medición de escritorio (ver §5) |
| `LATENCY_P95_AFTER_MS` | pendiente de la medición de escritorio (ver §5) |
| `ASR_INFERENCE_BEFORE_MS` | **1085** (whisper-base, p50) / **262** (canary, p50) |
| `ASR_INFERENCE_AFTER_MS` | **262** (canary, p50) — sin cambio: la inferencia no se toca |
| `MODEL_LOAD_BEFORE_MS` | **190** (whisper-base, p50) / **342** (canary, p50) |
| `MODEL_LOAD_AFTER_MS` | **190** / **342** — sin cambio, y ver §4.2 |
| `TEXT_INSERTION_BEFORE_MS` | pendiente de la medición de escritorio (ver §5) |
| `TEXT_INSERTION_AFTER_MS` | pendiente de la medición de escritorio (ver §5) |
| `WER_BEFORE` | **0,659** (whisper-base) / **1,024** (canary) |
| `WER_AFTER` | **0,659** (whisper-base) / **1,024** (canary) — sin cambio |
| `MODEL_SELECTED` | sin cambio: el modelo lo elige el usuario |
| `CPU_BACKEND` | `CPU` |
| `GPU_BACKEND` | `NOT_TESTED` (no hay ICD de Vulkan; ver §6) |
| `RAM_PRIVATE_MB` | 286,6 con el modelo cargado / 281,5 descargado (Fase 2B) |
| `BUGS_FOUND` | 1 (la contabilidad en el camino crítico) + 3 de la auditoría (§7) |
| `BUGS_FIXED` | 1 + 3 |
| `CRITICAL_BUGS_OPEN` | 0 |
| `HIGH_BUGS_OPEN` | 0 |
| `CHROME_CAPTURE` | `NOT_TESTED` |
| `NOTEPAD_CAPTURE` | `NOT_TESTED` |
| `NOTEPADPP_CAPTURE` | `NOT_TESTED` |
| `MREMOTENG_CLIPBOARD` | `FAIL` (canal del cliente RDP; ver §8) |
| `RELEASE_BUILD` | pendiente (§9) |
| `REGRESSION_TESTS` | pendiente (§9) |
| `NO_VISIBLE_CONSOLES` | `YES` |
| `MASTER_UNCHANGED` | `YES` |
| `MERGED_TO_DEVELOP` | `NO` |
| `BRANCH` | `feat/dixi-performance` |
| `FINAL_HEAD` | pendiente |

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
segundos de voz: el usuario habla, el modelo carga. Precargar al arrancar el
app sólo movería el coste del primer dictado al arranque, sin mejorar la
latencia percibida.

**El tiempo de espera de la liberación (50 ms) no se toca.** Es la protección
contra el auto-repeat del teclado. Reducirlo ahorra 50 ms a cambio de que una
pulsación repetida pueda cancelar la transcripción.

**El hold del acorde (100 ms) no se toca.** Es lo que garantiza que la
aplicación de destino vea la combinación de modificadores. Acortarlo es un
cambio a ciegas en la fiabilidad del pegado.

**El motor ASR no se toca.** No hay un cambio que lo haga más rápido sin
cambiar de modelo, y cambiar de modelo es una decisión de precisión, no de
rendimiento.

### 5.3 Lo que la medición de escritorio dirá

Pendiente de ejecutar con la voz del usuario (§9).

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

_(Se completa al final.)_

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

## 11. Archivos tocados

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
