# 08 — Reuniones (Notetaker) y Bloc de notas

> Fuente: tablas `Meetings` (60 columnas), `MeetingVersions`, `Notes`, `NoteVersions`, `NoteImages`, `NotetakerChats`/`Messages`/`AgentStates`, `Folders`, `Todos`, `CalendarEvents`, `Links`; payload del flag `meeting-recorder`; estructura de `meetings/<uuid>/` en disco; preferencias `prefs.user.notetaker*`.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 8.1 El Notetaker: un producto dentro del producto

Wispr Flow 1.6.1174 contiene **dos productos** que comparten base de datos y runtime:

| | **Flow** (dictado) | **Notetaker** (reuniones) |
|---|---|---|
| Tabla principal | `History` (3 160 filas) | `Meetings` (5 filas) |
| Sección del hub | «Dictado» | «Tomador de notas» |
| Atajo | `Ctrl+`` | `Alt+M+`` |
| Ventana | — | «Meeting Recorder» 634×1112 |
| Cuota | palabras/semana | 150 reuniones/semana |

La lista de migraciones muestra la **divergencia**: las 174 migraciones cubren 2024-05 → 2026-10, con el notetaker irrumpiendo en 2026-04 (`create-meetings-table`).

---

## 8.2 La tabla `Meetings`: 60 columnas que cuentan una historia

### 8.2.1 Ciclo de vida de la reunión

| Columna | Rol |
|---|---|
| `id` | UUID |
| `title` | Título |
| `createdAt` / `modifiedAt` / `endedAt` | Temporalización (epoch ms) |
| `recordedMs` | Duración grabada (obs.: 349 000 · 1 467 990 · 2 595 000 ms) [V] |
| `finalized` | ¿Reunión cerrada? (5/5 = true) [V] |
| `refineStatus` | Estado del refinamiento: `complete` en 5/5 [V] |
| `synced` / `isDeleted` | Estado de sincronización (borrado lógico) |
| `liveTranscriptPendingPush` / `liveTranscriptUploadedAt` / `liveTranscriptRetries` | **Push incremental de la transcripción en vivo** con reintentos |
| `latestRecordingStopSeq` / `refinePipelineRanThroughStopSeq` | **Secuencias de stop** ⇒ el pipeline de refinamiento es **incremental y reanudable** |

> **Hallazgo de diseño muy importante [V]:** `latestRecordingStopSeq` + `refinePipelineRanThroughStopSeq` + `*Retries` + `*PendingPush` ⇒ Wispr implementa un **pipeline de refinamiento reanudable por segmentos**: si falla el refinamiento, reintenta sólo lo pendiente; si falla el push, reintenta sólo ese push. La reunión sigue siendo utilizable aunque el servidor esté caído. **Este es el patrón exacto que Dixi necesita para un refinamiento local, y es la mejor idea de ingeniería que se puede copiar de Wispr.**

### 8.2.2 Contenido y Derivatives

| Columna | Rol |
|---|---|
| `notes` | Notas de la reunión (texto) |
| `summary` | Resumen generado |
| `candidateSummary` / `candidateSummaryMetadata` | **Resumen candidato** (aún no confirmado) ⇒Wispr genera un resumen y lo **sugiere** antes de fijarlo |
| `speakerMap` / `speakerMapPendingPush` / `speakerMapDraftTranscriptAt` | Mapa hablante ↔ identidad |
| `speakerAssignments` | Asignaciones de hablante editadas por el usuario |
| `speakerAssignmentEdits` / `speakerRoomLabelEdits` | Ediciones de asignación y de etiqueta de sala |
| `speakerArtifactUploadedAt` / `speakerArtifactRetries` | Artefacto de hablante (¿embedding? ¿perfil?) subido al servidor |
| `participantNames` | Nombres de participantes |
| `identityContacts` | **Contactos**.Del usuario usados para resolver identidades |
| `avatarPick` | Avatar elegido |
| `orgInsights` / `orgInsightsPendingPush` | Insights organizacionales (flag `org-insights` **desactivado**) |
| `summaryFeedback` | Feedback sobre el resumen |
| `summaryReady` / `transcriptReady` | Semáforos de disponibilidad |
| `summaryReadyEventId` / `notifiedSummaryReadyEventId` | **Notificación de evento** (idempotencia) |

### 8.2.3 Compartir y calendario

