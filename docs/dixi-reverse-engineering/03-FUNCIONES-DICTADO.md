# 03 — Funciones de dictado: pipeline, modos y atajos

> Reconstrucción del pipeline de dictado de Wispr Flow 1.6.1174 a partir del esquema real de la tabla `History` (3 160 dictados reales, agregados numéricos), la tabla de atajos (`prefs.user.shortcuts`), los flags de servidor y la estructura de sidecars de traza.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 3.1 El modelo de datos de un dictado

`History` es la tabla central. **63 columnas** para un solo evento de dictado — no es una fila simple, es un **registro de evento completo**.

### 3.1.1 Identidad y ciclo de vida

| Columna | Tipo | Rol |
|---|---|---|
| `transcriptEntityId` | VARCHAR(36) PK | UUID del dictado |
| `timestamp` | DATETIME | Momento del dictado |
| `status` | VARCHAR(255) | `formatted` · `raw_transcript` · `no_audio` · `dismissed` · `empty` |
| `isArchived` | TINYINT(1) | Archivado manual |
| `conversationId` | VARCHAR(255) | Agrupa dictados en una conversación |
| `shareType` | TEXT | `no` · `shared` |
| `feedback` | TEXT | Feedback del usuario (positivo/negativo) |

### 3.1.2 Cadena de texto: cinco estados del mismo dictado

| Columna | Contenido | Significado |
|---|---|---|
| `asrText` | **Salida cruda del ASR** | Lo que el modelo reconoció, sin formatear |
| `formattedText` | **Salida del formateador** | Texto con puntuación/mayúsculas/limpieza |
| `editedText` | **Texto tras la edición del usuario** | La verdad final |
| `toneMatchedText` | Resultado del **emparejamiento de tono** | Texto detectado como que debe llevar un tono formal/informal |
| `toneMatchPairs` | JSONB | Pares original↔tono detectados |
| `serverFinalizedText` | Versión final **del servidor** | Reconciliación cliente/servidor |
| `defaultAsrText` / `fallbackAsrText` | ASR primario vs **de respaldo** | Modelo alterno |
| `defaultFormattedText` / `fallbackFormattedText` | Formato primario vs respaldo | |
| `desiredAsr` / `desiredFormatted` | Lo que el usuario **quedría** haber obtenido | Referencia de calidad |
| `editedTextUnbounded` | Texto editado sin truncamiento | Para textos largos |
| `pastedText` | Lo que realmente se **pegó** en la app destino | Verificación de extremo a extremo |
| `textboxContents` | Contenido previo del campo de texto | Permite detectar colisiones al pegar |
| `hasRevertedAI` | TINYINT(1) | Si el usuario **revirtió** la IA |

> **Hallazgo clave:** Wispr no sólo dicta: **distingue 5 estados del mismo contenido** (crudo / formateado / editado / con tono / final del servidor) y además registra **dos modelos en paralelo** (primario + respaldo) con sus divergencias. Esto es un **sistema de calidad de producto**, no sólo un grabador. [V]

### 3.1.3 Métricas de calidad (la parte más útil para Dixi)

| Columna | Interpretación |
|---|---|
| `editDistanceToDictated` | Distancia de edición entre **dictado** y **editado** ⇒ métrica de calidad del formateo |
| `formattingDivergenceScore` | Divergencia frente al formateo por defecto |
| `fallbackAsrDivergenceScore` / `fallbackFormattingDivergenceScore` | Divergencia frente al modelo de respaldo |
| `numWordsCorrected` | Palabras corregidas por el formateo |
| `numDictionaryReplacements` | Sustituciones aplicadas desde el **diccionario** |
| `editedTextStatus` | `NOT_EXTRACTED` por defecto ⇒ Wispr **aún no extrae** el texto editado en esta instalación (0/3 160) |
| `editedTextAttempts` | Intentos de extracción |
| `averageLogProb` | Confianza media del ASR (¡métrica de ASR en producción!) |
| `contentObservationEndReason` / `contentObservationEndLastKeystroke` | Razón de fin de observación de contenido + última tecla ⇒ **detección de "el usuario ya escribió"** |

### 3.1.4 Audio (el motivo de los 755 MB)

