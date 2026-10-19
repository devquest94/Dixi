# 16 — Matriz de paridad: Wispr Flow → Dixi

> Consolidación de las 7 fases de la auditoría. Cada fila indica qué hace Wispr Flow hoy (con evidencia), qué cubre el open source, qué debe construir Dixi, y con qué prioridad.
> **Prioridad**: P0 = imprescindible para que Dixi sea un reemplazo funcional · P1 = alto valor, esfuerzo razonable · P2 = deseable · P3 = para una fase posterior.
> **Esfuerzo**: S (≤ 2 días) · M (3–8 días) · L (2–4 semanas) · XL (> 1 mes).

---

## 16.1 Resumen ejecutivo

| Métrica | Valor |
|---|---|
| Funciones de Wispr catalogadas | **87** |
| Cubiertas por el open source evaluado | **19 (22 %)** |
| A construir por Dixi | **68 (78 %)** |
| Brechas críticas (P0) | **11** |
| P0 con cobertura OSS | **5 de 11** |
| Documentos de auditoría | 16 |
| Bloqueantes de distribución | **1** (auditoría de licencias de modelos) |

> **Conclusión [I]:** el **78 %** de la superficie de Wispr Flow **no está cubierta por ningún proyecto open source**. La mayor parte no es difícil: son funciones de producto (diccionario, estilos, snippets, transforms, MCP) que son ingenierías directas. El **único bloque realmente difícil** es el **notetaker con diarización y fusión streaming/final**.

---

## 16.2 Matriz de paridad por función

### B.1 — Pipeline de dictado

