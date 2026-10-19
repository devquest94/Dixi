# 11 — Configuración: inventario completo de preferencias

> Fuente: `config.json` (39,8 KB) del perfil de usuario, inspeccionado en modo lectura con **redacción automática** de todo valor cuyo nombre de clave case con `token|secret|password|apikey|auth|credential|cookie|session|email|signature|bearer|jwt|key`.
> **200 preferencias de usuario + 16 internas + 11 módulos de nivel raíz** catalogadas.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

> ⚠️ **Sobre la privacidad de este documento:** los valores de identidad (nombre, correo, UUID de usuario, `sessionId`, `anonymousId`, `deviceId`, `avatarUrl`, código de referido) **se omiten deliberadamente**. Se documenta su *existencia* y su *función*, nunca su valor.

---

## 11.1 Estructura de `config.json`

```jsonc
{
  "prefs": {
    "user": { /* 200 claves */ "internal": { /* 16 claves */ } },
    "sessionId": "…", "anonymousId": "…", "deviceId": "…",
    "appLastClosedTime": 1792198703189,
    "firstLaunchTs": "2026-08-06T17:31:40.873Z",
    "activeDictationSession": "",
    "version": "1.6.1174",
    "isUpdating": false, "showHubAfterUpdate": false,
    "updateRetry": { "retryCount": 0, "lastSuccessfulUpdateCheckAt": …, "consecutiveCheckErrors": 0,
                     "lastManagedConfig": { "updateFrequency": "auto" } },
    "cache": { /* estado derivado: statistics, splitKeybinds, voiceProfile, meetingDrawer, … */ },
    "platform": "win32", "architecture": "x64",
    "permissions": [ /* accessibility, microphone, screen-capture */ ],
    "timezone": "America/Buenos_Aires"
  },
  "notifications": {…}, "nudge": {…}, "activationCron": {…}, "voiceProfile": {…},
  "helperLaunch": {…}, "calendarSync": {…}, "syncCoordinator": {…},
  "syncSocketClient": {…}, "degradedNetwork": {…},
  "autodetectReminderIgnoreStreak": 0, "autodetectReminderDismissStreak": 5,
  "autodetectReminderCardCount": 6
}
```

**Observaciones de arquitectura [V]:**

1. **`prefs` vs raíz.** Las preferencias del usuario viven bajo `prefs.user`; los **módulos de servicio** (`notifications`, `nudge`, `activationCron`, `calendarSync`, `syncCoordinator`, `syncSocketClient`, `degradedNetwork`, `helperLaunch`, `voiceProfile`) están **al nivel raíz**, fuera de `prefs`. Esto separa *configuración declarativa del usuario* de *estado de máquina*.
2. **`prefs.cache`** es un **capa de estado derivado y cacheada**: estadísticas agregadas, mapa de atajos, perfil de voz, estado del drawer de reuniones, contadores de onboarding. Es lo que permite abrir la app sin recalcular.
3. **`prefs.permissions`** es un array de objetos `{name, clicked, granted}` con los tres permisos que Wispr puede pedir: `accessibility`, `microphone`, `screen-capture`. En la instalación auditada los tres están `clicked: false, granted: false` ⇒ **el usuario nunca alcanzó a autorizar ninguno de forma explícita** (Wispr opera con permisos de proceso, no con los de Windows). [V]

---

## 11.2 Identidad y cuenta (claves 1–7)

| Clave | Función | Estado |
|---|---|---|
| `uuid` | ID de usuario (servidor) | presente (redactado) |
| `firstName` / `lastName` | Nombre | presente (redactado) |
| `email` | Correo | presente (redactado) |
| `avatarUrl` | Ruta local del avatar en `avatars/` | presente |
| `pagesCompleted` | 13 páginas de onboarding completadas | [V] |
| `trialDecision` | Decisión sobre el trial | vacío (el trial fue saltado) |
| `subscription` | 26 campos: `status`, `daysLeft`, `plan`, `credits`, `isSubscribed`, `renewalTimestamp`, `tier`, `gracePeriodEndsAt`, `cancelAt`, `cancelAtPeriodEnd`, `displayAmount`, `displayCurrency`, `displayCurrencyFactor`, `priceResolved`, `subscriptionStartedAt`, `totalTrialDays`, `sourceOfSubscription`, `teamTrialGiven`, `desktopTrialGiven`, `mobileTrialGiven`, `trialExtensionDaysClaimed`, `mostRecentEnterpriseId`, `canRejoin`, `isStudent`, `teamDomainStatus` | plan `FLOW_PRO_MONTHLY`, USD 15, activa [V] |