| Columna | Contenido |
|---|---|
| `audio` | BLOB — audio crudo |
| `builtInAudio` | BLOB — audio del **motor integrado** (vs. el del micrófono externo) |
| `opusChunks` | JSON — **segmentos Opus** (codificación por chunks para subida) |
| `duration` | Duración total del audio (s) |
| `speechDuration` | Duración **de habla** (s) — sin silencios |
| `micDevice` | Dispositivo de micrófono usado |
| `screenshot` | BLOB — captura de pantalla asociada |
| `additionalContext` | JSON — contexto adicional |

> `opusChunks` (migración `add_opus_chunks_column`, 2025-02-19) revela un **pipeline de subida incremental**: el audio se chunk-ea en Opus y se transmite por partes mientras se dicta. [I]

### 3.1.5 Contexto de la aplicación destino

| Columna | Contenido |
|---|---|
| `app` | Identidad de la app en foco |
| `url` | URL (navegador) |
| `platform` | SO |
| `appVersion` | Versión de Wispr quePerformed el dictado |
| `axText` | **Texto de accesibilidad** del campo enfocado |
| `axHTML` | **HTML de accesibilidad** del campo enfocado |
| `personalizationStyleSettings` | JSON — **ajustes de estilo aplicados** en ese dictado |
| `transcriptCommand` | Comando de transcripción (p. ej. un comando de Transform) |
| `transcriptOrigin` | Origen: `canto_1.0` · `canto_1.0.1` · `internal` · `internal_experimental` · `external` |
| `calledExternalAsr` | TINYINT(1) — se usó un ASR externo |
| `usedFallbackAsr` / `usedFallbackFormatting` / `fallbackLevel` | Uso y nivel del modelo de respaldo |
| `clientNetworkLatency` | **Latencia de red medida en cliente** (ms) |
| `e2eLatency` | **Latencia extremo a extremo** (ms) |
| `timezoneOffsetMinutes` | TZ del cliente |

> **Hallazgo de alto valor:** `transcriptOrigin` con valores `canto_1.0` (1 974 dictados), `canto_1.0.1` (507), `internal` (157), `internal_experimental` (470) y `external` (4) ⇒ **«Canto» es el nombre interno de la línea de modelos de ASR/formato de Wispr**, con versiones `1.0` → `1.0.1` → `experimental`. La migración `internal` → `internal_experimental` sugiere un **A/B test de modelos en producción**. [V] para los valores; [I] para la interpretación.
>
> ✅ **CONFIRMADO contra la documentación pública [V]:** Wispr **publicó su propio modelo de ASR, «Canto», en septiembre de 2026**, con la afirmación (del propio fabricante, no independiente) de que logra el **WER más bajo** frente a Google, OpenAI, AssemblyAI y Deepgram. La hipótesis «Canto = línea interna de modelos» queda **verificada**.
>
> ✅ **CONFIRMADO también [V]:** la documentación de controles de datos de Wispr dice literalmente ***«Transcription always occurs on the cloud.»*** y ***«Dictation is transcribed in the cloud.»*** ⇒ **no existe ASR en el dispositivo en ninguna modalidad**; la única opción local es el *historial*, no la transcripción. Esto confirma la premisa central de esta auditoría: **Dixi compite con un pipeline cloud contra un pipeline local.**

### 3.1.6 Distribución real observada (3 160 dictados)

**Estado:** `formatted` 3 086 (97,7 %) · `raw_transcript` 33 · `no_audio` 8 · `dismissed` 3 · `empty` 1.
⇒ El **97,7 % de los dictados llega formateado**; sólo 33 llegaron como transcripción cruda. [V]

**Modelo de respaldo:** `usedFallbackAsr` y `usedFallbackFormatting` = **0 / 3 160** ⇒ el pipeline cloud de Wispr fue 100 % fiable en la muestra. [V]

**Origen:** `canto_1.0` 1 974 · `canto_1.0.1` 507 · `internal_experimental` 470 · `internal` 157 · `external` 4. [V]

**Aplicaciones destino (top):**

| App | Dictados | Nota |
|---|---|---|
| `chrome` | 1 685 | Navegador |
| `WindowsTerminal` | 370 | Terminal |
| `warp` | 343 | Terminal Warp |
| `ms-teams` | 240 | Microsoft Teams |
| `notepad++` | 230 | Editor |

