# Dixi

> **Dixi** — dictado por voz local para Windows 10/11 x64.
> Privacidad por diseño: el audio **nunca** sale de la máquina.

Dixi es una aplicación **personal** (no SaaS, sin cuentas, sin pagos, sin
suscripciones) que captura el micrófono, transcribe **localmente** con un
modelo ASR abierto e inserta el texto en la aplicación que tenía el foco.

---

## Estado actual

**Fase 2A — construcción de la base funcional.** Completada.

| Entregable | Estado |
|---|---|
| Base de código reutilizada de Handy (MIT) | ✅ incorporada, con atribución |
| Identidad visual y nombre propios de Dixi | ✅ activos generados desde cero |
| Arquitectura de módulos por responsabilidad (`dixi/`) | ✅ |
| Flujo vertical: bandeja → atajo → captura → ASR → inserción | ✅ |
| Abstracción de proveedores de ASR (preparación para MiniMax) | ✅ |
| Modelo ASR local con procedencia y licencia verificadas | ✅ `whisper-base` Q8_0 |
| Pruebas automatizadas (Rust + TypeScript) | ✅ 304 Rust + 3 guiones de frontend |
| Medición de rendimiento | ✅ frente a los objetivos de la auditoría |
| Módulos fuera de alcance (notetaker, MCP, snippets, transforms, …) | ❌ no implementados (Fase 2B) |

**Informe completo**: [`docs/dixi-phase2a-report.md`](./docs/dixi-phase2a-report.md).

### Resultados medidos

| Objetivo | Objetivo | Medido |
|---|---|---|
| Arranque en frío | < 1,5 s | **195 ms** |
| RAM en reposo | < 120 MB | **8 MB** privados · 36,5 MB de working set |
| CPU en reposo | < 1 % | **0,0 %** |
| CPU durante transcripción (4 núcleos) | ≤ 250 % | **49–190 %** |
| Latencia de transcripción (whisper-base, CPU) | — | **~1 s** por frase corta |

---

## Stack

| Capa | Tecnología |
|---|---|
| Núcleo nativo | Rust 1.99 (stable, MSVC) |
| Runtime de escritorio | Tauri 2.11 |
| Interfaz | React 18 + TypeScript 5.6 + Vite 6 + Tailwind 4 |
| Persistencia | SQLite (`rusqlite` 0.37, embebido) + `rusqlite_migration` |
| IPC tipado | `tauri-specta` + `specta` (contratos Rust ↔ TypeScript sin `any`) |
| Captura de audio | `handy-recorder` + `earshot` + VAD Silero |
| ASR local | `transcribe-cpp` (GGUF; backends CPU y Vulkan) |
| Inserción de texto | `enigo` + ruta *reliable paste* de Handy |
| Atajos globales | `tauri-plugin-global-shortcut` + `handy-keys` |

Plataformas objetivo: **Windows 10/11 x64** (el código conserva las rutas de
macOS y Linux de Handy, pero esta fase sólo valida Windows).

---

## Arquitectura

```
┌──────────────────────────────────────────────────────────────────┐
│  ui/         React + TS — bandeja, barra flotante, overlay        │
│              IPC tipado (tauri-specta)                            │
└──────────────────────────┬───────────────────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────────────────┐
│  dictation/  máquina de estados del dictado (orquestación)        │
│              cancelable · errores recuperables · por etapa        │
├───────────┬───────────┬────────────┬────────────┬─────────────────┤
│  audio/   │  asr/     │  hotkeys/  │  paste/    │  storage/       │
│  captura  │  motores  │  globales  │  inserción │  SQLite         │
│  + VAD    │  ASR      │  + atajo   │  Windows   │  + migraciones  │
│           │  + trait  │  ciclo     │  + clipbrd │                 │
├───────────┴───────────┴────────────┴────────────┴─────────────────┤
│  settings/    configuración local tipada                          │
└──────────────────────────────────────────────────────────────────┘
```

Responsabilidades:

- **`audio`** — captura de micrófono y control de grabación.
- **`asr`** — proveedores de reconocimiento de voz (trait `AsrProvider`).
- **`dictation`** — coordinación del pipeline, máquina de estados, cancelación.
- **`hotkeys`** — atajos globales.
- **`paste`** — inserción de texto en Windows y manejo del portapapeles.
- **`storage`** — SQLite y migraciones.
- **`settings`** — configuración local.
- **`ui`** — interfaz, bandeja del sistema y barra flotante.

Las interfaces dejan preparado el punto de extensión para MiniMax, diccionario
personalizado, snippets, estilos por aplicación, transforms, reuniones,
notetaker y MCP. **Ninguno de esos módulos está implementado en esta fase.**

