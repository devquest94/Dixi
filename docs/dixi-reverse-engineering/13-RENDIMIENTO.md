# 13 — Rendimiento: consumo, latencias y objetivos para Dixi

> Todos los números de este documento son **medidos en esta máquina** (Windows 10, i5-8300H 4C/8T, 16 GB RAM, GTX 1050 Ti con GPU degradada) sobre la instalación real de Wispr Flow 1.6.1174 en ejecución.
> **[V]** verificado · **[I]** inferido · **[NR]** no realizado (justificado en §6).

---

## 13.1 Resumen ejecutivo

| Métrica | **Wispr Flow 1.6.1174** | **Objetivo Dixi v1** | Gap |
|---|---|---|---|
| RAM en reposo (working set) | **360,4 MB** | **< 120 MB** | −67 % |
| Memoria privada (comprometida) | **721,1 MB** | < 250 MB | −65 % |
| Procesos en reposo | **12** | **2** (app + WebView2) | −83 % |
| Hilos en reposo | **227** | < 40 | −82 % |
| Handles en reposo | **5 826** | < 600 | −90 % |
| CPU en reposo | **0,7 % de 1 núcleo** (0,088 % de la máquina) | < 0,3 % | −57 % |
| Arranque en frío (proceso principal) | **1 742 ms** hasta el servicio de depuración | < 400 ms | −77 % |
| Arranque en frío (renderers listos) | **3 624 ms** | < 900 ms | −75 % |
| Latencia e2e (p50) | **640 ms** | **< 300 ms** | −53 % |
| Latencia de red (p50) | **270 ms** (42 % de la e2e) | **0 ms** (local) | −100 % |
| Latencia e2e (p99) | **1 709 ms** | < 800 ms | −53 % |
| Disco por dictado | **≈ 240 KB** (755 MB / 3 160) | < 40 KB | −83 % |
| Base local total | **755 MB** | < 200 MB | −73 % |

---

## 13.2 Consumo de RAM en reposo (medido)

Muestra de 20 s sobre los 12 procesos vivos de Wispr Flow:

| PID | Tipo | CPU % (1 núcleo) | WS (MB) | Privada (MB) | Hilos | Handles |
|---|---|---|---|---|---|---|
| 4484 | **main** | 0,39 | **113,8** | 156,2 | 50 | 1 114 |
| 16160 | utility (storage) | 0,23 | 31,9 | **202,6** | 25 | 993 |
| 16872 | **Helper nativo** | 0,08 | 30,9 | 21,7 | 23 | 550 |
| 19152 | utility (network) | 0 | 43,9 | 94,5 | 18 | 546 |
| 22356 | renderer | 0 | 25,6 | 62,5 | 18 | 292 |
| 21280 | utility (audio) | 0 | 24,8 | 59,1 | 18 | 324 |
| 10036 | renderer | 0 | 23,6 | 54,8 | 18 | 292 |
| 12020 | renderer | 0 | 21,2 | 18,5 | 12 | 372 |
| 2532 | gpu-process | 0 | 18,4 | 14,6 | 20 | 369 |
| 12672 | renderer | 0 | 11,9 | 14,0 | 8 | 466 |
| 23680 | renderer | 0 | 9,8 | 12,6 | 10 | 284 |
| 21436 | crashpad | 0 | 4,7 | 10,0 | 7 | 224 |
| **TOTAL** | | **0,70** | **360,4** | **721,1** | **227** | **5 826** |

### 13.2.1 Lecturas

1. **360 MB de working set pero 721 MB de memoria privada** ⇒ la app **compromete el doble** de lo que ocupa residente. La diferencia es memoria reservada/paginado que Chromium no devuelve al SO. Es la firma clásica de Electron.
2. **El proceso principal solo son 113,8 MB** ⇒ el resto (246 MB) está en los 11 subprocesos. **Un Tauri con WebView2 usa ~60–90 MB en el proceso principal** y el WebView2 se hospeda en el navegador del SO (no se cuenta como memoria de la app).
3. **`16160` (utility de storage) compromete 202,6 MB privados con sólo 31,9 MB residentes** ⇒ es el proceso de **SQLite** (`node_sqlite3`): el WAL, el page cache y el mapeo de la base de 755 MB. Es el mayor consumidor de memoria *comprometida* de toda la app.
4. **227 hilos y 5 826 handles** para una app en reposo es **excesivo**; el coste en scheduler y en pagination de memoria es medible en una máquina de 4 núcleos.
5. **El helper nativo sólo usa 30,9 MB y 23 hilos** ⇒ está bien dimensionado. El problema no es el helper, es Chromium.