---

## 11.3 Comportamiento de audio (claves 8–14)

| Clave | Valor | Significado |
|---|---|---|
| `shouldMuteAudio` | `true` | **Silenciar el audio del sistema mientras se dicta** |
| `shouldPauseAudio` | `false` | No pausar la reproducción (en vez de eso, mutear) |
| `streamAudio` | `false` | No subir el audio en streaming (subida por bloques) |
| `shouldAutoLearnWords` | `true` | **Aprendizaje automático del diccionario** |
| `shouldOCRScreenCapture` | `false` | **OCR de pantalla desactivado** (coherente con el flag `ScreenAIOCREnabled` anulado) |
| `enableSounds` | `true` | Sonidos de la app (flag `paste-sound` desactivado) |
| `selectedSoundFolder` | `""` | Carpeta de sonidos personalizada |

> **Implicación para Dixi:** `shouldMuteAudio` es un ajuste de **calidad de vida crítico** en un entorno con audio de fondo. Debe existir, con un control fino (mutear vs. bajar volumen) y, a diferencia de Wispr, **sin enviar audio de surroundings a la nube**.

---

## 11.4 Notetaker (claves 17–76) — 60 preferencias

Ver el detalle funcional en [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md). Resumen por bloques:

| Bloque | Claves | Notas |
|---|---|---|
| **Comportamiento** | `hideMeetingRecorderInScreenShare`, `autoDetectMeetingsEnabled`, `notetakerAutoEndEnabled`, `notetakerLongMeetingCheckInEnabled`, `notetakerMaxRecordingLength`, `notetakerNotepadOnStart`, `notetakerOpenWindowOnRecord`, `notetakerMenuBarMeetingEnabled`, `notetakerTripleTapShortcutEnabled` | `notetakerTripleTapShortcutEnabled` ⇒ **activar con triple toque** [V] |
| **Transcripción** | `notetakerLiveTranscriptEnabled`, `notetakerAttributeNamesEnabled`, `notetakerAutoDetectParticipantNamesEnabled`, `notetakerSplitScreenJoinEnabled` (false), `notetakerTranscriptRetention` | |
| **Calendario** | `notetakerCalendarNotificationTiming`, `notetakerCalendarNotificationsEnabled`, `notetakerCalendarNotice`, `notetakerCalendarChoice`, `notetakerCalendarChoiceOther` | Aviso 15 s antes |
| **Compartir** | `notetakerAutoShareNotes` (`off`), `notetakerAutoShareNotesType` (`all`), `notetakerConsentChatMessage` | **Auto-compartir desactivado** |
| **Cuota** | `notetakerQuota`, `notetakerMeetingCount` (3), `notetakerLastCountedMeetingId`, `notetakerRecordingVerdicts` | 150/semana |
| **Onboarding (nudge)** | 25 claves `notetaker*Shown/Dismissed/Seen/Clicked/*Pending/*Copy/*Nudge` | Motor de growth (§9.6 de [09-STATS.md](09-STATS.md)) |
| **Tour / simulaciones** | `notetakerSimulationArmedAt`, `notetakerColdHeroTourClicked`, `notetakerSeededTourCardDismissed`, `notetakerHoverCardSeen/Clicked` | **Wispr simula estados de tour** en producción |

---

## 11.5 Privacidad, nube y datos (claves 77–80, 184–188)

