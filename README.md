# Dixi

> **Dixi** — dictado por voz local para Windows 10/11 x64.
> Privacidad por diseño: el audio **nunca** sale de la máquina.

Dixi es una aplicación **personal** (no SaaS, sin cuentas, sin pagos, sin
suscripciones) que captura el micrófono, transcribe **localmente** con un
modelo ASR abierto e inserta el texto en la aplicación que tenía el foco.

---

## Estado actual

**Fase 2A — construcción de la base funcional.** En curso.

| Entregable | Estado |
|---|---|
| Base de código reutilizada de Handy (MIT) | ✅ incorporada |
| Arquitectura de módulos por responsabilidad | ✅ |
| Flujo vertical: bandeja → atajo → captura → ASR → inserción | ✅ |
| Modelo ASR local con procedencia y licencia verificadas | ✅ |
| Pruebas automatizadas (Rust + TypeScript) | ✅ |
| Medición de rendimiento | ✅ |
| Módulos fuera de alcance (notetaker, MCP, snippets, transforms, …) | ❌ no implementados (Fase 2B) |

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
# 1. Dependencias de frontend
npm.cmd install --ignore-scripts

# 2. Build del núcleo Rust + empaquetado Tauri
cd src-tauri
cargo build --release

# 3. Binario
#    src-tauri/target/release/dixi.exe
```

Para desarrollo con recarga en caliente:

```powershell
npm.cmd run tauri dev
```

### Variables de entorno

| Variable | Efecto |
|---|---|
| `DIXI_MODEL_DIR` | Ruta donde se guardan los pesos de los modelos ASR (por defecto: `%LOCALAPPDATA%\Dixi\models`) |

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