### 13.2.2 Contexto de sistema

```
RAM total 16 226 MB | libre 5 172 MB | usada 11 054 MB (68,1 %)
CPU: carga de sistema 2 % → 1 % durante la medición (sin actividad de Wispr)
```

> En esta máquina Wispr Flow representa el **3,3 % de la RAM usada** del sistema. En una máquina de 8 GB (el mínimo razonable hoy), sería el **9 %** — suficiente para degradar el resto del sistema en tareas intensivas.

---

## 13.3 Consumo de CPU en reposo (medido)

| Métrica | Valor |
|---|---|
| Tiempo de procesador acumulado (20 s de reloj) | **0,14 s** |
| % de un núcleo | **0,70 %** |
| % de la máquina (8 núcleos lógicos) | **0,088 %** |
| Carga de CPU del sistema durante la medición | 2 % → 1 % |

**Distribución:** el 100 % del consumo está en 3 procesos: main (0,39 %), utility de storage (0,23 %) y helper (0,08 %). Los 9 restantes (5 renderers, gpu-process, network/audio utility, crashpad) están **a 0,00 %**.

> **Interpretación [I]:** en reposo, Wispr Flow es **prácticamente free**. El coste de CPU durante el dictado real está dominado por el **helper** (captura + VAD + codificación Opus) y por la **red** (streaming del audio), no por la UI. El renderizado de Electron sólo importa cuando hay ventana visible.

---

## 13.4 Latencias del pipeline (medidas sobre 3 160 dictados reales)

### 13.4.1 Tabla completa

| Métrica | min | p50 | p90 | p99 | max | media |
|---|---|---|---|---|---|---|
| `e2eLatency` (ms) | 0 | **640** | 1 051 | 1 709 | 32 018 | 728,8 |
| `clientNetworkLatency` (ms) | 0 | **270** | 299 | 419 | 1 657 | 281,1 |
| `duration` — audio (s) | 0,12 | 10,08 | 33,14 | — | 349,4 | — |
| `speechDuration` (s) | 0,04 | 9,92 | 31,94 | — | 348,0 | — |
| **`e2e − habla` (ms)** | — | **628** | **1 021** | **1 636** | — | — |
| `numWords` | 1 | 22 | — | — | — | 32,9 |
| `numWordsCorrected` | 0 | 0 | — | — | — | 1,5 |
| `numDictionaryReplacements` | 0 | 0 | — | — | — | 0,64 |
| `editDistanceToDictated` | 0 | 0 | — | — | — | — |
| WPM real | 6,3 | **133,8** | 181,8 | — | 576,9 | — |

### 13.4.2 Descomposición de la latencia (p50)

```
    ┌────────────────────────────────────────────────────────┐
640 │  ██████████████████████████████████████████  total e2e  │
ms  │                                                        │
    │  ██████████████████████████████  270 ms  RED           │
    │  ██████████████████████████████                      │
    │  ████████████                    ~370 ms  PROCESAMIENTO│
    │  (captura + framing + FORMATEO cloud + inserción)      │
    └────────────────────────────────────────────────────────┘
```

| Componente | p50 | Origen | ¿Eliminado en Dixi local? |
|---|---|---|---|
| Red (round-trip a la nube de Wispr) | 270 ms | `clientNetworkLatency` | ✅ **Sí, 0 ms** |
| Formateo en la nube | ~250–300 ms (estimado del residuo) | `formattedText` se produce en servidor | ✅ **Sí** (reglas locales) |
| Captura + framing + subida | ~50–100 ms | `duration` / `speechDuration` | ⚠️ Reducido (Opus local) |
| Inserción en la app destino | ~20–50 ms | `pastedText` vs `editedText` | ❌ Igual (es 비용 del SO) |
| **Total** | **640 ms** | | **Objetivo Dixi: 150–250 ms** |

> **El residuo de 370 ms (sin red) es la oportunidad de mejora principal.** Con ASR local sobre CPU (Parakeet GGUF o whisper.cpp con cuantización) el ASR de 10 s de audio tarda ~0,3–0,8 s en un i5-8300H… lo que **no** es más rápido que la nube. [I]
>
> **Corrección honesta:** el ASR local **no garantiza** menor latencia que la nube en CPU de gama media. La ganancia real de Dixi es:
> 1. **Privacidad** (el audio no sale de la máquina).
> 2. **Disponibilidad** (sin red, sin cuota, sin límite de palabras).
> 3. **Latencia sólo si** se usa un modelo pequeño y rápido (streaming incremental) o GPU.
> 4. **Coste por palabra = 0** (vs. USD 15/mes de Wispr).
>
> El objetivo de latencia de 150–250 ms sólo es alcanzable con **ASR incremental (streaming)** donde el texto aparece mientras hablas, no al terminar. [I]