---

## Compilación

### Requisitos

| Requisito | Versión | Nota |
|---|---|---|
| Rust | 1.99 (stable) | `rustup default stable-msvc` |
| Node.js | 20+ |usado 24.22 |
| Visual Studio | Build Tools 2022+ con *Desktop development with C++* | MSVC 14.51 |
| Windows SDK | 10.0.22621.0+ | |
| WebView2 Runtime | Evergreen | ya presente en Windows 10/11 |
| CMake / Ninja | 3.27+ / 1.10+ | requerido por el backend Vulkan de `transcribe-cpp` |

### Pasos

```powershell
# 0. Runtime de Visual C++ a nivel de aplicacion (Tauri lo exige antes
#    de compilar; upstream lo hace en su CI)
powershell -File scripts/stage-transcribe-libs.ps1

# 1. Dependencias de frontend
npm.cmd install --ignore-scripts

# 2. Build del nucleo Rust
#    OBLIGATORIO --features custom-protocol: sin esta caracteristica el
#    binario no puede cargar su propio frontend (la WebView2 busca el
#    servidor de desarrollo de Vite, la UI queda en blanco y los atajos
#    globales no se registran). Ver el informe de la Fase 2A.
cd src-tauri
cargo build --release --features custom-protocol

# 3. Binario
#    src-tauri/target/release/dixi.exe
```

> **El `--features custom-protocol` no es opcional.** Sin él, `cargo build
> --release` genera un binario que arranca pero queda inutilizable: la
> WebView2 intenta conectarse a `http://localhost:1420` (el servidor de
> desarrollo de Vite), el frontend no carga, la interfaz queda en blanco y
> **los atajos globales nunca se registran**. El CLI de Tauri
> (`npm run tauri build`) activa la característica por su cuenta y además
> genera el instalador NSIS.

> **Nota sobre el perfil de CPU**: `.cargo/config.toml` fija
> `GGML_NATIVE=OFF` y habilita explicitamente AVX2/FMA/F16C/BMI2. Sin esto,
> ggml ejecuta una prueba de AVX-512 que muere con *Illegal instruction* en
> CPUs sin AVX-512 y **CMake >= 4 lo trata como error fatal de configuracion**.
> Ver el informe, seccion 3.2.

Para desarrollo con recarga en caliente:

```powershell
npm.cmd run tauri dev
```

### Variables de entorno

| Variable | Efecto |
|---|---|
| — | Los pesos de los modelos ASR se guardan en `%APPDATA%\com.dixi.app\models` (derivado del identificador de Tauri `com.dixi.app`) |

---

## Pruebas

```powershell
# Rust (pruebas unitarias y de integración)
cd src-tauri
cargo test

# TypeScript (comprobación de tipos y lint)
npm.cmd run build      # tsc + vite build
npm.cmd run lint
```

Las pruebas que **requieren interacción manual con Windows** (inserción real
en otra aplicación, hotkey global en escritorio, portapapeles con otra app
abierta) están documentadas como tales en el informe técnico: no se simulan
en CI porque dependen del estado real del escritorio.

Ninguna prueba toma audio real del micrófono. Se usan muestras de audio
ficticio previamente generadas.

---

## Documentación

- [`docs/validation-phase2a/PROCEDIMIENTO.md`](./docs/validation-phase2a/PROCEDIMIENTO.md) —
  **cómo ejecutar Dixi, activar el dictado, configurar el micrófono y medir
  la validación**. Punto de partida de la Fase 2B.
- [`docs/dixi-phase2a-report.md`](./docs/dixi-phase2a-report.md) —
  informe técnico de la Fase 2A: procedencia, licencias, arquitectura, los
  bloqueos de compilación, la selección de modelo con mediciones, el
  rendimiento frente a los objetivos y el trabajo futuro.
- [`docs/dixi-reverse-engineering/`](./docs/dixi-reverse-engineering) —
  auditoría funcional de Wispr Flow (16 informes). Es la **especificación**
  de este proyecto: define el comportamiento esperado del pipeline de
  dictado, el modelo de datos, el presupuesto de rendimiento y la matriz de
  paridad.
- [`NOTICE.md`](./NOTICE.md) — procedencia del código, licencias y atribuciones.
- [`LICENSE`](./LICENSE) — MIT (Dixi).
- [`LICENSE-HANDY`](./LICENSE-HANDY) — MIT original de Handy.

---

## Licencia

MIT. Ver [`LICENSE`](./LICENSE) y [`NOTICE.md`](./NOTICE.md) para la
procedencia del código de terceros.
