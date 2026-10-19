# 14 — Alternativas open source: Handy vs OpenWhispr

> Investigación realizada sobre los repositorios públicos `cjpais/Handy` y `OpenWhispr/openwhispr`, con fuentes primarias (README, `docs/`, `Cargo.toml`, `package.json`, `LICENSE`, CHANGELOG y la API de GitHub).
> Marcadores: **[V]** verificado leyendo la fuente · **[I]** inferido · **[NV]** no verificado.
> Informe completo: `docs/dixi-reverse-engineering/evidencia/research-handy-openwhispr.md` (83 KB, 277 marcas VERIFIED / 23 INFERRED / 26 NOT VERIFIED).

---

## 14.1 Veredicto

> **Base: `cjpais/Handy`.** Es **exactamente** el stack objetivo de Dixi (Tauri 2 + Rust + React + TypeScript + SQLite + Whisper local), de modo que el núcleo se porta sin reescritura. **OpenWhispr** es un producto mucho más amplio, pero está construido sobre Electron + JavaScript/TypeScript: adoptarlo implicaría **reescribir su núcleo en Rust**. Se usa como **biblioteca de referencia**, no como base.

| | **Handy** | **OpenWhispr** |
|---|---|---|
| Stack | **Tauri 2.11.5 + Rust + React 18 + TS 5.6 + SQLite (rusqlite 0.37) + Whisper local** | **Electron 41 + JavaScript/TypeScript** |
| Licencia | **MIT** (`Copyright (c) 2025 CJ Pais`) | **MIT** (`Copyright (c) 2024 OpenWhispr Team`) |
| ASR | **Local** (GGUF/Vulkan, `transcribe-cpp`) | Local + cloud |
| LLM | **BYO-key** (clave del usuario) | Local + cloud |
| Alcance de producto | Dictado | Dictado + reuniones + notas + búsqueda + snippets + diccionario + agente + MCP + SSO |
| Tamaño del código | — | ~162 MB de fuente, ~30 scripts nativos |
| Estado | ⚠️ **feature freeze** declarado en `AGENTS.md` | Activo |
| Últimarelease | v0.9.8 (2026-10-03) | — |
| Último commit | 2026-10-15 | — |
| Stars / contribuidores | 34,1k / 100+ | 100+ |

---

## 14.2 Comparación lado a lado

| Dimensión | Handy | OpenWhispr | **Dixi necesita** |
|---|---|---|---|
| **Runtime** | Tauri 2 (WebView del SO) | Electron | Tauri 2 ✅ |
| **Lenguaje del núcleo** | Rust | JavaScript | Rust ✅ |
| **Base de datos** | SQLite (rusqlite 0.37) | — | SQLite ✅ |
| **ASR local** | ✅ `transcribe-cpp` (GGUF + Vulkan) | ✅ | ✅ |
| **VAD** | ✅ earshot + **Silero** | ✅ | ✅ |
| **Hotkey global** | ✅ | ✅ | ✅ |
| **Inserción de texto (Windows)** | ✅ `paste_tx/windows.rs` + `enigo` + truco «Reliable Paste» (notificación de lectura del portapapeles) | ✅ `windows-fast-paste` | ✅ |
| **Audio del sistema (loopback)** | ❌ | ✅ helper WASAPI **process-loopback** | ✅ (a portar de OpenWhispr) |
| **Monitor de texto enfocado** | ❌ | ✅ `windows-key-listener` | ✅ |
| **Diccionario** | ❌ | ✅ con **correction-learner** | ✅ |
| **Snippets** | ❌ | ✅ | ✅ (a construir, con variables) |
| **Captura de reuniones** | ❌ | ✅ | ✅ (a construir) |
| **Diarización** | ❌ | ✅ | ✅ (a construir) |
| **Notas + búsqueda semántica** | ❌ | ✅ (Qdrant + MiniLM) | Opcional (FTS5 basta para v1) |
| **MCP** | ❌ | ⚠️ **el servidor MCP NO está en el repo** (`contents/mcp` → 404) ⇒ no portable | ✅ a construir |
| **API pública** | ❌ | ✅ | Opcional |
| **SSO / empresa** | ❌ | ✅ | ❌ (no necesario) |
| **Telegram bot** | ❌ | ✅ | ❌ |
| **Privacidad** | ✅ offline puro | ⚠️ conectado a cloud comercial | ✅ local por defecto |
| **Feature freeze** | ⚠️ sí | no | — |

---

## 14.3 Handy — Portar (recomendado)

### 14.3.1 Piezas de mayor valor a portar