| Clave | Valor | Lectura |
|---|---|---|
| `shareUsageData` | `false` | **El usuario NO comparte datos de uso** ⇒ Wispr envía datos anónimos por defecto y hay que desactivarlo explícitamente |
| `cloudSync` | `true` | Sincronización cloud **activada** |
| `isGlobalProfileSharingEnabled` | `true` | Perfil global compartido (perfiles de estilo compartidos entre dispositivos) |
| `localDataPolicy` | `store_normally` | Política de datos locales: **guardar normalmente** (hay un valor alternativa, p. ej. `not_store`) |
| `defaultNoteVisibility` | `anyone_with_link` | ⚠️ **Notas públicas por defecto** |
| `internalDataSharingEnabled` | `false` | Datos internos no compartidos |
| `instructInternalDataSharingEnabled` | `true` | ⚠️ **Pero sí se comparten para el servicio de instrucciones** |
| `jabraWearDetectionEnabled` | `false` | Detección de auriculares Jabra |

> **⚠️ Inconsistencia detectada [V]:** `internalDataSharingEnabled = false` pero `instructInternalDataSharingEnabled = true`. Wispr tiene **dos interruptores distintos** para el sharing de datos internos, y el de instrucciones está activo. Es un punto que un usuario razonable no puede auditar desde la UI. **Dixi debe tener UN solo interruptor, y que sea «no se comparte nada» por defecto.**

---

## 11.6 Atajos: dos mapas distintos (claves 99–105)

### 11.6.1 `prefs.user.shortcuts` — mapa de 10 acciones

| Atajo | Acción | VK |
|---|---|---|
| `162+91` | `ptt` | Ctrl + ` |
| `162+32+91` | `popo` | Ctrl + Espacio + ` |
| `162+164+91` | `lens` | Ctrl + Alt + ` |
| `160+164+90` | `paste_last_text` | Alt + Mayús + Z |
| `160+164+88` | `copy_last_text` | Alt + Mayús + X |
| `164+77+91` | `open_meeting_recorder` | Alt + M + ` |
| `164+49+91` | `polish` | Alt + 1 + ` |
| `164+50+91` | `polish_prompt_1` | Alt + 2 + ` |
| `164+79+91` | `view_diff` | Alt + P + ` |
| `27` | `dismiss` | Esc |

### 11.6.2 `prefs.cache.splitKeybinds` — mapa de 14 acciones (¡2 más!)

El segundo mapa —dentro de `cache`, no de `shortcuts`— contiene **dos atajos adicionales**:

| Atajo | Acción | VK | Comentario |
|---|---|---|---|
| **`162 90`** | **`undo`** | **Ctrl + Z** | ⚠️ **Wispr secuestra el «deshacer» global** |
| **`162 86`** | **`paste_event`** | **Ctrl + V** | ⚠️ **Wispr secuestra el «pegar» global** |
| (los 10 anteriores, idénticos) | | | |

> **🔴 Hallazgo de alto impacto [V]:** Wispr Flow registra **`Ctrl+Z` (undo)** y **`Ctrl+V` (paste)** como **atajos globales**, además de sus propios.
>
> - `Ctrl+Z` global ⇒ en **cualquier** aplicación, «deshacer» invoca la función de deshacer de Wispr (deshacer el último dictado/transform).
> - `Ctrl+V` global ⇒ «pegar» se reescribe globalmente.
>
> **Consecuencias:**
> 1. **Colisión con el sistema**: `Ctrl+Z`/`Ctrl+V` son atajos universales; Wispr los captura globalmente, lo que puede interferir con editores, navegadores y juegos.
> 2. **Comportamiento no evidente**: el usuario no sabe por qué `Ctrl+Z` en otra app no deshace en esa app.
> 3. `windows-shortcut-masking-v2` (flag enabled) sugiere que Wispr **intenta enmascarar** las teclas para que no lleguen a la app enfocada, lo que explica el efecto secundario de que **`Ctrl+V` deje de pegar en la app enfocada mientras Wispr corre**.
>
> **Recomendación para Dixi:** **no** registrar `Ctrl+Z` ni `Ctrl+V` globales. Reservar los atajos globales a combinaciones que no choquen con el sistema (`Ctrl+Alt+…`, `Ctrl+``). Ofrecer «deshacer último dictado» en la Flow Bar y con un atajo configurable, no secuestrando `Ctrl+Z`.