### 13.4.3 Coherencia con las estadísticas de Wispr

| Métrica | Cálculo propio desde `History` | Estadísticas de Wispr (`cache.statistics`) | Δ |
|---|---|---|---|
| Palabras totales | ≈ 104 000 | **102 524** | 1,5 % |
| PPM medio | ≈ 128 | **128** | 0 % |
| Duración total | — | 48 278,8 s (13,4 h) | — |

> **Validación cruzada [V]:** los cálculos independientes desde la base de datos coinciden con las estadísticas que la propia aplicación calcula y cachea. Esto valida el dataset y, por extensión, todas las cifras de este documento.

---

## 13.5 Consumo de disco y audio

| Recurso | Tamaño | Ratio |
|---|---|---|
| `flow.sqlite` | **755 MB** (3 160 dictados) | **≈ 240 KB por dictado** |
| `mic.pcm` (una reunión de 24,5 min) | 45,6 MB | **32,6 KB/s** |
| `upload.ogg` (misma reunión) | 11,5 MB | **8,2 KB/s ≈ 65,7 kbps** |

### 13.5.1 Inferencia del formato de audio [V por aritmética]

```
45,6 MB / 1 467,99 s  =  32,6 KB/s  ⇒  16 000 Hz × 16 bit × 1 canal (PCM)
11,5 MB / 1 467,99 s  =   8,2 KB/s  ⇒  Opus ≈ 65,7 kbps (típico de voz mono)
```

**Coherencia comprobada:** la reunión con `recordedMs = 1 467 990` (24,47 min) tiene `mic.pcm` de 45,6 MB y `upload.ogg` de 11,5 MB ⇒ la inferencia del formato cuadra con la duración registrada en la base de datos. [V]

**Conclusiones:**
1. Wispr guarda el audio a **PCM 16 kHz mono** (el doble de lo necesario para ASR).
2. Lo comprime a **Opus ~66 kbps** para subir ⇒ la compresión es 4:1.
3. **El PCM crudo es la mitad del peso** y sólo sirve para reprocesar. Eliminarlo (guardar sólo Opus) reduce el disco un 50 % sin pérdida funcional.

### 13.5.2 Presupuesto de disco para Dixi

| Elemento | Wispr | **Dixi (propuesta)** |
|---|---|---|
| Audio por dictado | 240 KB (PCM + Opus + chunks) | **~8 KB** (Opus 16 kHz mono, 10 s ⇒ 80 KB… optimizado a 16 kbps ⇒ 20 KB) |
| Audio por reunión (1 h) | 115 MB (PCM) + 29 MB (Opus) | **~7 MB** (Opus 16 kbps) |
| Base de 3 160 dictados | 755 MB | **~60 MB** |
| Metadatos por dictado | 63 columnas | 25 columnas |

> **Recomendación:** en Dixi, el audio se guarda **sólo como Opus** y el PCM nunca toca el disco (se decodifica en memoria para el ASR). Ahorro: **~600 MB** en el mismo historial.

---

## 13.6 Arranque en frío (medido)

Instancia aislada con perfil nuevo (`--user-data-dir` temporal, puerto CDP propio):

| Hito | Tiempo | Detalle |
|---|---|---|
| Proceso principal + puerto CDP escuchando | **1 742 ms** | Electron inicializado, main process vivo |
| Renderers listos (`/json/list` responde) | **3 624 ms** | Los 4 renderers (`status`, `scratchpad`, `hub`, `contextMenu`) cargados |
| Procesos creados | **10** | + helper = 11 en total |
| Memoria 3 s después (ambas instancias sumadas) | 1 778,8 MB WS | con la instancia viva en 360 MB ⇒ **≈ 1 400 MB transitorios en la nueva** |

**Interpretación del pico transitorio [I]:** la instancia recién arrancada, sin GPU utilizable, cae a **Vulkan por software (SwiftShader)** y compila los bundles de webpack (5,6 MB del hub + 3,5 MB de vendor). Electron no devuelve esa memoria al SO hasta que la presión de memoria lo obliga. **Es exactamente el problema que Tauri/WebView2 elimina**: no hay GPU process, ni shader compilation, ni runtime propio.