Identidades adicionales detectadas: `Antigravity`, `WhatsApp.Root`, `SndVol`, `Studio by Spotify Labs`, `chrome_proxy`. [V]

> **Implicación para Dixi:** Wispr **identifica la aplicación de destino con precisión** (incluye aplicaciones .NET/Win32 como `SndVol` y `Studio by Spotify Labs`, que no exponen información de proceso trivial). El módulo de detección de app + contexto AX es un componente a replicar. [V]

**Idioma detectado:** `es` 3 075 · `en` 8. [V]

---

## 3.2 Pipeline de dictado reconstruido

```
┌─ CAPTURA ──────────────────────────────────────────────────────────────┐
│  Helper nativo (PID 16872)                                            │
│   • Códec: ffmpeg.dll → Opus                                         │
│   • Dispositivo: enumeración + ranking (prefs.rankedAudioDevices)    │
│   • mic.pcm crudo → opusChunks (JSON) → subida incremental           │
└───────────────────────────────────────────────────────────────────────┘
┌─ ENRIQUECIMIENTO (local) ──────────────────────────────────────────────┐
│  • app + url + ventana enfocada        (focus-change-detector)       │
│  • axText / axHTML del campo (useAxContext=true)                     │
│  • textboxContents previo                                        │
│  • estilo aplicable (personalizationStyleSettings)                    │
└───────────────────────────────────────────────────────────────────────┘
┌─ NUBE Wispr AI ───────────────────────────────────────────────────────┐
│  • ASR (Canto 1.0/1.0.1) → asrText          [clientNetworkLatency]   │
│  • Formateo            → formattedText      [formattingDivergence]   │
│  • Detector de tono    → toneMatchedText                               │
│  • Servidor final      → serverFinalizedText                           │
│  • Modelo de respaldo  → fallbackAsrText / fallbackFormattedText      │
└───────────────────────────────────────────────────────────────────────┘
┌─ POST-PROCESO (local) ────────────────────────────────────────────────┐
│  • Comparación con personalizedStyleSettings                          │
│  • Reemplazos del diccionario (numDictionaryReplacements)            │
│  • Detección de fin de observación de contenido (lastKeystroke)        │
└───────────────────────────────────────────────────────────────────────┘
┌─ INSERCIÓN ───────────────────────────────────────────────────────────┐
│  Helper nativo: clipboard + SendInput → app destino                  │
│  • Verificación: pastedText vs editedText                            │
│  • Sidecars de traza: paste / noun / textbox / verify / event / edit  │
└───────────────────────────────────────────────────────────────────────┘
```

**Medición de latencias (n = 3 160, segundos salvo indicación):** ver [13-RENDIMIENTO.md](13-RENDIMIENTO.md) §3.

| Métrica | min | p50 | p90 | p99 | max | media |
|---|---|---|---|---|---|---|
| `e2eLatency` (ms) | 0 | **640** | 1 051 | 1 709 | 32 018 | 728,8 |
| `clientNetworkLatency` (ms) | 0 | **270** | 299 | 419 | 1 657 | 281,1 |
| `duration` (audio) | 0,12 | 10,08 | 33,14 | — | 349,4 | — |
| `speechDuration` | 0,04 | 9,92 | 31,94 | — | 348,0 | — |
| **e2e − habla (ms)** | — | **628** | **1 021** | **1 636** | — | — |

> **Lectura de arquitectura:** la latencia de red (p50 270 ms) es el **42 %** de la latencia e2e (p50 640 ms). El resto (~370 ms) es captura + framing + **formateo cloud** + inserción. El formateo **no es local**: Wispr formatea en la nube (lo confirma `formattedText` vs `asrText` separados y `clientNetworkLatency` como fase medida). [I]
>
> **Objetivo de Dixi:** ASR local + formateo local con un LLM pequeño ⇒ p50 objetivo < 300 ms de latencia percibida.

---

## 3.3 Atajos de teclado (mapa completo, verificado)

Extraído de `prefs.user.shortcuts`. Códigos de tecla virtual de Windows + flags de modificador.