| Pieza | Qué aporta | Licencia | Dificultad Dixi |
|---|---|---|---|
| **`transcribe-cpp`** | Núcleo ASR GGUF con **aceleración Vulkan** ⇒ viable en CPU y GPU | MIT (+ forks personales) | Media (dependencia nativa) |
| **`handy-recorder`** + earshot + Silero VAD | Captura de micrófono + **detección de voz** (corta el silencio antes del ASR) | MIT | Baja |
| **`specta` + `tauri-specta`** | **IPC tipado** entre Rust y TypeScript ⇒ sin `any`, sin desincronizar contratos | MIT | Baja |
| **Pipeline de cancelación** | `complete_unless_cancelled` con sondeo de **25 ms** ⇒ el usuario puede cancelar el dictado | MIT | Baja |
| **`FinishGuard` + `trim_freed_memory()`** | Libera la memoria del modelo al terminar ⇒ evita el pico de RAM | MIT | Baja |
| **`paste_tx/windows.rs`** + `enigo` + **truco «Reliable Paste»** | Inserción fiable en la app enfocada: escribe en el portapapeles, **espera la notificación de lectura** y recién entonces envía `Ctrl+V` | MIT | Media (la parte más frágil de Windows) |

> **El truco «Reliable Paste» es el hallazgo más accionable de esta investigación.** En Windows, escribir al portapapeles y enviar `Ctrl+V` inmediatamente es una carrera: el destino puede leer el portapapeles **antes** de que la escritura termine, y pegar el contenido antiguo. La solución de Handy (esperar la **notificación de lectura del portapapeles** antes de enviar el atajo) elimina la condición de carrera. Es exactamente el tipo de detalle que hace que un dictado «a veces pegue mal», y que Wispr resuelve con su sidecar de traza `verify`. [V] para la existencia del truco; [I] para su aplicabilidad a Dixi.

### 14.3.2 Riesgos de Handy

| Riesgo | Detalle | Mitigación |
|---|---|---|
| **Feature freeze** | `AGENTS.md` declara explícitamente la congelación de features | Forkear el repo y **vendorizar** el núcleo antes de construir |
| **Dependencias de forks personales** | `cjpais/tao`, `cjpais/hf-hub`, `cjpais/transcribe-cpp` | Auditar y **vendorizar** con atribución |
| **Sin snippets** | La función que más se echa de menos en Dictado no existe | Construir (ver [05-SNIPPETS.md](05-SNIPPETS.md)) |
| **Sin reuniones** | No hay captura de audio del sistema ni diarización | Portar el helper de OpenWhispr + construir la cadena de diarización |
| **Sin MCP** | — | Construir (ver [10-MCP.md](10-MCP.md)) |
| **Marca** | Los assets de marca de Handy **no son open source** | Usar branding «Dixi» propio |

---

## 14.4 OpenWhispr — Referencia (no base)

### 14.4.1 Piezas a portar como referencia

| Pieza | Qué aporta | Por qué desde OpenWhispr |
|---|---|---|
| **Helper WASAPI process-loopback** | Captura de **audio del sistema** (Zoom/Teams) | Handy no lo tiene; es el componente crítico del notetaker |
| `windows-key-listener` | Detección de teclas en la app enfocada | Base del «¿el usuario ya escribió?» y del fin de observación de contenido |
| `windows-fast-paste` | Inserción rápida | Complemento de `paste_tx` |
| **Monitor de texto enfocado** | Lee el texto bajo el cursor | Base de la sustitución de nombres propios vía AX |
| **Lógica de snippets + dictionary-prompt-cap + correction-learner** | Algoritmos de expansión y aprendizaje | Handy no los tiene;wispr tampoco (ver [04](04-DICCIONARIO.md) §4.5) |
| Qdrant + MiniLM (búsqueda semántica) | Búsqueda por significado en el historial | Opcional; SQLite FTS5 cubre el 80 % |

### 14.4.2 Lo que NO se puede portar de OpenWhispr

> **El servidor MCP de OpenWhispr no está en el repositorio** (`contents/mcp` → 404). No es portable. [V]

### 14.4.3 Por qué NO usar OpenWhispr como base

1. **Electron ⇒ 360 MB de RAM** — exactamente el problema que Dixi viene a eliminar (ver [13-RENDIMIENTO.md](13-RENDIMIENTO.md)).
2. **162 MB de fuente y ~30 scripts de build/descarga nativos** ⇒ toolchain pesada, builds frágiles.
3. **Núcleo en JavaScript** ⇒ la reescritura a Rust del hotkey, la captura, el ASR y la inserción de texto es el 80 % del trabajo de Dixi.
4. **Conectado a un cloud comercial** ⇒ contradice el requisito de privacidad.

---

## 14.5 Lo que NINGÚN proyecto cubre (brecha de Dixi)

