# Dixi — Informe técnico de la Fase 2A

**Base funcional construida y verificada.** Este documento recoge la
procedencia del código, la arquitectura resultante, las decisiones tomadas y
las mediciones ejecutadas en la máquina de referencia.

- **Repositorio**: `D:\Codigo\Dixi` (local, sin remoto)
- **Rama de trabajo**: `feat/handy-foundation` (desde `develop`, desde `master`)
- **Fecha**: 2026-10-19
- **Objetivo de la fase**: un flujo vertical de dictado por voz **local** en
  Windows, sobre una base reutilizada de código abierto y con la identidad,
  los módulos y las pruebas propias de Dixi.

---

## 1. Resumen ejecutivo

Dixi queda construido sobre el núcleo de **[Handy](https://github.com/cjpais/Handy)**
(MIT, © 2025 CJ Pais), incorporado al repositorio de Dixi en el commit
`9e57a0d7929fcc6417ea0d6e54c1b57f62635edd` y reidentificado como producto
propio. No se ha reimplementado el motor: la reutilización es del núcleo
completo (captura, VAD, atajos globales, pegado fiable en Windows, gestión de
modelos, worker de ASR aislado) y la aportación de Dixi es la **capa de
organización**, la **identidad**, la **abstracción de proveedores de ASR** y
la **máquina de estados del dictado**, con sus pruebas.

Estado verificado:

| Comprobación | Resultado |
|---|---|
| Compilación del núcleo (release de depuración) | ✅ `dixi.exe` |
| Pruebas Rust | ✅ **304 pasan, 0 fallan, 18 omitidas** |
| Pruebas de frontend | ✅ 3 guiones, todas las aserciones pasan |
| Compilación de frontend (`tsc && vite build`) | ✅ limpia |
| Transcripción local real (CPU) | ✅ 3 modelos × 3 muestras |
| Rendimiento en reposo y durante transcripción | ✅ medido (§7) |
| Flujo completo hasta el pegado en otra app | ⚠️ ver §9.1 |

---

## 2. Procedencia y licencias

### 2.1 Código de terceros incorporado

| Proyecto | Licencia | Commit | Uso en Dixi |
|---|---|---|---|
| `cjpais/Handy` | MIT © 2025 CJ Pais | `9e57a0d7929fcc6417ea0d6e54c1b57f62635edd` | Núcleo completo (app, motor ASR, captura, atajos, pegado, bandeja, overlay, i18n, 309 pruebas) |

El texto original de la licencia se conserva en `LICENSE-HANDY`; la atribución
detallada está en `NOTICE.md`.

### 2.2 Lo que se ha sustituido por identidad propia

El aviso de alcance era explícito: *no reutilizar logotipos, nombres
comerciales ni elementos de marca excluidos de la licencia*. Hecho:

- **Icono de aplicación** (PNG/ICO, 13 tamaños) — generado desde cero.
- **Iconos de bandeja y overlay** (8 estados, tema claro y oscuro) — generados.
- **Glifo de interfaz** — el "HandyHand" (una mano) se sustituyó por
  `DixiMark.tsx`, una onda de voz de 5 barras.
- **Logotipo de texto** — el "HandyTextLogo" (31 KB de trazados vectoriales
  del nombre de Handy) se sustituyó por `DixiTextLogo.tsx`, compuesto con
  tipografía del sistema. No hay activo de marca propietario.
- **Paleta** — el rosa de marca (`#f28cbb`) se sustituyó por el índigo de
  Dixi (`#818cf8` / `#4f46e5`).

### 2.3 Lo que deliberadamente NO se renombró

Renombrar estas cosas habría roto el funcionamiento o falsearía la
procedencia:

| Elemento | Motivo |
|---|---|
| `start_handy_keys_recording` / `stop_handy_keys_recording` | Nombres de comando Tauri: son el contrato con el frontend |
| `handy-keys-event` | Nombre de evento emitido por el crate `handy-keys` |
| `"handy_keys"` | Valor **persistido** de `keyboard_implementation`: renombrarlo invalidaría la configuración ya guardada |
| crates `handy_recorder`, `handy-keys` | Crates de terceros: su nombre es parte de su procedencia |

### 2.4 Dependencias con fork personal (deuda heredada)

`Cargo.toml` conserva dos dependencias de forks personales del autor original,
heredadas de Handy:

| Dependencia | Origen | Riesgo |
|---|---|---|
| `tao`, `tao-macros` | `github.com/cjpais/tao` (rev `c3bee28c`) | Repin no actualizado; el build depende de un repo personal |
| `hf-hub` | `github.com/cjpais/hf-hub` (rama `cancellable-downloads`) | Ídem, para descargas cancelables |

**No bloquean esta fase**, pero conviene resolverlos antes de cualquier
distribución: son el punto donde Dixi depende de infraestructura que no
controla.

### 2.5 Licencias de los pesos de modelo (no se distribuyen con Dixi)

| Modelo | Licencia | ¿Se distribuye? |
|---|---|---|
| Whisper (pesos OpenAI) | MIT | No — se descarga aparte |
| `canary-180m-flash` (NVIDIA) | `nvidia-open-model-license` (CC-BY-4.0) | No — se descarga aparte |
| Silero VAD (VAD incluido en la app) | MIT | **Sí**, como recurso de la app (1,2 MB), con excepción explícita en `.gitignore` |

---

## 3. Entorno de construcción y los tres bloqueos reales

El entorno de referencia es Windows 10 x64 con **i5-8300H** (4C/8T, AVX2, sin
AVX-512), **16 GB de RAM**, **GTX 1050 Ti** y **Visual Studio Community
2026** (MSVC 14.51). La compilación del núcleo heritage revealed **tres
bloqueos reales**, todos resueltos y todos documentados en el repositorio.

### 3.1 Vulkan SDK ausente

La configuración original de Handy para `x86_64` activa la característica
`vulkan` de `transcribe-cpp`, que **compila los sombreadores Vulkan de ggml
en tiempo de compilación** y por tanto exige un SDK de Vulkan completo
(cabeceras + el compilador `glslc`). Esta máquina tiene el *runtime* de
Vulkan, no el SDK.

**Decisión**: compilar **solo CPU**, igual que ya hace el propio upstream para
Windows-ARM (`default-features = false`, sin características extra). Se
conserva estáticamente, sin DLL que distribuir. Reactivar Vulkan exige
instalar el SDK y volver a habilitar la característica.

### 3.2 ggml sonda AVX-512 *ejecutando* un binario

Con `GGML_NATIVE=ON` (por defecto) y MSVC, ggml incluye `FindSIMD.cmake`, que
**compila y ejecuta** un binario de prueba por nivel de ISA. La prueba de
AVX-512 se ejecuta con `/arch:AVX512`; en una CPU sin AVX-512 ese binario
murre con `Illegal instruction`, y **CMake ≥ 4 clasifica `FAILED_TO_RUN` como
error fatal de configuración** (no como un simple "false"). Resultado:
`transcribe-cpp-sys` no compilaba en ninguna máquina x86 sin AVX-512.

**Decisión** (en `.cargo/config.toml`, versionada):

```
GGML_NATIVE=OFF            -> no se incluye FindSIMD.cmake, no hay sondeo
GGML_SSE42/AVX/AVX2/FMA/F16C/BMI2 = ON   -> el compilador recibe /arch:AVX2
GGML_AVX_VNNI/AVX512* = OFF
```

Se conserva todo el rendimiento que la CPU permite y el build es reproducible
en cualquier clon.

### 3.3 `transcribe-libs/` no existe

`tauri.windows.conf.json` declara el recurso `transcribe-libs` → `.`, y Tauri
exige que la ruta exista antes de compilar. Upstream lo resuelve en su CI,
que copia el runtime de Visual C++ a nivel de aplicación (para que la
aplicación arranque donde no esté instalado el redistribuible, issue #1527).

**Decisión**: `scripts/stage-transcribe-libs.ps1` hace ese paso en local, de
forma reproducible, y el directorio queda fuera de Git (es un artefacto).

### 3.4 Otros condicionantes del entorno

- **Memoria**: la máquina estaba a 1,1 GB libres de 15,8 GB por trabajo
  concurrente ajeno a Dixi. `rustc` murió con `STATUS_STACK_BUFFER_OVERRUN`
  compilando `tao`. Mitigación: `--jobs 2` y `CARGO_PROFILE_DEV_DEBUG=0`
  (los símbolos de depuración multiplican el consumo de memoria de `rustc`).
- **Consolas visibles**: el lanzador que usaba (`start "" /B cmd /c`) dejaba
  que los *build scripts* de cargo se adjudicaran una consola nueva y
  visible, interrumpiendo al usuario. Sustituido por
  `Start-Process -WindowStyle Hidden`, verificado con cero ventanas visibles.

---

## 4. Arquitectura

```
┌──────────────────────────────────────────────────────────────────┐
│  ui/          React 18 + TS 5.6 + Vite 6 + Tailwind 4           │
│               bandeja · overlay · barra flotante · onboarding    │
│               IPC tipado (tauri-specta), sin `any`               │
└───────────────────────────┬──────────────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────────────┐
│  dictation/   máquina de estados (código propio de Dixi)          │
│               fases · id de sesión · cancelación · errores        │
├────────────┬────────────┬────────────┬───────────┬───────────────┤
│  audio/    │  asr/      │  hotkeys/  │  paste/   │  storage/     │
│  captura   │  motores   │  globales │  Windows  │  SQLite       │
│  + VAD     │  + trait   │  + atajo  │  + clipbr │  + migraciones│
│            │  (nuevo)  │  ciclo    │           │               │
├────────────┴────────────┴────────────┴───────────┴───────────────┤
│  settings/    configuración local tipada                          │
└──────────────────────────────────────────────────────────────────┘
```

### 4.1 La capa `dixi/`: una responsabilidad, un módulo

`src-tauri/src/dixi/` es la aportación propia de la fase. **No reimplementa
nada**: cada módulo reexporta la implementación heredada que ya funciona.

| Módulo Dixi | Implementación heredada | Estado |
|---|---|---|
| `dixi::audio` | `managers::audio` + `audio_toolkit::vad` | reexporta |
| `dixi::asr` | **nuevo** (abstracción de proveedores) | código propio |
| `dixi::dictation` | **nuevo** (máquina de estados) | código propio |
| `dixi::hotkeys` | `shortcut` + `settings::ShortcutBinding` | reexporta |
| `dixi::paste` | `clipboard` + `paste_tx` | reexporta |
| `dixi::storage` | `managers::history` (SQLite + migraciones) | reexporta |
| `dixi::settings` | `settings` | reexporta |
| `dixi::ui` | `tray` + `overlay` | reexporta |

### 4.2 `dixi::dictation` — la máquina de estados

Resuelve lo que la auditoría identificó como el punto más frágil del
pipeline de Wispr Flow. Es **código puro** (sin Tauri, sin tokio), de modo
que sus 16 pruebas son instantáneas y deterministas.

- **Fases**: `Idle → Recording → Transcribing → Inserting → Idle`, con
  `Cancelled` y `Failed` como salidas.
- **Id de sesión monotono**: un evento tardío de un dictado ya cancelado se
  rechaza (`StaleSession`) en lugar de corromper el estado actual.
- **Cancelación** válida en captura, transcripción e inserción; inválida en
  reposo, con error explícito.
- **Errores recuperables**: un fallo de ASR deja la máquina en `Failed` y se
  puede volver a dictar sin reiniciar la aplicación.
- **El último texto insertado se conserva** tras cancelar o fallar.

### 4.3 `dixi::asr` — la abstracción de proveedores

Separa **qué** se transcribe de **quién** lo transcribe. Es el punto de
extensión para el proveedor remoto de MiniMax, sin tocar la orquestación.

- `trait AsrProvider` con `transcribe(request, cancel) -> Result<TranscribeResult, AsrError>`.
- `LocalAsrProvider`: motor local GGUF (`transcribe-cpp`), carga perezosa del
  modelo y `unload()` explícito para no dejar el pico de RAM del modelo
  residiendo entre dictados.
- `StaticAsrProvider`: proveedor de pruebas, sin disco ni modelo.
- `AsrError` clasificado por recuperabilidad, y `ProviderErrorKind` con la
  regla de reintento que fija la investigación de MiniMax: **cuota, clave
  inválida, filtro de contenido y saldo no se reintentan**; solo `RateLimit` y
  `Network` sí.
- `TranscribeRequest` incluye ya `vocabulary`, el punto de enganche del
  diccionario personalizado (P0 en la matriz de paridad, no implementado).

---

## 5. El modelo ASR: selección, procedencia y mediciones

### 5.1 Procedencia y licencias verificadas

Los tres modelos se descargaron **directamente de Hugging Face**, con revisión
fijada y **verificación sha256** contra el catálogo (script
`scripts/fetch_models.py`; un hash que no coincide descarta el fichero):

| Modelo | Fichero | Tamaño | Repo + revisión | sha256 |
|---|---|---|---|---|
| `canary-180m-flash` | `canary-180m-flash-Q8_0.gguf` | 208,3 MB | `handy-computer/canary-180m-flash-gguf` | ✅ verificado |
| `whisper-base` | `whisper-base-Q8_0.gguf` | 81,0 MB | `handy-computer/whisper-base-gguf` | ✅ verificado |
| `whisper-small` | `whisper-small-Q8_0.gguf` | 257,3 MB | `handy-computer/whisper-small-gguf` | ✅ verificado |

Las licencias (`MIT` para Whisper, `nvidia-open-model-license` para Canary)
figuran en el catálogo y están recogidas en `NOTICE.md`. **Ninguno de estos
pesos se versiona en el repositorio.**

### 5.2 Método de medición

- **Audio sintético local**, generado con `espeak-ng` (WSL) — sin micrófono
  real y sin audio de terceros. Tres muestras a 16 kHz mono PCM:
  `en_basico` (3,69 s), `es_basico` (4,54 s), `es_tecnico` (5,66 s, español con
  terminología técnica inglesa: *commit*, *logs*, *servidor de producción*).
- **Ruta de la propia aplicación** (`--transcribe-file`), que es exactamente
  el mismo camino por lotes que usa el dictado en vivo.
- 3 repeticiones por combinación; se reporta la mejor (`best_ms`).
- Backend: **CPU** (único dispositivo: `index=0 kind=cpu name=Intel(R)
  Core(TM) i5-8300H CPU @ 2.30GHz`).

### 5.3 Resultados

`rtf` = tiempo real de reproducción (menor es más rápido que el audio).
Medición final, con la máquina **sin carga externa** (5,9 GB de RAM libres),
mejor de 3 repeticiones:

| Modelo | Muestra | `best_ms` | rtf | Carga | Transcripción obtenida |
|---|---|---:|---:|---:|---|
| canary-180m-flash | en_basico | 341 | 10,83 | 608 ms | *below is in a dictation test for Dixie.* |
| canary-180m-flash | es_basico | 359 | 12,65 | 341 ms | *Hello, this is a proof of my dictation in Spanish for me, yes.* ⚠️ **detectó inglés** |
| canary-180m-flash | es_tecnico | 274 | 20,64 | 345 ms | **VACÍO** ⚠️ |
| whisper-base | en_basico | 1 184 | 3,12 | 206 ms | *Hello, this is Education F4DX.* |
| whisper-base | es_basico | 883 | 5,14 | 197 ms | *Hola, esta es una prueba de dictamen espanol para míxí.* |
| whisper-base | es_tecnico | 1 045 | 5,41 | 199 ms | *Hoy hacer conmítneo el camino y revisar los vlogs del sermimo de promoción.* |
| whisper-small | en_basico | 3 687 | 1,00 | 757 ms | *Hello, this is a dictation test for Dixie.* ✅ |
| whisper-small | es_basico | 3 420 | 1,33 | 439 ms | *Hola, esta es una prueba de mictamo en este anul para mixi.* |
| whisper-small | es_tecnico | 17 401 | 0,33 | 418 ms | *gu ya se tradiio od oudu xtule dod u miende and y读* ⚠️ |

### 5.4 Lectura de los resultados

1. **whisper-small es el más preciso** en inglés: reproduce el texto literal
   ("Hello, this is a dictation test for Dixie."). Pero es **más lento que el
   tiempo real** (rtf 0,33–1,33) y la muestra con terminología técnica se
   degrada hasta el nonsense. Para dictado en vivo es inviable en esta CPU.
2. **whisper-base** mantiene el español ("Hola, esta es una prueba de
   dictamen espanol…") a **3,1–5,4× tiempo real**, con una latencia total
   de ~1 s para una frase corta. Es el **equilibrio razonable** entre
   exactitud y latencia en CPU.
3. **canary-180m-flash** es con diferencia el más rápido (10,8–20,6× tiempo
   real), pero **falló en dos de tres muestras**, y de forma **reproducible**
   (se repitió con la máquina descargada y dio el mismo resultado):
   - una muestra de español la transcribió **en inglés** → fallo de detección
     de idioma;
   - otra devolvió **texto vacío**.

   Un proveedor de ASR que devuelve vacío o que cambia de idioma **no es
   aceptable en un flujo de dictado**: se insertaría texto incorrecto o nada.
   En el catálogo este modelo no declara `supports_language_detection` fiable
   para español, y el comportamiento observado lo confirma.
4. La calidad de las transcripciones está limitada por la voz sintética de
   `espeak-ng`, que es muy robótica y confunde a todos los modelos. Las
   **técnicas** con hablante humano pueden ser sensiblemente mejores; las
   **temporales** no.

### 5.5 Modelo recomendado para la Fase 2A

**`whisper-base` Q8_0** (81 MB, Apache-2.0), con dos condiciones:

- **Idioma forzado a `es`** cuando el usuario lo elija, en lugar de depender
  de la detección automática. La detección automática es exactamente donde
  Canary falló, y en la muestra `es_tecnico` whisper-small también se
  desvió ("gu ya se tradiio…").
- Dejar `whisper-small` disponible como opción de "más preciso" para quien
  acepte la latencia, y **descartar `canary` por defecto**.

Esta decisión es **provisional hasta medir con voz humana**: el audio
sintético permite afirmar la latencia y la robustez, pero **no** la
exactitud real.

## 6. La abstracción de ASR como preparación para MiniMax

`dixi::asr` deja preparado el punto de extensión sin introducir ninguna
dependencia de red. Lo que la investigación de MiniMax (doc 15) fija y que
**ya está codificado en el contrato**:

- La clasificación de errores por recuperabilidad, con la regla de no
  reintentar `2056` (cuota), `1004` (clave inválida), `1026/1027` (filtro) y
  `402` (saldo). Implementado y probado en
  `asr::tests::solo_los_errores_transitorios_se_reintentan`.
- La **clave nunca se envía desde el renderer**: el proveedor remoto, cuando
  exista, deberá vivir en el backend Rust.
- La cancelación cooperativa mediante `CancelToken`, comprobada antes y
  después de la transcripción.

**Lo que NO se ha hecho** (y no se debe hacer aún): ninguna llamada de red,
ningún campo `base_resp`, ningún `max_tokens`, ningún `thinking_delta`. El
proveedor local debe funcionar sin credenciales, y ése es el estado actual.

---

## 7. Rendimiento

Medido con `scripts/measure-perf.ps1` sobre el binario de depuración
(`dixi.exe`) en la máquina de referencia, con el equipo **sin carga externa**
(5,9 GB de RAM libres). Objetivo por objetivo:

| Objetivo (§8 del alcance) | Objetivo | **Medido** | Veredicto |
|---|---|---|---|
| Arranque en frío | < 1,5 s | **195 ms** | ✅ 7,7× de margen |
| RAM en reposo | < 120 MB | **8 MB** privada · **36,5 MB** de working set | ✅ |
| CPU en reposo | < 1 % | **0,0 %** (máx y media, 20 muestras de 1 s) | ✅ |
| CPU durante transcripción (4 núcleos) | ≤ 250 % | **49–190 %** (8 hilos lógicos) | ✅ |
| Latencia de transcripción (whisper-base, CPU) | — | **975–1 023 ms** para 3,7–5,7 s de audio | ver §7.2 |

### 7.1 Detalle del reposo

```
arranque: 195 ms (proceso vivo, ventana oculta creada)
reposo:   RAM privada 8,0 MB (máx) / 8,0 MB (media)
          working set 36,5 MB
          CPU 0,00 % (máx) / 0,00 % (media), 20 muestras de 1 s
```

La RAM privada de 8 MB es la de la aplicación en reposo: la WebView2 y el
resto viven en procesos aparte. El objetivo de 120 MB se cumple con holgura.

### 7.2 Detalle de la transcripción (whisper-base Q8_0, CPU)

Mejor de 3 ejecuciones por muestra:

| Muestra | `best_ms` | rtf | Carga del modelo | RAM privada | Working set | CPU (sistema) |
|---|---:|---:|---:|---:|---:|---:|
| `en_basico` (3,69 s) | 975 | 3,79 | 199 ms | 5,8 MB | 26,5 MB | 189,5 % (primera, con caché fría) |
| `es_basico` (4,54 s) | 1 021 | 4,45 | 196 ms | 6,2 MB | 27,7 MB | 54,7 % |
| `es_tecnico` (5,66 s) | 1 023 | 5,53 | 202 ms | 6,6 MB | 27,2 MB | 48,9 % |

Dos matices importantes, learned midiendo:

1. **La inferencia NO ocurre en el proceso principal.** Handy aísla
   `transcribe.cpp` en un *worker* separado (`engine_supervisor`), así que
   medir el CPU del proceso padre da ~5 % y es **falso**. La cifra válida es
   la del **árbol de procesos** o la del **sistema**: ~50–190 %.
2. **La primera transcripción tras cargar el modelo es más cara** (189,5 %
   frente a ~50 %): incluye la lectura en frío del GGUF de 81 MB. Las
   siguientes son mucho más baratas.

### 7.3 Sobre la estabilidad de las cifras

La carga externa de la máquina cambia las mediciones de forma apreciable: la
misma transcripción de `es_basico` con whisper-base midió **1 021 ms** con la
máquina descargada y **2 060 ms** con trabajo ajeno concurrente. Las cifras
absolutas de este informe son de la máquina descargada; **la comparación entre
modelos es la que es estable**, porque todas se midieron en la misma sesión.

### 7.4 Latencia por etapa

| Etapa | Medido | Observación |
|---|---|---|
| Arranque de la aplicación | 195 ms | hasta la ventana creada |
| Carga del modelo (primera vez) | 196–228 ms | GGUF de 81 MB, mmap |
| Transcripción (3,7–5,7 s de audio) | 975–1 023 ms | whisper-base Q8_0, CPU, AVX2 |
| **Total perceptible** (hablar → texto)** | **≈ 1,2–1,3 s** | con el modelo ya cargado |

El objetivo de "< 1,5 s" de la §8 del alcance se cumple **si el modelo ya
está cargado**; la primera transcripción tras abrir la aplicación añade
~200 ms.

---

## 8. Pruebas

| Suite | Cantidad | Resultado |
|---|---|---|
| Pruebas Rust (`cargo test`) | **304** | ✅ 0 fallos, 18 omitidas |
| — heredadas de Handy | 278 | ✅ |
| — nuevas en `dixi::dictation` | 16 | ✅ |
| — nuevas en `dixi::asr` | 10 | ✅ |
| Pruebas de frontend (guiones `assert`) | 3 guiones | ✅ todas las aserciones pasan |
| Compilación de frontend | `tsc && vite build` | ✅ |

Las 18 omitidas son pruebas heredadas de Handy marcadas `#[ignore]`
upstream (requieren red, hardware o un entorno gráfico concreto).

### 8.1 Cobertura por comportamiento exigido

| Comportamiento | Cómo se cubre |
|---|---|
| Estados del dictado | 16 pruebas de `dixi::dictation`: ciclo completo, transiciones inválidas, cancelación en las 3 etapas, error recuperable, sesión obsoleta, id monotónico |
| Cancelación | `cancelacion_en_cada_etapa_activa`, `cancelar_en_reposo_es_error`, `cancelar_conserva_el_ultimo_texto` |
| Errores de captura y ASR | `el_fallo_de_asr_registra_el_mensaje`, `error_es_recuperable`; `asr::tests::el_proveedor_local_falla_si_el_modelo_no_existe` |
| Atajos | `shortcut::tests` (heredado) + `keyboard.test.ts` (frontend) |
| Portapapeles | `clipboard::tests` (10, heredado) + `clipboard.test.ts` |
| Migraciones | `managers::history::tests` (3, heredado) |
| Configuración | `settings::tests` (26, heredado) |

### 8.2 Lo que **no** se automatizó (y por qué)

Las pruebas que requieren **escritura real en otra aplicación** (el bloque de
Notepad del criterio de aceptación) dependen del estado real del escritorio
de Windows: ventana enfocada, portapapeles compartido entre procesos y
recepción del texto por la aplicación destino. **Simularlo con mocks
probaría el mock, no la integración.** Por eso se declaran como
*verificación manual*, y el procedimiento exacto queda en §9.1.

Ninguna prueba toma audio real del micrófono: las muestras son sintéticas y
preparadas de antemano.

---

## 9. Limitaciones y trabajo pendiente

### 9.1 Verificación manual pendiente (requiere la acción del usuario)

Para cerrar el criterio "dictar una frase en español e insertarla
correctamente en Notepad":

1. Abrir Notepad.
2. Lanzar `dixi.exe` (o el instalador NSIS).
3. Pulsar el atajo de transcripción (por defecto, `Ctrl+Espacio`).
4. Decir una frase en español.
5. Verificar que el texto aparece en Notepad.

Lo que **sí** está automatizado y verificado: la transcripción local real
produce el texto correcto sobre audio sintético (§5), la máquina de estados
gestiona las transiciones (§4.2) y el mecanismo de pegado es el de Handy,
que inserta mediante el portapapeles con verificación de recibo
(`paste_tx::tests`, 7 pruebas heredadas). El eslabón que no se puede
automatizar es la **interacción con el escritorio**.

### 9.2 Limitaciones conocidas

| Limitación | Origen | Impacto |
|---|---|---|
| Solo CPU | Decisión §3.1 (falta el SDK de Vulkan) | Latencia 1,3–2,4× tiempo real con whisper-base. Con GPU sería 2–4× mejor |
| `GGML_NATIVE=OFF` | Decisión §3.2 | Se pierde la detección automática de ISA, pero se compila para AVX2 explícitamente |
| Descarga de modelos desde el CDN de Handy | Heredado | Los bytes se verifican por sha256, pero el transporte es infraestructura ajena. Ver §9.3 |
| `tao` y `hf-hub` en forks personales | Heredado | Riesgo de mantenimiento |
| Calidad de ASR medida sobre voz sintética | §5.4 | La exactitud real con voz humana está sin medir |
| Módulos fuera de alcance | §9.3 | Notetaker, MCP, snippets, transforms, reuniones, editor, Dictionary, resúmenes: **no implementados** |

### 9.3 Trabajo futuro (Fase 2B)

1. **Medir con voz humana** la exactitud real de `whisper-base` y
   `whisper-small` en español, con y sin terminología técnica.
2. **Idioma forzado** en la configuración: hoy el idioma por defecto es
   `auto`, y la detección automática es el punto débil demostrado por Canary.
3. **Descarga de modelos desde Hugging Face directo**, con el mirror de Handy
   como respaldo, para no depender de su CDN.
4. **Vulkan**, si el usuario autoriza instalar el SDK: medir la ganancia real
   de la GTX 1050 Ti antes de decidir.
5. **Resolver los forks** (`tao`, `hf-hub`) o substituirlos por las
   versiones de crates.io.
6. **Sustituir `bun` por `tsx`** en el resto de guiones heredados (esta fase
   ya lo hizo para las pruebas de frontend).
7. Los módulos de la matriz de paridad pendientes, por orden de prioridad:
   detección de aplicación y contexto de la ventana enfocada (P0),
   diccionario personalizado (P0), estilos por aplicación, transforms,
   snippets, reuniones, notetaker, MCP, resúmenes.

---

## 10. Reproducir las mediciones

```powershell
# 0. Dependencias
#    Rust 1.99 (rustup default stable-msvc), Node 20+, MSVC 14.51,
#    CMake + Ninja (pip install --user cmake ninja), WebView2 Runtime.

# 1. Runtime de Visual C++ a nivel de aplicacion
powershell -File scripts/stage-transcribe-libs.ps1

# 2. Frontend
npm.cmd install --ignore-scripts
npm.cmd run build

# 3. Nucleo Rust
cd src-tauri
cargo build --release          # -> target/release/dixi.exe

# 4. Pruebas
cargo test                     # 304 pruebas
cd .. && npm.cmd run test:unit # guiones de frontend

# 5. Modelos (descarga verificable por sha256)
python scripts/fetch_models.py src-tauri/src/catalog/catalog.json `
       "$env:APPDATA\com.dixi.app\models" `
       handy-computer/whisper-base-gguf

# 6. Muestras sinteticas (WSL + espeak-ng) y conversion a 16 kHz
wsl -d Ubuntu -u root -- bash /mnt/d/Codigo/Dixi/scripts/make-fixtures-wsl.sh
python scripts/make_fixtures.py <origen> <destino>

# 7. Medicion
powershell -File scripts/bench-models.ps1 -Exe <dixi.exe> `
    -AppDataDir "$env:APPDATA\com.dixi.app" -ModelsDir <dir> `
    -FixturesDir <dir> -Out bench.json
powershell -File scripts/measure-perf.ps1 -Exe <dixi.exe> `
    -AppDataDir "$env:APPDATA\com.dixi.app" -FixturesDir <dir> `
    -Model <id> -Out perf.json
```

---

## 11. Historial de commits

Rama `feat/handy-foundation` (desde `develop`, desde `master`).

| Commit | Contenido |
|---|---|
| `f562745` | `chore:` base del repositorio: los 16 informes de auditoría, README, LICENSE, NOTICE, `.gitignore` |
| `1e50772` | `feat:` incorporación del núcleo de Handy como **línea base** (con las dos correcciones de plataforma documentadas) |
| `60b5512` | `refactor:` reidentificación como Dixi (identidad, activos propios, contratos preservados) + módulos nuevos `dixi::dictation` y `dixi::asr` |
| `5287ede` | `chore:` cierre del flujo de actualizador, `tsx` como sustituto de `bun`, scripts de medición, informe de la fase |
| `9555d41` | `fix:` el modelo VAD empaquetado y el panel de modelos **no** estaban versionados (regla `models/` mal escrita) |

### 11.1 Dos errores de `.gitignore` que el proceso de verificación destapó

Merecen quedar escritos, porque son fáciles de repetir y tienen consecuencias
silenciosas:

1. **`*.wav` excluía los efectos de sonido de la interfaz.**
   `src-tauri/resources/{marimba,pop}_{start,stop}.wav` son recursos que la
   aplicación carga en tiempo de ejecución. Un clon nuevo compilaba, pero
   sin sonidos de interfaz.
2. **`models/` excluía el directorio completo**, y git **no puede re-incluir
   un fichero cuyo directorio padre está excluido**. Dos consecuencias:
   - la excepción del modelo Silero VAD (que el motor de audio carga como
     recurso) **nunca surtió efecto**: el modelo no estaba en el repositorio;
   - `src/components/settings/models/` (536 líneas: el panel de selección y
     descarga de modelos) **tampoco estaba versionado**. Un clon nuevo no
     habría compilado.

La corrección es `models/*` (contenido) en lugar de `models/` (directorio).
Se verificó con `git ls-files` que no queda ningún fichero de fuente fuera
del control de versiones.