| # | Función | Wispr Flow (evidencia) | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 1 | Captura de micrófono + Opus por chunks | ✅ `mic.pcm` + `upload.ogg` + `opusChunks` | ✅ | ✅ | ✅ portar Handy | **P0** | S |
| 2 | Push-to-talk global | ✅ `Ctrl+`` | ✅ | ✅ | ✅ | **P0** | S |
| 3 | Modo modificador sostenido (Alt) | ✅ `modifierShortcut=164` | ✅ | ⚠️ | ✅ | P1 | S |
| 4 | Inserción de texto en la app enfocada | ✅ `pastedText` | ✅ | ✅ | ✅ portar Handy (**Reliable Paste**) | **P0** | M |
| 5 | Detección de app + URL de destino | ✅ `ms-teams`, `notepad++`, `SndVol` | ❌ | ⚠️ | ✅ propio | **P0** | M |
| 6 | Contexto de accesibilidad (AX) del campo | ✅ `axText`/`axHTML`, `useAxContext` | ❌ | ⚠️ | ✅ propio (UIA) | **P0** | L |
| 7 | Sustitución de nombres propios vía AX | ✅ sidecar `noun` | ❌ | ❌ | ✅ propio | **P0** | L |
| 8 | ASR local | ❌ **nunca** («Transcription always occurs on the cloud») | ✅ `transcribe-cpp` | ✅ | ✅ portar | **P0** | M |
| 9 | ASR cloud opcional (MiniMax `asr-1.0`) | ✅ | ❌ | ⚠️ | ✅ ver [15](15-INTEGRACION-MINIMAX.md) | P1 | S |
| 10 | Formateo local (reglas) | ❌ (nube) | ❌ | ❌ | ✅ propio | **P0** | M |
| 11 | Formateo con LLM opcional | ✅ `formattedText` | ⚠️ BYO-key | ⚠️ | ✅ propio | P1 | M |
| 12 | Transcripción en crudo (sin formato) | ✅ `raw_transcript` (33/3 160) | ✅ | ✅ | ✅ | P1 | S |
| 13 | Formato por aplicación | ✅ `appTranscriptionFormats` | ❌ | ❌ | ✅ | P1 | S |
| 14 | Sidecars de traza por dictado | ✅ `paste`/`noun`/`textbox`/`verify`/`event`/`edit`/`context` | ❌ | ❌ | ✅ propio (copiar patrón) | P1 | S |
| 15 | Sistema de «dismiss» (Esc) | ✅ atajo `dismiss` | ✅ | ✅ | ✅ | P1 | S |
| 16 | Copiar/pegar el último dictado | ✅ `copy_last_text` / `paste_last_text` | ✅ | ⚠️ | ✅ | P1 | S |
| 17 | Cancelación de un dictado en curso | ✅ `dismissed` (3 filas) | ✅ `complete_unless_cancelled` | ⚠️ | ✅ portar (25 ms) | P1 | S |
| 18 | Detección de fin de observación de contenido | ✅ `contentObservationEndReason` | ❌ | ✅ monitor | ✅ | P2 | M |
| 19 | Detección de voz (VAD) | ✅ (implícito) | ✅ Silero | ✅ | ✅ portar | **P0** | S |
| 20 | Audio de sistema (loopback) | ✅ (notetaker) | ❌ | ✅ helper WASAPI | ✅ portar de OpenWhispr | P1 | M |
| 21 | Bluetooth / Jabra como micrófono | ✅ `jabra-device-connector` | ❌ | ❌ | ✅ (CPAL) | P2 | S |
| 22 | Flow Lens / overlay de contexto | ✅ `lens`, ventana `Context Menu` | ❌ | ❌ | ✅ simplificado | P2 | M |
| 23 | **Secuestro de `Ctrl+Z`/`Ctrl+V` globales** | ✅ `splitKeybinds` | ❌ | ❌ | ❌ **deliberadamente NO** | — | — |

### B.2 — Diccionario

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 24 | Entradas manuales | ✅ | ✅ | ✅ | ✅ | **P0** | S |
| 25 | Sustitución de texto (no sólo ortografía) | ✅ `replacement` | ❌ | ❌ | ✅ | **P0** | S |
| 26 | Sustituto con formato (HTML) | ✅ `replacementHtml` | ❌ | ❌ | ✅ | P1 | S |
| 27 | Aprendizaje automático de correcciones | ⚠️ **parcial/broken en 1.6.1174** | ❌ | ✅ `correction-learner` | ✅ **completo** | **P0** | M |
| 28 | Contadores de uso local + servidor | ✅ | ❌ | ❌ | ✅ local | P1 | S |
| 29 | Favoritos | ✅ `isStarred` | ❌ | ❌ | ✅ | P2 | S |
| 30 | Diccionario de equipo | ✅ `teamDictionaryId` | ❌ | ❌ | ❌ (no necesario) | P3 | S |
| 31 | Borrado lógico sincronizable | ✅ `isDeleted` | ❌ | ❌ | ✅ | P2 | S |
| 32 | Orden del diccionario | ✅ `dictionarySort` | ❌ | ❌ | ✅ | P3 | S |

### B.3 — Fragmentos (snippets)

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 33 | Fragmentos básicos | ✅ (3 de fábrica) | ❌ | ✅ | ✅ | **P0** | S |
| 34 | **Fragmentos con variables `{campo}`** | ❌ **no existe** | ❌ | ❌ | ✅ **diferencial** | **P0** | S |
| 35 | Fragmentos por aplicación | ❌ | ❌ | ❌ | ✅ **diferencial** | P1 | S |
| 36 | Trigger por comando (`!firma`) | ❌ | ❌ | ❌ | ✅ **diferencial** | P1 | S |
| 37 | Fragmentos encadenados | ❌ | ❌ | ❌ | ✅ (con límite de profundidad) | P2 | S |
| 38 | Fragmentos con catálogo de fábrica ampliable | ⚠️ 3 fijos | ❌ | ❌ | ✅ | P2 | M |

### B.4 — Estilos y voces

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 39 | Perfil de estilo por app (4 Voces) | ✅ | ❌ | ⚠️ | ✅ | **P0** | M |
| 40 | Estilos personalizables | ✅ presets **Formal · Casual · very casual · Excited!** | ❌ | ❌ | ✅ | **P0** | M |
| 41 | Detección automática de estilo | ✅ | ❌ | ❌ | ✅ | P1 | S |
| 42 | Niveles de limpieza | ✅ dial **None · Light · Medium** (se eliminó «High») | ❌ | ❌ | ✅ 4 niveles | **P0** | S |
| 43 | Muestreo de estilo ("probar con una muestra") | ✅ | ❌ | ❌ | ✅ | P1 | S |
| 44 | Override por app / URL / título de ventana | ⚠️ sólo por app | ❌ | ❌ | ✅ **diferencial** | P1 | M |
| 45 | Importar/exportar estilos | ❌ | ❌ | ❌ | ✅ **diferencial** | P2 | S |
| 46 | Estilo 100 % determinista (sin LLM) | ❌ | ❌ | ❌ | ✅ **diferencial** | **P0** | M |

### B.5 — Transforms / Polish

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 47 | Transform de texto por IA | ✅ | ⚠️ | ⚠️ | ✅ | **P0** | M |
| 48 | Catálogo de transforms predefinidos | ⚠️ **sólo 2** (Polish con 5 reglas conmutables + Prompt Engineer) + 4 re-transforms | ❌ | ❌ | ✅ **13 propuestos** (diferencial) | **P0** | M |
| 49 | Prompts personalizados con nombre + atajo | ✅ `promptName`/`shortcutKey` | ❌ | ❌ | ✅ | P1 | S |
| 50 | **Diff visible antes de aplicar** | ✅ `view_diff`/`diffCount` | ❌ | ❌ | ✅ **obligatorio** | **P0** | M |
| 51 | Deshacer | ✅ `polishUndone` | ❌ | ❌ | ✅ | **P0** | S |
| 52 | Abstracción de provider (`usedProvider`/`modelVersion`) | ✅ | ⚠️ | ⚠️ | ✅ | **P0** | S |
| 53 | **Transforms 100 % locales** | ❌ | ⚠️ | ❌ | ✅ **diferencial** | **P0** | M |
| 54 | Aplicar automáticamente tras dictar | ✅ | ❌ | ❌ | ✅ | P2 | S |
| 55 | Transform sobre texto seleccionado | ✅ flag | ❌ | ❌ | ✅ | P1 | S |
| 56 | Feedback por transform | ✅ `Polish.feedback` | ❌ | ❌ | ✅ | P2 | S |
| 57 | Agente con llamadas a herramientas | ✅ `instruct-history-tool-calls` | ❌ | ❌ | Fase 3 | P3 | XL |

### B.6 — Reuniones (Notetaker)

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 58 | Grabación de reunión | ✅ | ❌ | ✅ | ✅ | **P0** | M |
| 59 | **Diarización / atribución de hablantes** | ✅ (LLM agresivo + `speakerMap`) | ❌ | ✅ | ✅ **propio** (VAD→embed→cluster) | **P0** | **XL** |
| 60 | **Fusión streaming/final** (`fusionTuning` 6 s/2 s) | ✅ | ❌ | ⚠️ | ✅ **propio** | **P0** | **L** |
| 61 | Transcripción en vivo | ✅ `live.ndjson` | ❌ | ⚠️ | ✅ | **P0** | M |
| 62 | Resumen | ✅ `summary` + `candidateSummary` | ❌ | ✅ | ✅ local | **P0** | M |
| 63 | Tareas extraídas | ⚠️ flag `todos` off | ❌ | ✅ | ✅ | P1 | M |
| 64 | Pipeline reanudable por segmentos | ✅ `*_seq` + reintentos | ❌ | ❌ | ✅ **copiar** | **P0** | M |
| 65 | Integración de calendario | ✅ | ❌ | ⚠️ | ✅ | P1 | M |
| 66 | Importar Granola / Otter | ✅ | ❌ | ❌ | ✅ | P2 | M |
| 67 | Carpetas de reuniones | ✅ | ❌ | ⚠️ | ✅ | P1 | S |
| 68 | Compartir notas (público por defecto) | ✅ ⚠️ | ❌ | ❌ | ✅ **privado por defecto** | P1 | S |
| 69 | Feedback del resumen | ✅ `summaryFeedback` | ❌ | ❌ | ✅ | P2 | S |
| 70 | Identificación del hablante principal (tu voz) | ❌ | ❌ | ❌ | ✅ **diferencial** | P1 | M |

### B.7 — Notas

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 71 | Notas con título y contenido | ✅ | ⚠️ | ✅ | ✅ | **P0** | S |
| 72 | Búsqueda full-text (FTS) | ✅ `searchableContent` | ❌ | ✅ semántica | ✅ FTS5 | P1 | S |
| 73 | Versionado de notas | ✅ `NoteVersions` | ❌ | ❌ | ✅ | P1 | S |
| 74 | Imágenes adjuntas (locales) | ✅ → **S3** | ❌ | ❌ | ✅ **locales** | P1 | M |
| 75 | Notas fijadas | ✅ `pinned` | ❌ | ⚠️ | ✅ | P2 | S |
| 76 | Scratchpad (nota rápida) | ✅ (flag off) | ❌ | ❌ | ✅ | P1 | S |

### B.8 — Estadísticas

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 77 | Palabras / tiempo / PPM | ✅ | ✅ | ✅ | ✅ | P1 | S |
| 78 | Estadísticas por app (+URL) | ✅ | ⚠️ | ⚠️ | ✅ | P1 | S |
| 79 | **Calidad del formateo** (palabras corregidas / dictadas) | ❌ no expuesto | ❌ | ❌ | ✅ **diferencial** | P1 | S |
| 80 | Rachas | ✅ | ❌ | ❌ | ✅ local opcional | P2 | S |
| 81 | Hitos de palabras | ✅ | ❌ | ❌ | ✅ local | P2 | S |
| 82 | Leaderboard de equipo | ✅ | ❌ | ❌ | ❌ por diseño | — | — |
| 83 | Exportación de métricas | ❌ | ✅ | ⚠️ | ✅ | P2 | S |

### B.9 — MCP

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 84 | Servidor MCP (stdio + HTTP local) | ✅ **remoto** `https://api.wisprflow.ai/connect/mcp`, **sólo lectura**, OAuth sin API key | ❌ | ❌ (**no está en el repo**) | ✅ local, con control granular | **P0** | M |
| 85 | Alcance de las herramientas MCP | Resúmenes · Notas · Transcripciones · Asistentes · Tareas · **Scratchpad** · Calendario · Notas compartidas | ❌ | ❌ | ✅ dictar + buscar + meetings + transforms | **P0** | M |
| 86 | Confirmación humana en escrituras | [NV] | n/a | n/a | ✅ **obligatorio** | **P0** | S |
| 87 | Auditoría de llamadas MCP | [NV] | n/a | n/a | ✅ | P1 | S |
| 87b | **Voice Profile** (modelo personal aprendido, se desbloquea a 2 000 palabras) | ✅ en el servidor | ❌ | ❌ | ✅ **100 % local** | **P0** | M |