| Combinación | Acción (`shortcuts`) | Tecla virtual |
|---|---|---|
| **Ctrl + `** | `ptt` — **push-to-talk** (hablar mientras se mantiene) | 162 (LCtrl) + 91 (`) |
| **Ctrl + Espacio + `** | `popo` — **overlay desplegable** | 162 + 32 + 91 |
| **Ctrl + Alt + `** | `lens` — **Flow Lens** | 162 + 164 (LAlt) + 91 |
| **Alt + Mayús + Z** | `paste_last_text` — pegar el último dictado | 164 + 160 (LShift) + 90 |
| **Alt + Mayús + X** | `copiar_last_text` — copiar el último dictado | 164 + 160 + 88 |
| **Alt + M + `** | `open_meeting_recorder` — abrir el grabador de reuniones | 164 + 77 (M) + 91 |
| **Alt + 1 + `** | `polish` — Transform rápido | 164 + 49 (1) + 91 |
| **Alt + 2 + `** | `polish_prompt_1` — prompt personalizado #1 | 164 + 50 (2) + 91 |
| **Alt + P + `** | `view_diff` — **ver el diff** de la IA | 164 + 79 (P) + 91 |
| **Esc** | `dismiss` — descartar | 27 |

Además: `prefs.user.modifierShortcut = 164` (Alt) ⇒ existe un **modo modificador sostenido** (mantener Alt y hablar).

**Flags asociados:** `windows-caps-lock-shortcut` (enabled) ⇒ atajo alternativo con Caps Lock; `windows-shortcut-masking-v2` (enabled) ⇒ **enmascaramiento de atajos** para evitar que otras apps capturen las teclas; `windows-key-up-simulation` (enabled) ⇒ simulación de key-up.

**Convención de diseño noteworthy:** la tecla ` (91, VK_OEM_4) es el **ancla** de todos los atajos de_CONTROL, lo que hace la combin improbable pero accesible: `Ctrl+Alt+Alt+M+` sería un conflicto. La convención real es **«Ctrl como modificador base + ` como tecla de función»**, con Alt/Ctrl+Espacio como Launchers de overlay.

> **Implicación para Dixi:** Hotkeys de Windows reserved (`Ctrl+Alt+…` se usa para cambio de idioma regional) crean colisiones. El flag `windows-shortcut-masking-v2` muestra que Wispr investierte en esto. [V] flag, [I] interpretación.

---

## 3.4 Modos de operación

| Modo | Disparo | Evidencia |
|---|---|---|
| **Push-to-talk (PTT)** | mantener `Ctrl+`` | atajo `ptt` [V] |
| **Modo modificador sostenido** | mantener `Alt` (`modifierShortcut=164`) y hablar | `prefs.user.modifierShortcut` [V] |
| **Overlay / popover** | `Ctrl+Espacio+`` | atajo `popo` [V] |
| **Flow Lens** | `Ctrl+Alt+`` | atajo `lens` + tabla `FlowLensHistory` [V] |
| **Context Menu** | ventana `contextMenu` a pantalla completa (2048×1112) | enumeración de ventanas [V] |
| **Flow Bar (barra flotante)** | siempre visible (ventana `Status` 512×614) | ventanas [V] |
| **Scratchpad** | ventana dedicada 506×423 | ventanas [V] |
| **Hands-free / whisper** | flag `windows-caps-lock-shortcut` | flags [V] |
| **Dictado por comando** | `transcriptCommand` en `History` | esquema [V] |
| **Modo foco / sin interrupciones** | `prefs.user.focusModeEnabled` + `focusModeBlockedApps/Urls` | config [V] |
| **Detección de cambio de app** | flag `focus-change-detector-app-change` | flags [V] |
| **Detección de voz** | `prefs.user.shouldAutoLearnWords` = true | config [V] |

---

## 3.5 «Flow Lens», «Ask anything» y Command Mode

| Concepto | Evidencia | Estado |
|---|---|---|
| **Flow Lens** (`lens`) | atajo `Ctrl+Alt+`` + tabla `FlowLensHistory` (0 filas) |[I] Probably overlay de revisión/edición del último dictado |
| **Ask anything** | input visible en la captura de pantalla de la sesión («Ask anything» + «Full access») | [V] visible en el overlay |
| **Command Mode** | `prefs.user.commandModeEnabled = false`, `pressEnterCommandEnabled = false` | [V] existe, está desactivado |
| **MCP** | flag `ask-anything-mcp-hint` (enabled) + botón «Configurar MCP» en el hub | [V] |