| Columna | Rol |
|---|---|
| `shareSlug` / `shareVisibility` | Compartir por enlace (default de nota: `anyone_with_link`) |
| `calendarEventExternalId` | Evento de calendario asociado |
| `calendarOccurrenceStartAtUtc` | Inicio de la **ocurrencia** (soporta eventos recurrentes) |
| `importSource` | Origen de importación (`granola`, `otter`, …) — flag `granola-import` y `otter-import` **activos** |
| `isTourDemo` | Marcador de reunión de demo del tour |
| `currentJobId` / `pendingJobRequest` | **Cola de trabajos** del pipeline |
| `refineRetries` / `encodeRetries` / `refinedFetchRetries` | Reintentos por etapa |
| `refineUploadFailureReason` | Motivo de fallo (diagnóstico) |

---

## 8.3 El pipeline de refinamiento (reconstrucción)

```
[CAPTURA]  mic.pcm (crudo) ──┐
            system audio ─────┤→ mix → Opus chunks → upload.ogg
                                │
[EN VIVO]  live.ndjson  ← transcripción en vivo (NDJSON, 1 línea por segmento)
           speakers.observations.ndjson ← observaciones de hablante
           audio-devices.json ← dispositivos de audio usados
           native-language-evidence.json ← evidencia de idioma nativo
           .mic-retained / .encode-cohort.json ← flags internos

[SERVIDOR]  refineStatus='complete'  →  serverRefinedUploadedAt
             refinedFetchedThroughAt  →  cursor de descarga
             →  summary / notes / speakerMap  (con push pendiente y reintentos)
```

**Ficheros reales observados en `meetings/<uuid>/`:** [V]

| Fichero | Tamaño (ejemplo) | Contenido |
|---|---|---|
| `mic.pcm` | 45,6 MB | **PCM crudo del micrófono** |
| `upload.ogg` | 11,5 MB | Versión Opus para subida (≈ 4:1 de compresión) |
| `live.ndjson` | — | Transcripción en vivo (newline-delimited JSON) |
| `refined.ndjson` | — | Transcripción refinada |
| `speakers.observations.ndjson` | — | Observaciones de hablante |
| `audio-devices.json` | — | Dispositivos de audio |
| `native-language-evidence.json` | — | Evidencia de idioma |
| `.observations-tmp/` | — | Observaciones en curso |

> **Dato de diseño [V]:** `mic.pcm` + `upload.ogg` ⇒ Wispr guarda **dos representaciones** del audio: el crudo para reprocesar y la comprimida para subir. Es la causa directa del consumo de disco. Con `localAudioRetentionDays = 14`, el PCM se purga a los 14 días.

---

## 8.4 El algoritmo de fusión (del payload de `meeting-recorder`)

```json
"fusionTuning": {
  "nativeHealWindowMs": 6000,
  "ultraHighWindowMs": 2000,
  "proofGatedUltraHolders": ["zoom", "teams"],
  "clickProofHolders": ["zoom", "teams"]
},
"browserLeaveClick": "enabled",
"nativeAnchorShadow": true,
"confRoomWalkbackReachSeconds": 60,
"speakerLlmAttribution": "full-aggressive"
```

**Reconstrucción [I]:**

1. La transcripción en vivo (streaming) y la transcripción final refinada **se reconcilian** con una **ventana de sanación de 6 s** (`nativeHealWindowMs`): cuando llega el resultado final, se replacesegmentos de la ventana reciente.
2. Existe un modo **«ultra-high»** con ventana de **2 s** (`ultraHighWindowMs`) que se aplica **sólo a las reuniones detectadas por evidencia** (`proofGatedUltraHolders`: Zoom y Teams).
3. `clickProofHolders` ⇒ la «prueba» que habilita el modo ultra-high es **un clic detectado** en la ventana de la reunión (`browserLeaveClick: enabled` ⇒ el clic dentro del navegador cuenta como prueba).
4. `walkbackReachSeconds: 60` ⇒ se puede **retroceder 60 s** para corregir.
5. `speakerLlmAttribution: full-aggressive` ⇒ la **atribución de hablantes la resuelve el LLM** de forma agresiva.
6. `nativeAnchorShadow` ⇒ hay un **anclaje nativo** (posiblemente un modelo local de anclaje) que hace de sombra/anchor para estabilizar la atribución.

> **Ésta es la pieza más difícil de todo el producto y no está cubierta por ningún proyecto open source evaluado.** Es también la que más valor aporta: la atribución de hablantes automática es lo que hace útil un notetaker. Ver [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) §3.

---

## 8.5 Integraciones del notetaker