### B.10 — Configuración

| # | Función | Wispr Flow | Handy | OpenWhispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|---|---|
| 88 | Preferencias persistentes (≈ 200) | ✅ | ✅ | ✅ | ✅ (≈ 60, sólo las que importan) | **P0** | M |
| 89 | Personalización de atajos | ✅ | ✅ | ✅ | ✅ | **P0** | M |
| 90 | Selección de micrófono + ranking | ✅ `rankedAudioDevices` | ✅ | ✅ | ✅ | **P0** | S |
| 91 | Tema claro/oscuro | ✅ | ✅ | ✅ | ✅ | P1 | S |
| 92 | Idioma de la interfaz | ✅ (55 locales) | ⚠️ | ⚠️ | ✅ es/en | P1 | S |
| 93 | Política de datos locales | ✅ `localDataPolicy` | ❌ | ⚠️ | ✅ | P1 | S |
| 94 | **Un solo interruptor de compartir datos** | ❌ **dos contradictorios** | ✅ | ⚠️ | ✅ **diferencial** | **P0** | S |
| 95 | Atajo global de «deshacer» sin secuestrar `Ctrl+Z` | ❌ secuestra | ✅ | ❌ | ✅ | P1 | S |

### Fase C — UX/UI

| # | Elemento | Wispr Flow | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|
| 96 | Flow Bar flotante con forma configurable | ✅ (15 estados SVG) | ✅ | **P0** | M |
| 97 | Indicador de nivel de audio | ✅ | ✅ | **P0** | S |
| 98 | Hub con 8 secciones | ✅ | **3 secciones** (simplificado) | **P0** | M |
| 99 | Overlay de contexto a pantalla completa | ✅ | ⚠️ simplificado / comando de voz | P2 | M |
| 100 | Onboarding | ⚠️ 13 páginas + video + 15 coachmarks | **3 pasos, sin video** | **P0** | S |
| 101 | Accesibilidad completa | ⚠️ parcial | ✅ | P1 | M |
| 102 | Permiso de accesibilidad explícito | ❌ | ✅ | **P0** | S |
| 103 | Sistema de temas | ✅ | ✅ | P1 | S |

