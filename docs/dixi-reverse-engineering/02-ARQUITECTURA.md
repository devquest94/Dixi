# 02 — Arquitectura del sistema Wispr Flow (y equivalencias en Dixi)

> Reconstrucción de la arquitectura a partir de evidencia observable: árbol de procesos, ventanas, esquema SQLite, migraciones, feature flags y superficie CDP.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 2.1 Capas del sistema

```
┌───────────────────────────────────────────────────────────────────────────┐
│  CAPA 4 — SERVICIOS CLOUD (Wispr AI)                                      │
│  • ASR + formato (streaming)            • Polish / Transforms (LLM)      │
│  • Notetaker: transcripción + speaker attribution + resumen + insights   │
│  • Sincronización multi-dispositivo    • Gamification / milestones       │
│  • Notificaciones remotas (RemoteNotifications)  • Entitlements / billing │
└───────────────────────────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────────────────────────┐
│  CAPA 3 — APP ELECTRON (proceso principal, PID 4484)                      │
│  ┌──────────────┬──────────────┬──────────────┬────────────────────────┐  │
│  │  hub         │  status      │ contextMenu  │ scratchpad             │  │
│  │  (5,6 MB JS) │  (1,2 MB JS) │              │                        │  │
│  │  Ventana     │  Flow Bar    │  Overlay     │  Bloc de notas         │  │
│  │  principal   │  flotante    │  contextual  │  rápido                │  │
│  └──────────────┴──────────────┴──────────────┴────────────────────────┘  │
│  • Tray (Electron_NotifyIconHostWindow)   • Diálogo del sistema de archivos│
│  • 5 renderers + gpu-process + 3 utility + crashpad                     │
│  • Secure: Sentry (nativo + JS)  • Protocolo propio: wispr-extension://  │
└───────────────────────────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────────────────────────┐
│  CAPA 2 — HELPER NATIVO (proceso separado, PID 16872, 19,8 MB)           │
│  • Hotkeys globales  • Captura de micrófono  • Inserción de texto        │
│  • Integración con dispositivos Jabra   • v1.0.0+38f663c0… (hash git)   │
└───────────────────────────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────────────────────────┐
│  CAPA 1 — PERSISTENCIA LOCAL                                              │
│  • flow.sqlite (755 MB, 29 tablas, Sequelize + 174 migraciones)           │
│  • config.json (preferencias)  • feature-flags-cache.json                │
│  • meetings/<uuid>/ (mic.pcm, upload.ogg, live.ndjson, refined.ndjson)   │
│  • Dictionaries/  • avatars/  • backups/                                 │
└───────────────────────────────────────────────────────────────────────────┘
```

**Equivalencia en Dixi (Tauri 2 + Rust + React):** las capas 3–4 se reducen a un solo proceso Rust (backend) + WebView del sistema, y la Capa 2 se integra en el mismo proceso Rust. Resultado: **2 procesos en vez de 12**, sin GPU process, sin utility processes, sin crashpad propio.

---

## 2.2 Superficie de ventanas (verificada vía Win32 `EnumWindows`)

| HWND | Clase | Título | Visible | Tamaño | Renderer CDP |
|---|---|---|---|---|---|
| 198504 | Chrome_WidgetWin_1 | **Status** | **Sí** | 512×614 | `status/index.html` |
| 199174 | Chrome_WidgetWin_1 | **Wispr Flow** | No | 1366×860 | `hub/index.html` |
| 93194198 | Chrome_WidgetWin_1 | **Context Menu** | No | 2048×1112 (a pantalla completa) | `contextMenu/index.html` |
| 329570 | Chrome_WidgetWin_1 | **Meeting Recorder** | No | 634×1112 | (renderer diferido) |
| 81203060 | Chrome_WidgetWin_1 | **Scratchpad** | No | 506×423 | `scratchpad/index.html` |
| 57672262 | Electron_NotifyIconHostWindow | — | No | 0×0 | (tray) |

Todas comparten `Chrome_WidgetWin_1` ⇒ **Chromium multi-view**: 5 `BrowserWindow` frameless con canvas propio.