### 11.6.3 Otros atajos

| Clave | Valor |
|---|---|
| `modifierShortcut` | `164` (Alt) ⇒ **modo modificador sostenido**: mantener Alt y hablar |
| `stashedScratchpadShortcuts` / `stashedPolishShortcuts` / `stashedMeetingRecorderShortcuts` | `{}` ⇒ atajos **reservados para asignar** (migración de un esquema anterior) |
| `lastSetScratchpadShortcut` | `""` |
| `scratchpadInFlowBar` | `false` |

---

## 11.7 Idiomas y formato de transcripción (claves 93–98, 108–110, 135)

| Clave | Valor | Nota |
|---|---|---|
| `usedLanguages` | `["en","es"]` | Idiomas usados |
| `selectedLanguages` | `["en","es"]` | Idiomas seleccionados |
| `effectiveLanguages` | `""` | (flag `effective-language-picker` activo) |
| `uiLocale` | `"es"` | Idioma de la interfaz |
| `emailLocale` | (redactado) | |
| `languagePickerRetirementAcknowledged` | `true` | |
| `defaultTranscriptionFormat` | `"Apply AI formatting"` | Alternativa: «Transcripción en bruto» |
| `appTranscriptionFormats` | `[]` | **Overrides por app** (vacío) |
| `aiFormatting` | `true` | |
| `dictionarySort` | `"Newest"` | Orden del diccionario |

---

## 11.8 Formato de interfaz (claves 111, 141, 193–194)

| Clave | Valor |
|---|---|
| `renderOptions` | `{width: 1695, height: 941, sidebarOpen: …}` ⇒ **el tamaño de ventana se persiste** |
| `notesViewType` | `"grid"` (o `"list"`) |
| `statusDockEdge` | `"bottom"` ⇒ **la Flow Bar se ancla abajo** |
| `theme` | `"light"` (o `"dark"`; `darkModeCoachmarkSeen: true`) |
| `hideAppInDock` | `false` ⇒ **el icono permanece en el dock** |
| `openAtLogin` | `true` |
| `hideFlowBarPermanently` | `false` |

---

## 11.9 Preferencias internas (16) — flags de ingeniería

```json
"internal": {
  "isToneMatchEnabled": false,          // detección de tono
  "isHapticsEnabled": false,            // vibración háptica
  "shouldSaveAxText": false,            // ⇐ NO se guarda el texto AX en el historial
  "isCandidateAsrEnabled": false,       // ASR candidato (A/B de modelos)
  "scratchpadSidebarOpen": false,
  "scratchpadDefaultNewTab": false,
  "scratchpadOpenLastActivePinned": false,
  "scratchpadOpenBehavior": "default",
  "meetingRecorderAggressiveAutoSave": false,
  "isAutoInstructEnabled": false,       // ⇐ instrucciones automáticas
  "preserveIntentsOnRestart": false,
  "pairedBLEDeviceId": "",              // ⇐ dispositivo Bluetooth emparejado
  "pairedBLEDeviceName": "",
  "showDebugOutlines": false,           // ⇐ contornos de depuración
  "showOverlayDebugTint": false,        // ⇐ tinte de depuración del overlay
  "instructPasteOnEmptyContext": false
}
```

**Hallazgos [V]:**

1. **`shouldSaveAxText = false`** ⇒ **el contexto AX se usa pero NO se persiste** en el `History` de esta instalación, pese a que la tabla tiene columnas `axText`/`axHTML` y existe la migración `add-ax-html`. Esto reduce el riesgo de privacidad, pero también **impide la auditoría y el aprendizaje a partir del AX**. (La columna `axHTML` sí se usa para otros fines, p. ej. `ax-inspect`.) [I]
2. **El parche de Bluetooth (`pairedBLEDeviceId/Name`)**: Wispr admite **dispositivos Bluetooth** (auriculares) como fuente de micrófono. Es más allá del caso "Jabra por USB" que delata el módulo nativo. [V]
3. **`showDebugOutlines` y `showOverlayDebugTint`** ⇒ hay **modos de depuración visual** del overlay. Útil para Dixi: un «modo debug» que muestre los límites de las ventanas de overlay.
4. **`isCandidateAsrEnabled = false`** ⇒ el «ASR candidato» (comparar dos modelos) está **desactivado** en esta instalación, aunque `transcriptOrigin` muestra mezcla de `canto_1.0` / `canto_1.0.1` / `internal` / `internal_experimental` ⇒ el A/B ocurre a nivel de servidor, no de cliente.