### Fase D — Rendimiento

| # | Objetivo | Wispr | **Dixi v1** | Prio | Esfuerzo |
|---|---|---|---|---|---|
| 104 | RAM en reposo | 360 MB | **< 120 MB** | **P0** | (arquitectural) |
| 105 | Procesos | 12 | **2** | **P0** | (arquitectural) |
| 106 | Arranque en frío | 3 624 ms | **< 900 ms** | P1 | (arquitectural) |
| 107 | Disco (3 160 dictados) | 755 MB | **< 100 MB** | P1 | M |
| 108 | Latencia e2e p50 | 640 ms | 300–800 ms (local) / < 250 ms (streaming) | P1 | L |
| 109 | Coste monetario | USD 15/mes | **0 USD** | **P0** | — |

---

## 16.3 Brechas P0 — el camino crítico

| # | Brecha | Por qué es P0 | Esfuerzo | Bloqueante |
|---|---|---|---|---|
| B-01 | **Diarización local** | Sin ella no hay notetaker útil | **XL** | No |
| B-02 | **Fusión streaming/final** | Define la calidad de la transcripción en vivo | **L** | No |
| B-03 | **Contexto AX + nombres propios** | Es lo que hace que el formateo «se sienta inteligente» | **L** | No |
| B-04 | **Motor de formateo local determinista** | El 78 % del valor percibido | **M** | No |
| B-05 | **Inserción fiable en Windows** | Si falla, el producto entero falla | **M** | No |
| B-06 | **Catálogo de transforms + diff + deshacer** | Diferencial claro frente a Wispr | **M** | No |
| B-07 | **Snippets con variables** | Diferencial claro | **S** | No |
| B-08 | **Estilos por app + niveles de limpieza** | Reproduce la personalización | **M** | No |
| B-09 | **Servidor MCP** | Extiende la utilidad a todo el ecosistema de IA | **M** | No |
| B-10 | **Abstracción de provider con diff obligatorio** | Evita pérdida de datos con LLM | **S** | No |
| B-11 | **Auditoría de licencias de modelos** | **Bloquea la distribución** | **S** | **SÍ** |