### 2.2.1 «Status» = el Flow Bar (barra flotante de estado)

Es la superficie que Wispr llama **Flow Bar**. Atributos de datos observados en el DOM (renderer `status`, 228 nodos, 5 `BUTTON`, 3 `svg`, 15 `path`, 2 `circle`, 2 `rect`):

| Atributo | Significado |
|---|---|
| `data-bar-position` | Posición de anclaje (coincide con `prefs.user.statusDockEdge = "bottom"`) |
| `data-status-window-shape` | Forma de la ventana (rect/redondeada/…) |
| `data-status-window-shape-inflate` | Dilatación de la forma (estado pulsado) |
| `data-status-window-native-shape` | Forma nativa de la ventana |
| `data-indicator-state` | Estado del indicador de grabación (inactivo/escuchando/procesando) |
| `data-tour-anchor` | Ancla del tour de onboarding |
| `aria-haspopup` / `aria-expanded` | Botón con menú desplegable |
| `aria-label` (3), `role` (3), `data-testid` (7) | Accesibilidad instrumentada de fábrica |

**Conclusión:** la barra flotante es un **widget de estado redimensionable con forma configurable** (no un simple tooltip), con indicador gráfico vectorial (SVG de 15 paths + waveform de 2 circles/rects). Es la pieza que Wispr permite **arrastrar y anclar** (flag `can-drag-flow-bar`).

### 2.2.2 «Context Menu» = overlay contextual a pantalla completa

2048×1112 = tamaño exacto de la pantalla primaria. Es un overlay **translúcido a pantalla completa** que se abre al pulsar el atajo de apertura del menú contextual. El flag `remote-viewer-paste` (desactivado) sugiere que Wispr contempla **pegar desde un visor remoto** dentro de ese overlay.

### 2.2.3 «Scratchpad» = bloc de notas rápido

506×423. El flag `scratchpad` está **desactivado** en la configuración del servidor (`scratchpad: false`), pero **la ventana existe** ⇒ el flag governs *funcionalidad*, no la existencia del binario. El sidebar muestra «Bloc de notas» como sección de primer nivel.

---

## 2.3 El helper nativo

| Propiedad | Valor |
|---|---|
| Ruta | `app-1.6.1174\resources\Release\Wispr Flow Helper.exe` |
| Tamaño | 19,8 MB |
| Versión | `1.0.0+38f663c09352c1e3d4a297dd007dbba409d237f1` |
| Hilos | 24 |
| Working set | 32,3 MB |
| Padre | PID 4484 (proceso principal) |

**La versión incluye un hash SHA-1 de commit git de 40 caracteres** ⇒ el helper se compila directamente desde un repositorio, y la versión publicada es trazable al commit exacto. [V] (el hash) · [I] (origen git del artefacto).

**Inventario de responsabilidades inferidas** (por flags, esquema de BD y cmdline):

1. **Registro de hotkeys globales** — el main no puede recibir hotkeys cuando no tiene foco; el helper sí. [I]
2. **Captura de micrófono** — la columna `History.audio` es BLOB; el `Helper` es el único proceso con acceso al micrófono a nivel de dispositivo. [I]
3. **Inserción de texto** —Wispr inserta el texto transcrito en la app enfocada; en Windows eso requiere `SendInput`/clipboard, y debe ocurrir con foco de otra aplicación ⇒ proceso aparte. [I]
4. **Integración Jabra** — el módulo nativo `jabra-device-connector.node` permite leer el micrófono del auricular. [V] módulo presente.

> **Implicación Dixi:** estos 4 puntos son exactamente los que `Handy` resuelve en Rust (`global-hotkey`, `cpal`, `enigo`, `paste_tx/windows.rs`) según [14-ALTERNATIVAS-OPEN-SOURCE.md](14-ALTERNATIVAS-OPEN-SOURCE.md). La ventaja de Rust aquí es **un solo proceso** en vez de dos.

---

## 2.4 El motor de datos: 174 migraciones = 2 años de hoja de ruta