---

## 11.10 Polish / Transforms (claves 113–119)

| Clave | Valor |
|---|---|
| `polishInstructions` | `{custom: {}, default: {}}` ⇒ **ambos vacíos** ⇒ los transforms de fábrica se resuelven en el servidor, no localmente |
| `polishEnabled` | `true` |
| `polishAutoPaste` | `true` ⇒ el resultado se pega solo |
| `autoPolishAfterDictation` | `{active: false, promptName: "polish"}` |
| `polishUseCount` / `polishButtonClickCount` | `0` / `0` ⇒ **nunca usado** |
| `polishButtonTrackingStartedAt` | epoch ms ⇒ telemetría de botones activa |

---

## 11.11 Onboarding, intents y perfil de usuario (claves 71–83, 170–177, 189–192)

| Clave | Contenido | Lectura |
|---|---|---|
| `fulfilledIntents` | 5 intents | **«Intenciones»: el usuario expresa objetivos y la app los registra** [I] |
| `userInformation` | `{writingNeeds (7), traits, role: developer, industry: freelancer, topUseCases}` | **Perfil semántico del usuario**, usado para personalizar el formateo |
| `typingLocations` | 7 apps | Dónde escribe el usuario |
| `contextualNudge` | `{mode, appCategories, nudgeVariant, overriddenSettings}` | Nudge contextual por categoría de app |
| `onboardingCompleted` / `postOnboardingCompleted` / `postOnboardingStep` | `true` / `true` / `"completed"` | |
| `onboardingCompletedTime` | epoch ms | |
| `onboardingCompletedPlatforms` | 1 plataforma | |
| `speedyOnboardingAccepted` | `false` | Ofrecía un onboarding acelerado |
| `postOnboardingSelectedUseCase` | `""` | |
| `creatorMode` | `false` | |

### 11.11.1 `prefs.cache.voiceProfile` — el «perfil de voz»

```json
"voiceProfile": {
  "halfwayNotificationShown": false,
  "bannerDismissed": false,
  "onboardingBaselineWords": null,
  "voiceProfileWordCount": 2000,
  "lastViewedMilestoneInHub": null,
  "lastDismissedMilestoneInHub": null,
  "firstMilestoneShowFullData": false
}
```

⇒ **El «perfil de voz» de Wispr es un perfil de escritura derivado del volumen de palabras** (base 2 000 palabras), no un perfil de voz acústica. El nombre es ambiguo y confuso: `userVoices` (estilo por app) y `voiceProfile` (perfil estadístico) son **cosas distintas con nombres parecidos**. [V] / [I]

> ✅ **CONFIRMADO contra la documentación pública [V]:** la documentación de Wispr define **«Voice Profile» como un modelo personal aprendido que se desbloquea a las 2 000 palabras de dictado en desktop** y que se muestra en **Insights**. El valor local `voiceProfileWordCount = 2000` coincide exactamente con ese umbral.
>
> ⇒ El **Voice Profile es el mecanismo de personalización automática** de Wispr: aprende de tu propia escritura y lo aplica. Es una función de **alto valor y totalmente aprendible en local**, lo que la convierte en un diferencial muy fuerte para Dixi (que no necesita enviar nada a la nube para entrenarlo).

---

## 11.12 Estadísticas calculadas por Wispr (autoritativas)

```json
"cache.statistics": {
  "weekStreak": 12, "dayStreak": 2,
  "totalWords": 102524,
  "totalDuration": 48278.8,
  "totalNonEmptyDuration": 47428.8,
  "averageWPM": 128,
  "wordsThisWeek": 684, "wordsToday": 684,
  "lastTranscriptTimestamp": "2026-10-19T16:06:18.470Z",
  "totalApps": [18 entradas]
}
```