| Integración | Evidencia | Estado |
|---|---|---|
| **Calendario** | `CalendarEvents` (tabla), `notetakerCalendarNotificationTiming = "15_seconds_before"`, `notetaker-notetaker-controls`, `add-calendar-event-notetaker-controls`, `add-calendar-event-series-key` | Activa [V] |
| **Granola** | `GranolaImportRun`, `GranolaTranscriptQueue` (índices: `state`, `granolaAccountHash`, `granolaDocumentId`), flag `granola-import` | Activa [V] |
| **Otter** | flag `otter-import` | Activa [V] |
| **Chat con el notetaker** | `NotetakerChats`, `NotetakerChatMessages` (`role`, `status`, `content` JSON, `serverSequence`, `localOrder`), `NotetakerChatAgentStates` (`trajectory` JSON, `revision`) | **Es un agente con estado** [V] |
| **Notas compartidas** | `SharedNotes` (tabla), `NotetakerChats.shareNote` | [V] |
| **Carpetas** | `Folders`, `FolderMeetings`, `add-folders-member-count`, `add-folders-sharing-blocked` | Activa [V] |
| **Tareas** | `Todos` (tabla), `add-todos-table` | Flag `todos` **desactivado** [V] |
| **MCP** | `notetakerMCPVisibilityState = "ModalDismissed"`, botón «Configurar MCP» | Ver [10-MCP.md](10-MCP.md) |

### 8.5.1 El chat con el notetaker es un agente con estado

`NotetakerChatAgentStates` tiene `trajectory` (JSON), `displaySequenceIndex` y **`revision`** (control de concurrencia optimista) ⇒ el estado del agente se versiona y las mutaciones se resuelven por revisión. `NotetakerChatMessages` tiene `serverSequence` **y** `localOrder` con dos índices únicos ⇒ **reconciliación servidor/local** de mensajes. [V]

> **Diseño a copiar:** `localOrder` + `serverSequence` con UNIQUE evita duplicados al reconectar y permite ordenar de forma determinista. Es un patrón de **CRDT simplificado** que Dixi puede aplicar igual.

---

## 8.6 El Bloc de notas (`Notes`)

```sql
CREATE TABLE Notes (
  id                VARCHAR(36) PRIMARY KEY,
  title             VARCHAR(255) NOT NULL,
  contentPreview    TEXT NOT NULL,     -- extracto para listados
  content           TEXT NOT NULL,     -- cuerpo completo
  searchableContent TEXT,              -- índice FTS (migración add-notes-searchable-content)
  createdAt / modifiedAt DATETIME NOT NULL,
  synced            TINYINT(1) NOT NULL DEFAULT 1,
  isDeleted         TINYINT(1) NOT NULL DEFAULT 0,
  finalized         TINYINT(1) NOT NULL DEFAULT 0,
  pinned            TINYINT(1) NOT NULL DEFAULT 0
);
```

| Característica | Evidencia |
|---|---|
| **Búsqueda full-text** | `searchableContent` + migración `add-notes-searchable-content` (2026-04) ⇒ FTS5 [V] |
| **Versionado** | tabla `NoteVersions` (`noteId`, `content`, `source`, `transformId`, `transformPrompt`) ⇒ cada nota tiene **historial de versiones con el transform que la generó** [V] |
| **Imágenes** | tabla `NoteImages` con `data` BLOB, `width`, `height`, `sizeBytes`, `contentType`, **`s3Key`**, `uploadedToS3`, `presignedGetUrl`, `urlExpiresAt` ⇒ **las imágenes se suben a S3 con URL prefirmada y expiración** [V] |
| **Fijadas** | `pinned` [V] |
| **Visibilidad por defecto** | `prefs.user.defaultNoteVisibility = "anyone_with_link"` ⇒ **enlace público por defecto** ⚠️ [V] |

> **⚠️ Observación de privacidad [V]:** las notas se comparten con `anyone_with_link` **por defecto**, y las imágenes se suben a **S3** (almacenamiento externo, presumiblemente AWS, del proveedor). Para Dixi, el valor por defecto debería ser **privado** y el almacenamiento de adjuntos **local** con sharing opcional y explícito. Es una diferencia de privacidad defendible en marketing.

---

## 8.7 Carpetas y organización