`resources/migrations/` contiene **174 archivos `.js`** aplicados en orden; `SequelizeMeta` (174 filas) confirma su aplicación completa. Es la **fuente de requisitos más fiable** del producto: no es documentación, es el registro de lo que se construyó.

### 2.4.1 Línea de tiempo reconstruida (agrupada)

| Periodo | Migraciones clave | Capacidad introducida |
|---|---|---|
| **2024-05 → 2024-08** | `create-dictionary`, `create-history`, `add-e2e-latency-to-history`, `sharing` | Núcleo: diccionario + historial + telemetría de latencia + compartir |
| **2024-10 → 2025-01** | `add-stats`, `add-textbox-contents`, `add-app-version` | Estadísticas, captura del contenido del textbox, versionado |
| **2025-02** | `add-ax-html`, `add_opus_chunks_column` | **Contexto de accesibilidad (AX)** del campo de texto + **audio comprimido en chunks Opus** |
| **2025-03** | `to_replace_dictionary`, `rename_to_replace` | El diccionario pasa de «corregir ortografía» a **sustituir por texto** |
| **2025-04 → 2025-05** | `create-notifications-table`, `create-notes-table` | Notificaciones persistentes, notas |
| **2025-08** | `snippets`, `unique-phrase`, `add-transcript-command` | **Fragmentos (snippets)**, frases únicas, comando de transcripción |
| **2025-09 → 2025-10** | `add-personalization-style`, `add-network-processing-latency`, `add-observed-source` | **Estilos de personalización**, latencia de red, origen observado |
| **2026-01 → 2026-03** | `create_polish_table`, `add-polish-model-version-user-email`, `add-polish-feedback`, `add-polish-used-provider`, `add-polish-prompt-name-shortcut-key`, `add-dictionary-starred` | **Transforms/Polish** (con versionado de modelo, feedback, proveedor), favoritos en diccionario |
| **2026-03 → 2026-04** | `create-note-versions-table`, `add-per-app-stats-index`, `create-note-images-table`, `add-timezone-offset-to-history`, `add-correction-counts-to-history` | Versionado de notas, **estadísticas por app**, imágenes en notas, correcciones |
| **2026-04** | `create-meetings-table`, `create-meeting-versions-table`, `create-links-table`, `add-top-app-url-combinations-index`, `add-meetings-transcript`, `add-notes-pinned`, `add-links-pinned-column`, `add-notes-searchable-content` | **Reuniones (notetaker)**, enlaces, **FTS sobre notas** |
| **2026-04 → 2026-05** | `create-calendar-events-table`, `add-summary-columns`, `create-user-context-table`, `create-instruct-history-table`, `create-user-voice-preferences-table` | Calendario, **resúmenes**, contexto de usuario, historial de instrucciones, **preferencias de voz** |
| **2026-06** | `create-todos-table`, `instruct-history-tool-calls`, `add-meetings-participant-names`, `create-automations-table` | **Tareas (todos)**, llamadas a herramientas, **automatizaciones** |
| **2026-06 → 2026-07** | `add-meetings-import-source`, `create-notetaker-chat-tables`, `add-history-edit-distance`, `add-meetings-share-columns`, `create-shared-notes-table`, `create-notetaker-chat-agent-states` | **Importación de reuniones**, **chat con el notetaker**, **distancia de edición** (métrica de calidad de formato), notas compartidas |
| **2026-07 → 2026-09** | `create-folders-table`, `create-folder-meetings-table`, `add-meetings-org-insights`, `add-meetings-summary-feedback`, `add-folders-sharing-blocked` | **Carpetas**, insights organizacionales, feedback de resúmenes |
| **2026-08 → 2026-10** | `add-transcript-corrections`, `add-calendar-event-series-key`, `add-calendar-event-notetaker-controls`, `add-calendar-event-alternate-conference-url`, `add-meeting-room-label-edits`, `create-usage-outbox` | Correcciones de transcripción, **eventos de calendario recurrentes**, controles de notetaker en calendario, **outbox de uso** (telemetría diferida) |

**Conclusiones estructurales:**