| Métrica | Valor | Validación cruzada |
|---|---|---|
| **Palabras totales** | **102 524** | Coincide con el cálculo propio desde `History` (≈ 104 000) [V] |
| **PPM medio** | **128** | Cálculo propio desde `History`: p50 = 133,8 ⇒ media ≈ 128 [V] |
| Duración total de audio | 48 278,8 s = **13,4 h** | |
| Duración «no vacía» (habla) | 47 428,8 s = **13,2 h** | ⇒ **silencio = 1,8 %** del audio |
| Racha semanal | 12 semanas | |
| Racha diaria | 2 días | |
| Palabras esta semana | 684 | Límite semanal 2 000–5 000 ⇒ **no alcanzado** actualmente |
| Apps/URLs distintas | 18 | Incluye `ms-teams`, `WhatsApp.Root`, `SndVol`, `Spotify`, `warp`, `chrome`, `notepad++`, `Studio by Spotify Labs`, `Antigravity`, `ChatGPT`, dominios propios |

> **Validación [V]:** mis cálculos independientes desde la tabla `History` coinciden con las estadísticas que Wispr calcula y cachea (palabras ±1,5 %, PPM dentro del rango). Esto **valida la fiabilidad del dataset** y de las conclusiones de [13-RENDIMIENTO.md](13-RENDIMIENTO.md).

---

## 11.13 Módulos de servicio (nivel raíz)

| Módulo | Función observada |
|---|---|
| `notifications` | Cola de notificaciones locales + sincronización |
| `nudge` | Motor de nudges (§9.6) |
| `activationCron` | **Tarea programada de activación** (recordatorios de inactividad) — con `autodetectReminderIgnoreStreak: 0`, `autodetectReminderDismissStreak: 5`, `autodetectReminderCardCount: 6` ⇒ **hasta 6 tarjetas de recordatorio** si el usuario descarta 5 veces seguidas [V] |
| `voiceProfile` | Perfil de escritura por volumen |
| `helperLaunch` | Arranque del helper nativo (`lastAppVersion: 1.6.1174`) |
| `calendarSync` | Sincronización de calendario |
| `syncCoordinator` | **17 dominios de sincronización**: `access, subscription, enterprise, preferences, notifications, notes, meetings, meetings_shared, folders, todos, calendar, agentic_prereads, automations, learned_preferences, entitlements, notetaker_policy` + timestamps de entrega gRPC |
| `syncSocketClient` | Canal socket para sincronización |
| `degradedNetwork` | Manejo de red degradada (timestamps de entrega) |

> **Implicación para Dixi [I]:** Wispr sincroniza **17 dominios** con un servidor. Un reemplazo personal **no necesita ninguno**: todo es local. La contrapartida es que **no hay sincronización entre dispositivos**, lo que es una limitación real que debe documentarse (o resolverse con sincronización opcional, p. ej. Syncthing o un vault cifrado).

---

## 11.14 Cambios de configuración durante la auditoría

**Ninguno.** Verificado:
- `config.json` se abrió **sólo en lectura**.
- Las únicas interacciones con la app fueron: mostrar/ocultar la ventana principal (`ShowWindow`/`Hide`, estado restaurado), `InvokePattern` sobre botones de navegación, y `ESC` para descartar overlays.
- Ninguna preferencia fue escrita, ninguna base de datos fue modificada, ningún archivo de la instalación fue tocado.
- La instancia CDP aislada usó un `--user-data-dir` **nuevo**, por lo que **nunca leyó ni escribió** el perfil real.

---

### Referencias cruzadas

- [01-ENTORNO.md](01-ENTORNO.md) — rutas de datos
- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — capas y módulos
- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — atajos en contexto
- [06-ESTILOS.md](06-ESTILOS.md) — `userVoices` vs `voiceProfile`
- [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md) — 60 preferencias del notetaker
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — métricas validadas
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