| Tabla / migración | Función |
|---|---|
| `Folders` + `FolderMeetings` | Carpetas de reuniones (migración 2026-09-09) |
| `add-folders-member-count` | Nº de miembros (compartidas) |
| `add-folders-sharing-blocked` | Bloqueo de compartir |
| `add-meeting-identity-fields` | Identidad de la reunión |
| `add-meetings-candidate-summary` | Resumen candidato |
| `add-meetings-org-insights` | Insights organizacionales |
| `add-meetings-summary-feedback` | Feedback del resumen |
| `add-meetings-share-columns` | Columnas de compartir |
| `add-meetings-recorded-ms` | Duración grabada |
| `add-meetings-ended-at` | Fin de reunión |
| `add-meetings-participant-names` | Nombres de participantes |
| `add-meetings-speaker-artifact-columns` | Artefacto de hablante |

---

## 8.8 El Notetaker en el hub (UI)

Del árbol de accesibilidad de la sección «Tomador de notas»:

- **Checklist «Configurar Notetaker»** con 4 pasos y su estado:
  1. **«Probar Notetaker»** — estado **Listo** ✓
  2. **«Conectar calendario»**
  3. **«Importar desde Granola u Otter»**
  4. **«Configurar MCP»**
- Es un **onboarding guiado** con checklist, no un formulario.

---

## 8.9 Cuotas y límites del notetaker

| Límite | Valor | Fuente |
|---|---|---|
| Reuniones por semana | **150** (instalación auditada, plan Pro) | `prefs.user.notetakerQuota` [V] |
| **Reuniones por semana — plan Basic (público)** | **50** (una reunión de ≥ 5 min cuenta; **las importaciones no cuentan**; descartar la reunión **devuelve** la cuota) | documentación pública [V] |
| Reuniones por semana — Pro / Growth | «límite mayor» / «ilimitado (add-on)» | documentación pública [V] |
| **Idiomas del notetaker** | **21** (frente a **100+** del dictado) | documentación pública [V] |
| **Plataformas del notetaker** | **macOS + Windows** (no iOS/Android) | documentación pública [V] |
| **Almacenamiento del notetaker** | **sin opción de no enviar** (a diferencia del historial, que sí puede ser local-only) | documentación pública [V] |
| Duración máxima de grabación | **2 h** | `notetakerMaxRecordingLength = "2hr"` [V] |
| Retención de audio local | **14 días** | `localAudioRetentionDays = 14` [V] |
| Retención de transcripción | **nunca** | `notetakerTranscriptRetention = "never_delete"` [V] |
| Inicio automático | activo tras **3 reuniones** | `autoStartPromptThreshold = 3` [V] |
| Aviso de calendario | **15 s antes** | `notetakerCalendarNotificationTiming` [V] |
| Notepad al iniciar | **abrir** | `notetakerNotepadOnStart = "open"` [V] |
| Atributos de hablante | activo | `notetakerAttributeNamesEnabled = true` [V] |
| Detección auto de nombres de participante | activo | `notetakerAutoDetectParticipantNamesEnabled = true` [V] |
| Transcripción en vivo | activo | `notetakerLiveTranscriptEnabled = true` [V] |
| Split screen (Zoom/Teams) | **inactivo** | `notetakerSplitScreenJoinEnabled = false` [V] |
| Ocultar grabador en screen share | **activo** | `hideMeetingRecorderInScreenShare = true` [V] |
| Detección automática de reuniones | **activo** | `autoDetectMeetingsEnabled = true` [V] |
| Áudio del sistema en capturas | activo (flag `notetaker-system-audio-probe`) | [V] |

---

## 8.10 Diseño propuesto para Dixi

### 8.10.1 Pipeline local-first con la misma robustez

```rust
pub enum RefineStage { Transcribe, Diarize, Align, Summarize, Publish }

pub struct Segment {
    pub seq: u64,              // secuencia monotónica (latestRecordingStopSeq)
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Option<SpeakerId>,
    pub confidence: f32,
}
```

**El patrón clave a copiar** (de `latestRecordingStopSeq` / `refinePipelineRanThroughStopSeq`):

```sql
CREATE TABLE dixi_meetings (
  id                    TEXT PRIMARY KEY,
  title                 TEXT NOT NULL,
  started_at            TEXT NOT NULL,
  ended_at              TEXT,
  recorded_ms           INTEGER,
  status                TEXT NOT NULL,      -- recording|transcribing|ready|failed
  latest_segment_seq    INTEGER NOT NULL DEFAULT 0,  -- último segmento capturado
  refine_ran_through_seq INTEGER NOT NULL DEFAULT 0,  -- hasta dónde se refinó
  refine_retries        INTEGER NOT NULL DEFAULT 0,
  upload_retries        INTEGER NOT NULL DEFAULT 0,
  ...
);
```

Con esto, un fallo a mitad de refining **no pierde nada**: se reintenta desde `refine_ran_through_seq`.