1. **El producto se dividió en dos apps con identidades separadas:** *Flow* (dictado, hasta 2025-09) y *Notetaker* (reuniones, desde 2026-04), sobre la misma base de datos. El `History` pertenece al flujo de dictado; `Meetings`/`Notes`/`Folders` al notetaker.
2. **El formateo por IA ("Polish") llegó tarde (2026-01)** y se 최저ó como *producto de pago/modelo/versionado de proveedor* ⇒ Wispr trata el LLM como un **servicio con coste por uso**, no como una capacidad local.
3. **`add_ax-html` (2025-02) es la pieza clave de la «conciencia contextual»**: Wispr captura el **contexto de accesibilidad** del campo de texto donde se dicta, lo que habilita la corrección de nombres propios y la adaptación de estilo. `prefs.user.useAxContext = true` en la instalación auditada.
4. **`add-network-processing-latency` (2025-10)** ⇒ Wispr mide por separado la latencia de red, lo que confirma un pipeline cloud descompuesto en fases medibles.

---

## 2.5 Esquema de datos: las 29 tablas

Extraído con `sqlite3` (sólo metadatos, sin filas de contenido). Detalle completo en [03](03-FUNCIONES-DICTADO.md) §2 y [08](08-REUNIONES-NOTAS.md) §2.

| Tabla | Filas | Rol | Tabla | Filas | Rol |
|---|---|---|---|---|---|
| `History` | **3 160** | Cada dictado | `Polish` | 0 | Transformaciones IA |
| `Dictionary` | 43 | Diccionario + fragmentos | `InstructHistory` | 0 | Historial de instrucciones IA |
| `Meetings` | 5 | Reuniones grabadas | `InstructChatSession` | 0 | Sesiones de chat instruct |
| `MeetingVersions` | 0 | Versiones de reunión | `NotetakerChats` / `NotetakerChatMessages` / `NotetakerChatAgentStates` | 0 / 0 / 0 | Chat con el notetaker |
| `Notes` / `NoteVersions` / `NoteImages` | 0 / 0 / 0 | Notas, versionado, imágenes | `Todos` | 0 | Tareas extraídas |
| `Folders` / `FolderMeetings` | 0 / 0 | Carpetas | `Links` | 0 | Enlaces guardados |
| `CalendarEvents` | 0 | Eventos de calendario | `SharedNotes` | 0 | Notas compartidas |
| `GranolaImportRun` / `GranolaTranscriptQueue` | 0 / 0 | Importación Granola | `UserContext` | 0 | Contexto del usuario (para IA) |
| `UserVoicePreferences` | 0 | Preferencias de voz/estilo | `Automations` | 0 | Automatizaciones |
| `RemoteNotifications` | 19 | Notificaciones remotas | `TranscriptCorrections` | 0 | Correcciones de transcripción |
| `UsageOutbox` | 0 | Cola de telemetría diferida | `FlowLensHistory` | 0 | Historial del Flow Lens |
| `SequelizeMeta` | 174 | Control de migraciones | | | |

**Observaciones de diseño para Dixi:**

- `History` tiene **63 columnas** para un solo dictado: es un **evento de registro amplio**, no una fila simple. Incluye: texto ASR, texto formateado, texto editado, audio (BLOB), chunks Opus (JSON), captura de pantalla (BLOB), contexto AX (`axText`, `axHTML`), contenido del textbox, latencias (e2e, red), conteos de correcciones, distancia de edición, preferencias de estilo aplicadas, estado de fallback, origen de transcripción, comando, razón de fin de observación de contenido, tz offset, `hasRevertedAI`, `feedback`, `shareType`, `isArchived`, `micDevice`. [V]
- El modelo es **`audio-first`**: `audio` (BLOB) + `opusChunks` (JSON) ⇒ se conserva el audio **y** su versión comprimida ⇒ 755 MB para 3 160 dictados. [V]
- Las tablas del notetaker están **separadas** de `History` ⇒ **Dixi puede replicar la separación** (diktado vs reuniones) sin acoplamiento.
- `SequelizeMeta` + migraciones `.js` ⇒ el patrón de migración de Wispr es exactamente el propuesto para Dixi (SQLite + migraciones versionadas).