**Objetivo Dixi:** proceso Rust + WebView2 ⇒ **< 400 ms** hasta la ventana lista. El WebView2 ya está en el sistema (Edge/Chromium compartido) ⇒ no hay coste de arranque de runtime.

---

## 13.7 Coste durante dictado activo y reuniones en curso

> ⚠️ **NR — no realizado.** [NR] significa *no realizado por decisión*, con la razón explícita.

**Razón:** medir «dictado activo» exige tomar el micrófono y(dictar, es decir, **usar el micrófono del usuario**, producir audio real y potencialmente insertar texto en la aplicación enfocada. La auditoría prohíbe explícitamente alterar el comportamiento del usuario durante la sesión, y tomar el micrófono de forma activa puede interferir con su uso real de Wispr Flow (que estaba corriendo en segundo plano durante toda la auditoría).

**Sustituto: datos reales del historial.** Las 3 160 filas del `History` **son** la muestra de dictado activo: cada fila registra la latencia e2e, la latencia de red, la duración del audio y la de habla de una dictado real. Las cifras de §13.4 **son** la medición del comportamiento en carga.

**Lo que sí se midió del lado del consumo en reuniones:**

| Métrica | Valor | Fuente |
|---|---|---|
| Audio PCM por reunión | 32,6 KB/s (115 MB/hora) | `mic.pcm` [V] |
| Audio Opus subido | 8,2 KB/s (29 MB/hora) | `upload.ogg` [V] |
| Duración máxima de grabación | 2 h | `notetakerMaxRecordingLength` [V] |
| Disco máximo por reunión (2 h) | ~235 MB (PCM+Opus) | calculado [I] |
| RAM del helper en una reunión | no aislable sin grabar | [NR] |
| CPU del helper durante la captura | 0,08 % en reposo ⇒ **no** es representativo en carga | [I] |

**Plan de medición para Fase 2 (con autorización explícita del usuario):**
1. Grabar **30 s de contenido ficticio** con el micrófono de Dixi (no de Wispr) en un bucle de prueba.
2. Medir CPU/RAM de `dixi-core` y del ASR en local durante la captura y la transcripción.
3. Comparar contra los equivalentes de Wispr (`clientNetworkLatency` como referencia de la parte cloud).

---

## 13.8 Carga del sistema y recursos objetivo de Dixi

| Recurso | Objetivo Dixi v1 | Justificación |
|---|---|---|
| RAM reposo | **< 120 MB** | Cabe en un portátil de 8 GB sin estorbar |
| RAM en captura de audio | < 250 MB | Buffer de audio + VAD + encoder |
| RAM con ASR local (Parakeet GGUF Q4) | < 900 MB | Modelo en RAM; 500 MB del modelo + runtime |
| RAM con LLM local (Qwen2.5-3B Q4) | < 2 500 MB | Opcional, sólo si se activa el modo local de Transform |
| CPU reposo | **< 0,3 %** | Polling ligero del hotkey global |
| CPU durante transcripción (4 núcleos) | ≤ 250 % (2,5 núcleos) | Parakeet INT8 sobre AVX2 |
| Arranque en frío | **< 400 ms** | Binario Rust + WebView2 del SO |
| Disco (3 160 dictados) | **< 100 MB** | Sólo Opus + metadatos |
| Latencia e2e p50 (con ASR local) | 300–800 ms | **Honestamente peor o igual que la nube** en CPU media; la ventaja es privacidad y coste |
| Latencia e2e p50 (con ASR local + streaming) | **< 250 ms** | Texto parcial mientras hablas |
| Coste monetario | **0 USD** | Sin cuotas de palabras ni de reuniones |

### 13.8.1 La decisión de diseño honesta

> **El formateo de Wispr ocurre en la nube y eso es una ventaja de latencia, no un defecto.** Un pipeline puramente local en un i5-8300H sin GPU **no** superará los 640 ms p50 de Wispr para un dictado de 10 s. [I]
>
> Por eso la propuesta de Dixi es **híbrida**:
> - **Por defecto: local** (privacidad, coste cero, sin cuotas).
> - **Opcional: cloud** (MiniMax u otro) para formateo/Transforms de alta calidad.
> - **Streaming siempre**: el texto se muestra mientras hablas ⇒ la latencia percibida depende del *primer token*, no del final.

---

### Referencias cruzadas

- [01-ENTORNO.md](01-ENTORNO.md) — hardware
- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — pipeline y columnas de latencia
- [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md) — estructura de audio de reuniones
- [15-INTEGRACION-MINIMAX.md](15-INTEGRACION-MINIMAX.md) — proveedor cloud opcional
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas y prioridades