### 8.10.2 Diarización

**El problema central de un notetaker local.** Opciones para Dixi:

| Opción | Licencia | Viabilidad local | Nota |
|---|---|---|---|
| **pyannote/segmentation-3.0 + wespeaker-v2** | MIT (modelos CC-BY-4.0) | ⚠️ Python | Precisión de referencia, pero requiere Python |
| **Silero VAD** (Handy ya lo usa) | MIT | ✅ RustONNX | **Segmentación de voz**, no diarización |
| **NVIDIA NeMo Sortformer / diarizer** | Apache-2.0 / CC-BY-4.0 | ⚠️ GPU | Pesado |
| **Embedding local + agglomerative clustering** | propio | ✅ | Sobre `wespeaker` ONNX exportado |
| **LLM para atribuir** (como hace Wispr con `full-aggressive`) | — | ⚠️ | Costoso, pero es lo que hace Wispr |

**Estrategia pragmática para Dixi (v1):**
1. **VAD** con Silero (Rust/ONNX) para segmentar habla.
2. **Embeddings de voz** con `wespeaker-v2` exportado a ONNX (CPU, ~20 ms/segmento).
3. **Clustering aglomerativo** con umbral coseno.
4. **LLM opcional** para reetiquetar los clusters con nombres («el que habla de los budgets es el CFO») usando el `UserContext` del usuario.
5. **Perfil de voz por usuario** (el usuario registra «mi voz» una vez) ⇒ atribución inmediata del hablante principal.

> Esta cadena (VAD → embedding → clustering → etiquetado LLM) es **el mayor bloque de trabajo propio** de Dixi y el que define si el notetaker es viable localmente.

### 8.10.3 Almacenamiento de audio

| Decisión | Wispr | **Dixi (propuesta)** |
|---|---|---|
| Guardar PCM crudo | Sí (45 MB por reunión) | **No** — sólo Opus/Vorbis 16 kHz mono (~1 MB/hora) |
| Guardar para reprocesar | Sí | Sí, pero **Opus** (el Whisper/Parakeet procesa Opus/PCM decodificado) |
| Subir a la nube | Sí | **No por defecto** |
| Retención | 14 días audio / ∞ transcripción | Configurable, por defecto **∞ transcripción + 7 días audio** |

### 8.10.4 Privacidad (diferencia defendible)

| Aspecto | Wispr | **Dixi** |
|---|---|---|
| Visibilidad por defecto de notas | `anyone_with_link` | **privada** |
| Imágenes de notas | S3 + URL prefirmada | **locales** (opcional: shared folder propio) |
| Audio de reuniones | sube a la nube | **nunca sale del equipo** por defecto |
| Datos de identidad (`identityContacts`) | enviados | locales |

---

## 8.11 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| Grabación de reunión | ✅ | ❌ | ✅ | ✅ |
| **Audio del sistema (loopback)** | ✅ (`notetaker-system-audio-probe`) | ❌ | ✅ (helper WASAPI process-loopback) | ✅ |
| Transcripción en vivo | ✅ (`live.ndjson`) | ❌ | ⚠️ | ✅ |
| Diarización / hablantes | ✅ (LLM agresivo + `speakerMap`) | ❌ | ✅ | ✅ (local, ver §8.10.2) |
| Resumen | ✅ | ❌ | ✅ | ✅ local |
| Tareas extraídas | ⚠️ flag `todos` desactivado | ❌ | ✅ | ✅ |
| Carpetas | ✅ | ❌ | ⚠️ | ✅ |
| Importar Granola/Otter | ✅ | ❌ | ❌ | ✅ (importadores) |
| Calendario | ✅ | ❌ | ⚠️ | ✅ |
| Pipeline reanudable | ✅ (`*_seq` + reintentos) | ❌ | ❌ | ✅ **a copiar** |
| Chat con el notetaker (agente) | ✅ (`trajectory`, `revision`) | ❌ | ❌ | Fase 3 |
| Compartir por enlace | ✅ (público por defecto) | ❌ | ❌ | ✅ (privado por defecto) |
| Huella de audio local | ❌ (nube) | ❌ | ❌ | ✅ **diferencial** |

---

### Referencias cruzadas

- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — §2.7.1 payload de `meeting-recorder`
- [10-MCP.md](10-MCP.md) — «Configurar MCP»
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — preferencias del notetaker
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — coste de audio
- [14-ALTERNATIVAS-OPEN-SOURCE.md](14-ALTERNATIVAS-OPEN-SOURCE.md) — helper de loopback de OpenWhispr
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