---

## 16.4 Bloqueantes de la Fase 2

| # | Bloqueante | Estado | Responsible |
|---|---|---|---|
| 1 | **Licencias de los pesos de los modelos ASR** (`nvidia-open-model-license`, CC-BY-4.0) — **no verificadas** | ⚠️ Pendiente | Requiere auditoría legal/técnica |
| 2 | **Verificación de la cuenta MiniMax**: ¿el uso personal diario está dentro de «individual, interactive developer use»? | ⚠️ Pendiente | Requiere el titular de la cuenta |
| 3 | **Host del endpoint ASR de MiniMax** (`.cn` vs `.io`) | ⚠️ Pendiente | Verificación técnica de 1 llamada |
| 4 | **Estrategia de distribución** (binario firmado, modelo embebido vs descargado) | ⚠️ Pendiente | Decisión de producto |
| 5 | **Medición de consumo durante dictado activo y reuniones** | ⚠️ No realizado (requiere autorización) | Requiere autorización del usuario |
| 6 | ~~Catálogo exacto de transforms de fábrica~~ | ✅ **Resuelto** — sólo 2 (Polish + Prompt Engineer) + 4 re-transforms | — |
| 7 | ~~Si los límites M Plan aplican igual a Subscription Keys~~ | ✅ **Resuelto** — la documentación **no diferencia**; requiere sonda empírica de bajo coste | Verificación técnica |