> `prefs.user.notetakerMCPVisibilityState = "ModalDismissed"` ⇒ el usuario ya vio y cerró el modal de MCP ⇒ **el MCP está en la UI y es dismissed, no ausente**. [V]

Detalle en [10-MCP.md](10-MCP.md).

---

## 3.6 Contexto de accesibilidad (AX): la pieza diferencial

`prefs.user.useAxContext = true` ⇒ **captura activa** del contexto AX del campo enfocado.

**Cómo funciona (reconstrucción):**

1. El helper determina la ventana en foco.
2. Se solicita al renderer de esa ventana el **árbol de accesibilidad** de los elementos editables.
3. Se extrae `axText` (texto plano) y `axHTML` (HTML con roles) del control enfocado.
4. Se envía junto al audio al backend, para que el formateo pueda:
   - capitalizar y corregir **nombres propios** presentes en el contexto,
   - inferir el **tipo de contenido** (código, prosa, email, chat),
   - adaptar el estilo.

**Tabla `Dictionary` columna `noun`** y el sidecar `noun` (de `ax-inspect.mjs`) confirman que Wispr tiene una **sustitución de sustantivos propios** guiada por el AX. [V]

**Implicación para Dixi:** la captura AX en Windows es directa con **UI Automation** (`UIAutomationClient`), y es lo que permite a Dixi competir en calidad de capitalización de nombres propios sin un modelo grande. En Linux/macOS el equivalente es AX (macOS) / AT-SPI (Linux).

---

## 3.7 Limpieza automática

`prefs.user.autoCleanup.level = "light"` (por defecto) ⇒ nivel de limpieza configurable (ligero/estándar/agresivo). `add_personalization-style` (2025-09) y la tabla `UserVoicePreferences` (2026-05) confirman que el estilo es un **perfil**, no un interruptor.

Detalle en [06-ESTILOS.md](06-ESTILOS.md).

---

## 3.8 Límites de cuota observados

| Límite | Valor | Fuente |
|---|---|---|
| Palabras/semana (desktop) | soft **2 000** / hard **5 000** | flag `weekly-word-limits-config` [V] |
| Reuniones/semana | **150** | `prefs.user.notetakerQuota = {counted:0, limit:150, remaining:150}` [V] |
| Duración de grabación de reunión | **2 h** | `notetakerMaxRecordingLength = "2hr"` [V] |
| Retención de audio local de reunión | **14 días** | `localAudioRetentionDays = 14` [V] |
| Retención de transcripción de reunión | **nunca** | `notetakerTranscriptRetention = "never_delete"` [V] |
| Límite de palabras attained | 40 (uso) | `userJourney.features.formatting.usageCount = 40` [V] |

---

## 3.9 Lo que Dixi necesita replicar de este módulo

| # | Elemento | Complejidad | Referencia open source |
|---|---|---|---|
| 1 | Captura de micrófono + Opus por chunks | Media | `handy-recorder` (MIT) |
| 2 | PTT con hotkey global | Baja | `global-hotkey` (Rust) |
| 3 | Inserción de texto en la app enfocada | **Alta** (Windows) | `enigo` + `paste_tx/windows.rs` de Handy |
| 4 | Detección de app + texto del campo | Media | **No cubierto** — replicar con UIA |
| 5 | ASR local (Parakeet/Whisper GGUF) | Media | `transcribe-cpp` (MIT) |
| 6 | Formateo local (reglas + LLM opcional) | Media | Reglas propias + `llama.cpp` |
| 7 | Sustitución de nombres propios vía AX | **Alta** | **No cubierto** |
| 8 | Sistema de sidecars de traza | Baja | Diseño propio (copiar patrón) |

---

### Referencias cruzadas

- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — capas y ventanas
- [04-DICCIONARIO.md](04-DICCIONARIO.md) — diccionario y snippets
- [06-ESTILOS.md](06-ESTILOS.md) — estilos y voces
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — preferencias
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — latencias y consumo
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