---

## 2.6 El inspector de accesibilidad embebido (`ax-inspect`)

El bundle incluye **tres scripts de depuración** en `resources/`:

| Archivo | Tamaño | Función |
|---|---|---|
| `ax-inspect.mjs` | 3,7 KB | CLI que lee `flow.sqlite` y muestra el **contexto AX** asociado a un dictado |
| `ax-inspect-lib.mjs` | 92,5 KB | Biblioteca de lectura de la BD y de los sidecars de traza |
| `ax-inspect-server.mjs` | 5,2 KB | Servidor de Always-on refresh con la lista de dictados |

**Hallazgo de arquitectura (alto valor para Dixi):** el script expone el **esquema real de trazas en disco** bajo el directorio de usuario. Por cada dictado, Wispr escribe *sidecars* de traza:

| Sidecar | Contenido |
|---|---|
| `paste` | Lo que se pegó en la app destino |
| `noun` | Sustitución de sustantivos/proper nouns |
| `textbox` | Contexto del campo de texto |
| `verify` | Verificación de la inserción |
| `event` | Eventos de la sesión |
| `edit` | Ediciones posteriores del usuario |
| `context` | Contexto capturado |

**Implicación para Dixi:** Wispr mantiene un **sistema de trazas por dictado** independiente de la base relacional. Es un diseño excelente para depuración y para un modo «verbose» de Dixi, y explica por qué el `History` guarda tanto `axText` como `axHTML`.

> **Nota de seguridad:** el `ax-inspect` se ejecutó **sólo en modo de lectura estructural** sobre una **copia** de la BD en `.tmp-audit/db/`. No se extrajeron transcripciones reales.

---

## 2.7 Catálogo de feature flags (superficie de producto servida desde el servidor)

`feature-flags-cache.json` (40,7 KB) es la **caché de flags servidos por la API**, con *payloads de configuración por función*. Es, en la práctica, un **catálogo del producto**. Extracto de las flags relevantes para Dictado/Windows:

| Flag | Estado | Señal de producto |
|---|---|---|
| `notetaker-windows` | **enabled** | Notetaker soportado en Windows |
| `granola-import` / `otter-import` | enabled | Importación de reuniones desde Granola y Otter |
| `ask-anything-mcp-hint` | enabled | **Superficie MCP** |
| `windows-caps-lock-shortcut` | enabled | Atajo con Caps Lock |
| `windows-shortcut-masking-v2` | enabled | **Enmascaramiento de atajos** (evita colisión con otras apps) |
| `windows-key-up-simulation` | enabled | Simulación de key-up (Windows) |
| `focus-change-detector-app-change` | enabled | **Detección de cambio de app en foco** |
| `auto-polish-on-short-text` | enabled (select-all) | Polish automático en textos cortos |
| `effective-language-picker` | enabled | Selector de idioma efectivo |
| `notetaker-folders` | enabled | Carpetas de reuniones |
| `notetaker-live-transcript` | enabled | Transcripción en vivo |
| `notetaker-audio-settings` | enabled | Ajustes de audio del notetaker |
| `notetaker-hub-ui-improvements` | enabled | Mejoras de UI del hub |
| `notetaker-transcript-romanization` | enabled | Romanización de transcripción |
| `notetaker-system-audio-probe` | enabled | **Sonda de audio del sistema** |
| `notetaker-audio-switching` | disabled | Cambio de fuente de audio |
| `notetaker-seat-controls` | disabled | Controles por asiento (empresa) |
| `notetaker-l2` / `notetaker-retention` | disabled | Funcionalidad empresarial/retention |
| `org-insights` | disabled | Insights organizacionales |
| `domain-capture` | disabled | Captura por dominio |
| `logitech-actions` | disabled | Integración con acciones Logitech |
| `flowbar-processing` | disabled | Indicador de «procesando» en la Flow Bar |
| `Flow-State-Generation` | disabled | Generación de estado de flujo |
| `multi-keybind-service` | disabled | Servicio de multi-atajo |
| `remote-viewer-paste` | disabled | Pegado desde visor remoto |
| `paste-sound` | disabled | Sonido de pegado |
| `automations` | disabled | Automatizaciones |
| `connectors` | disabled | Conectores |
| `todos` | disabled | Tareas (la tabla `Todos` existe) |
| `scratchpad` | disabled | Scratchpad (la ventana existe) |
| `extension-system` | disabled | Sistema de extensiones |
| `cursor-integration` | disabled | Integración con Cursor |
| `gpu-crash-guard` | enabled | Guardia contra crash de GPU |
| `defer-updates-while-active` | enabled | **No actualizar mientras se usa** |
| `daily-streak-desktop` / `desktop-team-leaderboard` | enabled | Gamificación |
| `new-insights-and-voice-profile` | enabled (variant `insights`) | Nuevos insights + perfil de voz |
| `hub-top-apps` | enabled | Apps más usadas en el hub |
| `edge-proxy` | enabled | Proxy perimetral |
| `proactive-incident-alerts` | enabled | Alertas proactivas de incidencias |
| `student-plan-free` | enabled | Plan estudiantil gratuito |
| `member-self-serve-data-export` / `…-usage-data-erasure` | enabled | Exportación/borrado de datos por el miembro |
| `weekly-word-limits-config` | enabled | Límites semanales (escritorio: soft 2 000 / hard 5 000 palabras) |
| `use-wispr-qwerty` | enabled | Distribución de teclado Wispr |