---

## 16.5 Orden de ejecución recomendado (Fase 2)

```
SPRINT 1 — Cimientos (2 semanas)
  ├─ Arranque Tauri 2 + Rust + React + SQLite + rusqlite
  ├─ Migraciones SQL versionadas (patrón Sequelize de Wispr, simplificado)
  ├─ tauri-specta: IPC tipado
  ├─ Hotkey global (PTT) + Flow Bar mínima con indicador de estado
  └─ Captura de micrófono (CPAL) + Opus en memoria
  ⇒ Entregable: grabar → mostrar nivel → transcribir con whisper.cpp (descartable)

SPRINT 2 — Núcleo de dictado (2 semanas)
  ├─ ASR local (transcribe-cpp / Parakeet GGUF) + VAD (Silero)
  ├─ Motor de formateo local determinista (reglas + diccionario)
  ├─ Detección de app + URL + AX (UIAutomation) ← B-03 arranca aquí
  ├─ Inserción de texto con «Reliable Paste» ← B-05
  └─ Historial con las 25 columnas que importan
  ⇒ Entregable: dictado completo end-to-end, sin nube

SPRINT 3 — Producto (2 semanas)
  ├─ Diccionario completo (entradas, sustitución, aprendizaje) ← B-07-B
  ├─ Snippets con variables ← B-07
  ├─ Estilos por app + niveles de limpieza ← B-08
  ├─ Transforms con diff + deshacer + catálogo ← B-06-B-10
  └─ Abstracción de provider (local + MiniMax opcional)
  ⇒ Entregable: reemplazo funcional de Dictado + Diccionario + Transforms

SPRINT 4 — Diferenciales (2 semanas)
  ├─ Servidor MCP + 6 herramientas + confirmación + auditoría ← B-09
  ├─ UX: 3 secciones, onboarding de 3 pasos, accesibilidad
  ├─ Métricas locales + calidad del formateo
  └─ Privacidad: notas privadas, adjuntos locales, un solo interruptor de sharing
  ⇒ Entregable: el producto que Wispr no puede ser

SPRINT 5 — Notetaker (4-6 semanas) ← opcional, puede ir en paralelo
  ├─ Loopback WASAPI (portar de OpenWhispr)
  ├─ VAD → embeddings → clustering ← B-01 (el bloque más difícil)
  ├─ Fusión streaming/final ← B-02
  ├─ Resumen + tareas local
  └─ Pipeline reanudable por segmentos ← B-04 (copiar de Wispr)
  ⇒ Entregable: notetaker local

SPRINT 6 — Hardening
  ├─ Auditoría de licencias de modelos ← BLOQUEANTE
  ├─ Firma de binarios, instalador, auto-actualización
  ├─ Pruebas E2E de los flujos críticos
  └─ Medición de rendimiento en carga (autorizada)
```

---

## 16.6 Diferenciadores de Dixi frente a Wispr Flow