| Función de Wispr | Handy | OpenWhispr | **Debe construir Dixi** |
|---|---|---|---|
| **Estilos por app / Voces** | ❌ | ❌ | ✅ **sí** — 4 perfiles + estilos |
| **Transforms / Polish** | ⚠️ BYO-key genérico | ⚠️ | ✅ **sí** — catálogo de 13 transforms |
| **Snippets con variables** | ❌ | ⚠️ sin variables | ✅ **sí** |
| **Sustitución de nombres propios vía AX** | ❌ | ❌ | ✅ **sí** |
| **Notetaker con diarización + fusión streaming/final** | ❌ | ⚠️ parcial | ✅ **sí** (el bloque más grande) |
| **Servidor MCP** | ❌ | ❌ (no en el repo) | ✅ **sí** |
| **Motor de fusionTuning** (ventanas de 6 s / 2 s) | ❌ | ❌ | ✅ **sí** |
| **Pipeline reanudable por segmentos** | ❌ | ❌ | ✅ **sí** (copiar de Wispr) |
| **Reconocimiento de app + URL + AX** | ❌ | ⚠️ | ✅ **sí** |
| **Estadísticas de calidad del formateo** | ❌ | ❌ | ✅ **sí** |

---

## 14.6 ⚠️ Riesgo de licencia más importante (no resolver)

> **Repo MIT ≠ producto MIT.** Los **pesos de los modelos ASR tienen licencia propia**:

| Modelo | Licencia | Implicación |
|---|---|---|
| **Parakeet GGUF recomendado por Handy** | `nvidia-open-model-license` (**no MIT**) | Distribuir el modelo exige cumplir esa licencia |
| `nvidia/parakeet-tdt-0.6b-v3` (OpenWhispr) | **CC-BY-4.0** | Atribución obligatoria |
| Silero VAD | MIT | ✅ |
| sherpa-onnx / onnxruntime | Apache-2.0 / MIT | ✅ |
| whisper.cpp | MIT | ✅ |
| llama.cpp | MIT | ✅ |
| Qdrant / MiniLM | Apache-2.0 / varies | Auditar |
| **MiniLM (búsqueda semántica)** | varies | Auditar |

> **Pendiente crítico antes de distribuir Dixi:** auditar la licencia de `transcribe-cpp`, `handy-recorder`/keys, Silero VAD, sherpa-onnx, onnxruntime, whisper.cpp, llama.cpp, Qdrant, MiniLM y Orukeet. **No realizado** en esta auditoría y **bloqueante para distribución**.

---

## 14.7 Arquitectura resultante de Dixi

```
┌──────────────────────────────────────────────────────────────────┐
│  d-frontend (React + TS)   ← de Handy (UI) + diseño propio        │
│  tauri-specta IPC tipado  ← de Handy                              │
└──────────────────────────┬───────────────────────────────────────┘
                           │  (sin Electron, sin GPU process)
┌──────────────────────────▼───────────────────────────────────────┐
│  d-core (Rust)                                                    │
│   ├── capture      ← handy-recorder + earshot + Silero VAD        │
│   ├── asr          ← transcribe-cpp (Parakeet GGUF / Vulkan)      │
│   ├── format       ← propio (reglas) + LLM local (llama.cpp)     │
│   ├── ax           ← propio (UIAutomation) — SIN COBERTURA OSS    │
│   ├── dictionary   ← OpenWhispr (correction-learner) + propio    │
│   ├── snippets     ← propio (con variables)                       │
│   ├── transforms   ← propio (13 transforms, locales)              │
│   ├── notetaker    ← loopback de OpenWhispr + diarización propia  │
│   ├── mcp          ← propio (servidor MCP stdio)                  │
│   ├── hotkey/paste ← Handy (paste_tx + Reliable Paste)            │
│   └── storage      ← SQLite + rusqlite + migraciones (patrón Wispr)│
└──────────────────────────────────────────────────────────────────┘
```

**Balance de esfuerzo estimado [I]:**

| Bloque | Esfuerzo | Origen |
|---|---|---|
| Captura + ASR + hotkey + paste | **Bajo** (20 % del trabajo) | Portar de Handy |
| Loopback de audio del sistema | Bajo | Portar de OpenWhispr |
| Formateo local + estilos + snippets + transforms | **Medio** (30 %) | Propio |
| AX + detección de app + nombres propios | **Medio-alto** (20 %) | Propio |
| Notetaker + diarización + fusión | **Alto** (25 %) | Propio (el bloque crítico) |
| MCP | Bajo | Propio |

---

### Referencias cruzadas

- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — arquitectura de Wispr a replicar
- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — pipeline
- [05-SNIPPETS.md](05-SNIPPETS.md) — snippets con variables (brecha)
- [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md) — notetaker y diarización
- [10-MCP.md](10-MCP.md) — MCP a construir
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — por qué abandonar Electron
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — priorización final