### 2.7.1 Payload del flag `meeting-recorder` (extracto)

```json
{
  "serverAudioRetention": true,
  "live": "source",
  "summaryFeedback": true,
  "shareMeeting": true,
  "speakerLlmAttribution": "full-aggressive",
  "transcriptEditing": true,
  "bypassCloudSync": true,
  "calendarNotice": true,
  "splitScreen": true,
  "liveTranslation": true,
  "shareControls": true,
  "guidedTour": true,
  "identityRestampEnabled": true,
  "notetaker-system-audio-probe": true,
  "confRoomWalkbackReachSeconds": 60,
  "localAudioRetentionDays": 14,
  "autoStartNotetaker": true,
  "autoStartPromptThreshold": 3,
  "notetakerMaxRecordingLength": "2hr",
  "notetakerTranscriptRetention": "never_delete",
  "fusionTuning": {
    "nativeHealWindowMs": 6000,
    "ultraHighWindowMs": 2000,
    "proofGatedUltraHolders": ["zoom", "teams"],
    "clickProofHolders": ["zoom", "teams"]
  },
  "browserLeaveClick": "enabled",
  "nativeAnchorShadow": true
}
```

**Lectura de arquitectura (muy valiosa):**

- `fusionTuning.nativeHealWindowMs = 6000` y `ultraHighWindowMs = 2000` ⇒ Wispr implementa un **algoritmo de fusión/reconciliación** entre la transcripción en vivo (streaming) y la final, con ventanas de "sanación" de 6 s y ventana "ultra-alta" de 2 s, y **pruebas de evidencia** (`clickProofHolders`, `proofGatedUltraHolders`) para los holders de reunión detectados por click en Zoom/Teams. [V para los valores; I para la interpretación]
- `walkbackReachSeconds = 60` ⇒ ventana de retroceso para corregir los últimos 60 s. [V]
- `speakerLlmAttribution = "full-aggressive"` ⇒ la atribución de hablantes se delega al LLM de forma agresiva. [V]
- `localAudioRetentionDays = 14` y `notetakerTranscriptRetention = "never_delete"` ⇒ política dual: audio local 14 días, transcripción nunca. [V]
- `bypassCloudSync: true` ⇒ los ajustes del notetaker pueden saltarse la sincronización cloud. [V]
- `autoStartNotetaker: true` + `autoStartPromptThreshold: 3` ⇒ **inicio automático** del notetaker tras 3 reuniones detectadas. [V]