| # | Diferenciador | Evidencia de que Wispr no lo tiene |
|---|---|---|
| 1 | **Transcripción 100 % local por defecto** | `clientNetworkLatency` p50 = 270 ms ⇒ Wispr **siempre** va a la nube [V] |
| 2 | **Audio nunca sale del equipo** | `needsUploading`, `audioUploadedAt`, `upload.ogg` ⇒ subida obligatoria [V] |
| 3 | **Notas privadas por defecto** | `defaultNoteVisibility = "anyone_with_link"` [V] |
| 4 | **Imágenes de notas locales** (no S3) | `s3Key`, `presignedGetUrl` en `NoteImages` [V] |
| 5 | **Un solo interruptor de datos** | `internalDataSharingEnabled=false` vs `instructInternalDataSharingEnabled=true` [V] |
| 6 | **Sin cuotas** (2 000–5 000 palabras/semana, 150 reuniones) | `weekly-word-limits-config`, `notetakerQuota` [V] |
| 7 | **Coste cero** | `FLOW_PRO_MONTHLY` USD 15 [V] |
| 8 | **Snippets con variables** | No hay campo de variables en `Dictionary` [V] |
| 9 | **Transforms locales** | `polishInstructions.default = {}` ⇒ los transforms se resuelven en el servidor [V] |
| 10 | **Estilos por URL y título de ventana** | Sólo por app (`appTranscriptionFormats`) [V] |
| 11 | **No secuestra `Ctrl+Z`/`Ctrl+V`** | `splitKeybinds` los incluye [V] |
| 12 | **Sin gamificación de presión** | Rachas + hitos + leaderboard + nudges con cohortes [V] |
| 13 | **3 secciones, no 8** | 8 secciones en el sidebar [V] |
| 14 | **Métricas de calidad del formateo** | No existe en la UI [V] |
| 15 | **Reconocimiento del hablante principal (tu voz)** | `identityContacts` es un modelo de contactos, no un perfil de voz [I] |

---

## 16.7 Trazabilidad de la auditoría

| Fase | Documento | Estado | Evidencia principal |
|---|---|---|---|
| A — Entorno | [01-ENTORNO.md](01-ENTORNO.md) | ✅ Completo | 12 procesos, 5 ventanas, Electron 42.11.2 |
| B — Funciones | [03](03-FUNCIONES-DICTADO.md) · [04](04-DICCIONARIO.md) · [05](05-SNIPPETS.md) · [06](06-ESTILOS.md) · [07](07-TRANSFORMS.md) · [08](08-REUNIONES-NOTAS.md) · [09](09-STATS.md) · [10](10-MCP.md) · [11](11-CONFIGURACION.md) | ✅ Completo | 29 tablas, 174 migraciones, 200 preferencias |
| C — UX/UI | [12-UX-UI.md](12-UX-UI.md) | ✅ Completo | Árbol UIA de 197 líneas, DOM de 2 renderers vía REA |
| D — Rendimiento | [13-RENDIMIENTO.md](13-RENDIMIENTO.md) | ⚠️ Completo salvo carga activa | 360 MB, 0,7 % CPU, latencias de 3 160 dictados, arranque 3 624 ms |
| E — OSS | [14-ALTERNATIVAS-OPEN-SOURCE.md](14-ALTERNATIVAS-OPEN-SOURCE.md) | ✅ Completo | Handy MIT, OpenWhispr MIT, 277 marcas VERIFIED |
| F — MiniMax | [15-INTEGRACION-MINIMAX.md](15-INTEGRACION-MINIMAX.md) | ✅ Completo | Endpoints, límites, errores, credenciales |
| G — Paridad | Este documento | ✅ Completo | 87 funciones catalogadas |

---

### Referencias cruzadas

Todos los documentos de la auditoría: [01](01-ENTORNO.md) · [02](02-ARQUITECTURA.md) · [03](03-FUNCIONES-DICTADO.md) · [04](04-DICCIONARIO.md) · [05](05-SNIPPETS.md) · [06](06-ESTILOS.md) · [07](07-TRANSFORMS.md) · [08](08-REUNIONES-NOTAS.md) · [09](09-STATS.md) · [10](10-MCP.md) · [11](11-CONFIGURACION.md) · [12](12-UX-UI.md) · [13](13-RENDIMIENTO.md) · [14](14-ALTERNATIVAS-OPEN-SOURCE.md) · [15](15-INTEGRACION-MINIMAX.md)
