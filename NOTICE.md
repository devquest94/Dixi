# NOTICE — Procedencia del código de Dixi

Dixi es una aplicación de dictado por voz para Windows, construida sobre el
núcleo de **[Handy](https://github.com/cjpais/Handy)**.

---

## 1. Proyecto de origen

| Campo | Valor |
|---|---|
| Proyecto | `cjpais/Handy` |
| URL | https://github.com/cjpais/Handy |
| Licencia | MIT — `Copyright (c) 2025 CJ Pais` |
| Commit de origen | `9e57a0d7929fcc6417ea0d6e54c1b57f62635edd` |
| Descripción del commit | `long form chunking with vad (#2282)` |
| Fecha del commit | 2026-10-15 22:04:40 +0800 |
| Tag más cercano | `v0.9.8` (el commit de origen está 23 commits por delante del tag) |
| Rama de origen | `main` |
| Versión declarada en `Cargo.toml` | `0.9.8` |

El núcleo de Handy se incorpora a Dixi **sin reescritura**: se conserva la
estructura de módulos, el pipeline de transcripción, la captura de audio, el
gestor de atajos globales y la inserción de texto en Windows.

Handy está licenciado bajo MIT, que permite uso, modificación y redistribución
con este aviso. El texto íntegro de la licencia original se conserva en
[`LICENSE-HANDY`](./LICENSE-HANDY).

---

## 2. Qué se modificó respecto de Handy

| Área | Cambio | Motivo |
|---|---|---|
| Identidad del producto | `productName`, `identifier` y recursos visuales | Sustituir la marca de Handy por la de Dixi (§4 del alcance) |
| Nombres de paquete | `handy` → `dixi`, `handy_app_lib` → `dixi_app_lib` | Identidad propia |
| Assets de marca | Iconos y recursos reemplazados por recursos propios | Los assets de marca de Handy no son open source |
| Módulos | Nombres de módulos alineados a las responsabilidades de Dixi | §5 del alcance: `audio`, `asr`, `dictation`, `hotkeys`, `paste`, `storage`, `settings`, `ui` |

No se realizaron refactorizaciones masivas del núcleo.

---

## 3. Dependencias de terceros y forks

El `Cargo.toml` de origen incluye dependencias en forks personales del autor
original. Se conservan tal cual, con su procedencia, y quedan marcadas en el
informe técnico como deuda a resolver antes de cualquier distribución pública:

| Dependencia | Origen | Nota |
|---|---|---|
| `tao`, `tao-macros` | `github.com/cjpais/tao` (rev `c3bee28c1d446d95f08c95c3b6f8d4bde052b876`) | Fork personal vía `[patch.crates-io]` |
| `hf-hub` | `github.com/cjpais/hf-hub` (rama `cancellable-downloads`) | Fork personal para descargas cancelables |
| `handy-recorder` | crates.io | Captura de micrófono |
| `handy-keys` | crates.io | Atajos de teclado |
| `transcribe-cpp` | crates.io | Núcleo ASR (GGUF + backends CPU/Vulkan) |
| `earshot` | crates.io | Detección de actividad de voz |
| `enigo` | crates.io | Inyección de teclas (inserción de texto) |
| `rusqlite` | crates.io | Persistencia SQLite |
| `tauri-specta` / `specta` | crates.io | IPC tipado Rust ↔ TypeScript |
| `rdev` | `github.com/rustdesk-org/rdev` | Entrada global |

---

## 4. Licencias de los pesos de modelos (NO se distribuyen con Dixi)

Los pesos de los modelos ASR **no forman parte de este repositorio** y tienen
licencias propias, distintas de la del código. Se descargan por separado y se
verifica su licencia antes de usarlos:

| Modelo | Licencia |
|---|---|
| Whisper (`ggml` / GGUF) | MIT (código whisper.cpp) — los pesos Official OpenAI son MIT |
| Parakeet GGUF (recomendado por Handy) | `nvidia-open-model-license` — **no es MIT** |
| Silero VAD | MIT |
| sherpa-onnx / onnxruntime | Apache-2.0 / MIT |

Dixi no distribuye pesos de modelos. Verificar la sección de licencias del
informe técnico antes de cualquier publicación.

---

## 5. Auditoría funcional de referencia

La especificación funcional de Dixi procede de una auditoría de ingeniería
inversa de **Wispr Flow 1.6.1174** conducted sobre la instalación local del
usuario. Los 16 informes resultantes están en
[`docs/dixi-reverse-engineering/`](./docs/dixi-reverse-engineering) y se
versionan como especificación. La evidencia cruda (capturas, logs, esquemas
extraídos del equipo del usuario) se excluye de Git por contener rutas
absolutas e identificadores de sesión.

No se ha modificado ni interfierido con Wispr Flow.

---

## 6. Licencia de Dixi

MIT — ver [`LICENSE`](./LICENSE).