> **Implicación para Dixi:** la fusión streaming/final con ventanas de sanación y la atribución de hablantes son **las dos piezas más complejas** del notetaker y no están cubiertas por ningún proyecto open source evaluado (ver [14](14-ALTERNATIVAS-OPEN-SOURCE.md) §5). Son la principal brecha técnica de Dixi.

---

## 2.8 Superficie de IPC / protocolo propio

- Esquema registrado: `wispr-extension://` (`--standard-schemes` / `--secure-schemes`) ⇒ **extensión de navegador** que se comunican con la app de escritorio.
- `sentry-ipc` con `bypasscsp-schemes` ⇒ canal interno de reporte de errores.
- `shared_proto_db/` en el perfil ⇒ **protobuf** compartido entre procesos (probablemente el bus de telemetría/eventos).
- El helper nativo se comunica con el proceso principal por un mecanismo **no visible desde fuera** (no aparece en la lista de puertos loopback) ⇒ probablemente **named pipe** o IPC de Electron vía `utilityProcess`/sockets de Node.

---

## 2.9 Cambios de Chromium observables desde la app

En el arranque de la instancia aislada se registran en `isolated.log`:

```
[ERROR:gpu_process_host.cc(1236)] GPU process isn't usable. Goodbye.
at ChromeWin: .../chrome/electron/.../browser/...
    …\app.asar\.webpack\main\index.js
```

Esto confirma:

1. El **proceso GPU de Chromium falla** en esta máquina (driver Intel 2018 + Vulkan por software) ⇒ Chromium cae al **SwiftShader por software** (`vk_swiftshader.dll`, 5,4 MB presente). [V]
2. El stack de error pasa por el **bundle webpack del proceso principal** (`app.asar/.webpack/main/index.js`) ⇒ **webpack bundling confirmado también en el proceso main**, no sólo en los renderers. [V]
3. Wispr **inyecta la confianza del almacén de sistema de Windows (174 certificados) en el trust store de Node** ⇒ soporte para TLS inspeccionado por proxies corporativos (Avast/AnyDesk activos en la máquina). [V] (mensaje «174 certificates were applied to the trust store»).

> **Implicación para Dixi:** un stack Tauri/Rust no arrastra GPU process ni utility processes: en una máquina con GPU degradada, **Dixi parte con ventaja de consumo y estabilidad**.

---

## 2.10 Síntesis: qué replicar, qué descartar

| Componente Wispr | Decisión para Dixi | Justificación |
|---|---|---|
| Electron + 4 renderers | **Descartar** → Tauri 2 (WebView del SO) | −12 procesos, −GPU process, −utility services |
| Helper nativo separado | **Integrar en el proceso Rust** | `global-hotkey` + `cpal` + `enigo` en un binario |
| `flow.sqlite` (755 MB) | **Rediseñar**: audio opcional + retenible | El audio-first consume 240 KB/dictado sin Benefit claro |
| ASR cloud | **Híbrido**: local por defecto, cloud opcional | Latencia p50 640 ms → objetivo < 300 ms local |
| Migraciones `.js` + Sequelize | **Adoptar patrón**: SQL versionado en Rust | Curva de aprendizaje cero, ya validadas 174 iteraciones |
| Feature flags por servidor | **Reemplazar por config local versionada** | Evita dependencia de servidor, permite.diffusion |
| Sistema de sidecars de traza | **Adoptar** (`~/.dixi/traces/<id>/`) | Excelente para depuración y para un modo verbose |
| `wispr-extension://` | **Opcional**: extensión de navegador para contexto web | Valor real para dictar en el navegador |
| Ax-inspect (depuración) | **Adoptar como `dixi-inspect`** | Mismo valor, código propio |

---

### Referencias cruzadas

- [01-ENTORNO.md](01-ENTORNO.md) — sistema, instalación, procesos
- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — pipeline de dictado
- [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md) — notetaker y notas
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — preferencias
- [14-ALTERNATIVAS-OPEN-SOURCE.md](14-ALTERNATIVAS-OPEN-SOURCE.md) — qué código reutilizar
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas priorizadas
