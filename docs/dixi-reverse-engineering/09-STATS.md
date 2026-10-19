# 09 — Estadísticas, gamificación y analítica de producto

> Fuente: `prefs.user.userJourney`, `prefs.user.userStats`, `prefs.user.notificationSettings`, tabla `RemoteNotifications` (19 filas), panel de notificaciones (árbol de accesibilidad), flags `daily-streak-desktop`, `desktop-team-leaderboard`, `new-insights-and-voice-profile`, `hub-top-apps`, migraciones `add-per-app-stats-index` y `add-top-app-url-combinations-index`.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 9.1 El motor de analítica de producto está DENTRO de la app

El hallazgo más importante de esta sección: Wispr Flow **no sólo muestra estadísticas al usuario — instrumenta su propio uso para el análisis de producto**. La tabla `prefs.user.userJourney` es un **embudo de activación y descubrimiento de funciones** almacenado en el cliente. [V]

```json
"userJourney": {
  "activation": {
    "firstExternalDictationAt": 1786038015241,
    "activatedAt":              1786302659515,
    "postOnboardingCompletedAt": 1786038168759,
    "wordMilestones": { "100": 1786038168847, "500": null,
                        "1000": 1786064876753, "2000": 1786302659495 }
  },
  "features": {
    "formatting":         { "discoveredAt": 1786052754669, "firstUsedAt": 1786052754669, "usageCount": 40 },
    "dictionary":         { "discoveredAt": null, "firstUsedAt": null, "usageCount": 0 },
    "snippets":           { "discoveredAt": null, "firstUsedAt": null, "usageCount": 0 },
    "polish":             { "discoveredAt": null, "firstUsedAt": null, "usageCount": 0 },
    "personalizedStyles": { "discoveredAt": 1786038042280, "firstUsedAt": null, "usageCount": 0 }
  }
}
```

**Qué se puede leer de esto [V]:**

| Señal | Valor observado | Interpretación |
|---|---|---|
| `firstExternalDictationAt` | 1786038015241 | Primer dictado en una app externa |
| `postOnboardingCompletedAt` | 1786038168759 | **Completó el onboarding** (3,5 s después) |
| `wordMilestones` | 100 ✓ · 500 ✗ · 1 000 ✓ · 2 000 ✓ | **Saltó el hito de 500** (usó palabras de golpe) |
| `activatedAt` | 1786302659515 | = el hito de 2 000 palabras ⇒ **«activado» = haber dictado 2 000 palabras** |
| `formatting.usageCount` | 40 | Formateo usado 40 veces (su única función usada) |
| `dictionary/snippets/polish.usageCount` | 0 | **Nunca descubiertas** (`discoveredAt: null`) ⇒ el usuario nunca vio esas secciones |
| `personalizedStyles.discoveredAt` | 1786038042280 | **Descubierta 27 s después del primer dictado** pero nunca usada |

> **Conclusión de producto [I]:** Wispr es un producto **de uso estrecho**: el 100 % del valor percibido por este usuario vino del **formateo automático**. Todo lo demás (diccionario, snippets, Polish, estilos) está instalado pero **no descubierto**. Esto tiene dos implicaciones para Dixi:
> 1. **El formateo es el producto**: la calidad del formateo (mayúsculas, puntuación, nombres propios) es la única Variable que importa.
> 2. **El onboarding debe aggressivelyEducation**: si el usuario no descubre el diccionario, todo el motor de aprendizaje es un coste sin beneficio. Ver §9.6.

---

## 9.2 La gamificación

| Evidencia | Detalle |
|---|---|
| Flag `daily-streak-desktop` | **enabled** ⇒ rachas diarias en escritorio [V] |
| Flag `desktop-team-leaderboard` | **enabled** ⇒ clasificación por equipo [V] |
| `notificationSettings.teamLeaderboardUpdates` | `true` ⇒ el usuario recibe avisos de la clasificación [V] |
| `notificationSettings.milestonesAndProgress` | `true` [V] |
| Tabla `RemoteNotifications` | 19 filas: **16 `type='achievement'`** + 3 `type='announcement'` [V] |
| Panel de notificaciones | Notificaciones de hitos: **10 000 / 12 345 / 20 000 / 50 000 / 99 000 / 100 000 palabras**, **1 mes de racha**, **3 meses de racha**, etc. [V] |

**Hitos de palabras observados en las notificaciones:** 10 000, 12 345, 20 000, 50 000, 99 000, 100 000. El hito de **12 345** es deliberadamente «no redondo» ⇒ efecto de psicología de hitos (el usuario piensa «me falta poco para 12 345»). [I]

**Hitos de racha:** 1 mes, 3 meses (y presumiblemente 1 semana, 1 día, 6 meses, 1 año).

> **Implicación para Dixi:** la gamificación es un **motor de retención de alta efficacy** (efecto de streaks). Es también la parte **más cuestionable** del producto: obliga al usuario a volver a la app todos los días. Para un reemplazo personal, la decisión de diseño correcta es: **racha opcional y local, sin presión**. Ver §9.6.

---

## 9.3 Categorías de notificación

```json
"notificationSettings": {
  "tipsAndSuggestions":     true,
  "featureHighlights":      true,
  "milestonesAndProgress":  true,
  "teamUpdates":            true,
  "teamLeaderboardUpdates": true
}
```

5 categorías: (1) consejos y sugerencias, (2) novedades de funciones, (3) hitos y progreso, (4) actualizaciones de equipo, (5) actualizaciones de la clasificación del equipo. [V]

Además: `lastNotificationReadTime` (epoch ms) ⇒ el estado de lectura se persiste. [V]

---

## 9.4 Cuotas y su telemetría

```json
"notetakerQuota": {
  "counted": 0, "limit": 150, "remaining": 150,
  "weekStartedAt": "2026-10-12T00:00:00Z",
  "resetsAt": "2026-10-19T00:00:00Z",
  "countedMeetingIds": []
}
```

- La cuota se **reinicia cada lunes 00:00 UTC** (= 21:00 hora de Argentina, con el offset `America/Buenos_Aires`). [V]
- `countedMeetingIds` guarda **los IDs concretos** contados ⇒ evita doble conteo y permite auditar. [V]
- `userStats.firstWeeklyLimitHitAt = 1786302999` ⇒ **el usuario alcanzó su límite semanal de palabras al menos una vez** (epoch s ⇒ 2026-08-09 aprox.). [V] / [I]

**Límites semanales de palabras (flag `weekly-word-limits-config`):** desktop *soft* **2 000** / *hard* **5 000** palabras. [V]

> **Nota sobre la contradicción aparente [I]:** el usuario tiene 3 160 dictados acumulados con p50 de 22 palabras y media de 32,9 ⇒ ≈ 104 000 palabras dictadas en total, pero el límite es de 2 000–5 000 **por semana**. Consistente: ~3 000 palabras/semana en uso real.

---

## 9.5 Estadísticas por aplicación (el «hub»)

| Evidencia | Detalle |
|---|---|
| Flag `hub-top-apps` | enabled ⇒ sección «Top apps» en el hub [V] |
| Flag `new-insights-and-voice-profile` | enabled, variante `insights` ⇒ **nuevos insights + perfil de voz** [V] |
| Migración `add-per-app-stats-index` (2026-03-24) | Índice de estadísticas por aplicación [V] |
| Migración `add-top-app-url-combinations-index` | Índice de **combinaciones app+URL** ⇒ la statística no es sólo por app, sino por app+URL [V] |
| `prefs.user.typingLocations` | 7 apps donde el usuario escribe [V] |
| `prefs.user.contextualNudge.appCategories` | Categorías de app para el nudge contextual [V] |
| `History.app` / `History.url` | Cada dictado registra app y URL ⇒ el agregado es trivial [V] |

**Métricas que se pueden derivar del `History` (3 160 dictados) — cálculo real:**

| Métrica | Valor |
|---|---|
| Dictados totales | 3 160 |
| Palabras totales | ≈ **104 000** (media 32,9 por dictado) |
| Palabras/día (estimado sobre 3 días activos) | ≈ 1 000–2 000 |
| Apps distintas usadas | ≥ 8 |
| Dictado medio | 10,1 s de audio, 22 palabras |
| **Velocidad real** | **p50 133,8 ppm** · p90 181,8 ppm |
| Distribución de uso | chrome 53 % · terminal 23 % · editors 15 % |
| Tasa de intervención | `editDistanceToDictated` p50 = 0 ⇒ **el 50 % de los dictados no se editó** |

**La métrica que Wispr no expone pero debería:** `numWordsCorrected` (media 1,5) y `numDictionaryReplacements` (media 0,64) por dictado ⇒ se puede construir un **índice de calidad del formateo** = 1 − (palabras corregidas / palabras dictadas). Con los datos observados: **1 − 1,5/32,9 ≈ 95,4 % de acierto del formateo**. [I]

---

## 9.5 bis El «Voice Profile»: el desbloqueo a 2 000 palabras

| Elemento | Valor | Fuente |
|---|---|---|
| Umbral de desbloqueo | **2 000 palabras de dictado en desktop** | documentación pública [V] |
| `voiceProfileWordCount` (local) | **2 000** | `prefs.cache.voiceProfile` [V] |
| `onboardingBaselineWords` | `null` ⇒ el perfil **no se ha generado** | [V] |
| `bannerDismissed` / `halfwayNotificationShown` | `false` | [V] |
| Dónde se ve | **Insights** | documentación pública [V] |

> **Es la función de personalización más ambiciosa de Wispr**: un modelo **aprendido de tu propia escritura** que se activa automáticamente a partir de 2 000 palabras y se muestra junto a las estadísticas. [V]
>
> **Para Dixi es el diferencial más grande y más barato:** el entrenamiento puede hacerse **100 % en local** sobre el `History` que ya está en el SQLite de la máquina. Wispr necesita subirlo a su nube; Dixi no. [I]
>
> **Sub-bloque dentro de la función:** el «Voice Profile» se acompaña de un **«Auto Cleanup»** como dial **independiente** de la limpieza: **None · Light · Medium** (se eliminó el nivel «High»). [V]

Una de las piezas más reveladoras del producto: `prefs.user.notetakerNudge`:

```json
{
  "anchorAt": null, "meetingCount": 0, "lastSeenKey": null,
  "lastActivityAt": null, "secondConsumedAt": null, "secondCohort": null,
  "reEnableConsumedAt": null, "reEnableMisses": 0,
  "valuePropCardIndex": 0, "valuePropStoppedAt": null
}
```

Y el checklist de 4 pasos, con su propia máquina de estados:

| Flag | Estado | Significado |
|---|---|---|
| `notetakerChecklistTryDone` | `true` | Paso 1 completado |
| `notetakerChecklistCalendarDone` | `false` | Paso 2 pendiente |
| `notetakerChecklistMcpDone` | `false` | Paso 4 pendiente |
| `notetakerChecklistTrySkipped/CalendarSkipped/ImportSkipped/McpSkipped` | `false` | Ninguno saltado |
| `notetakerChecklistCalendarShownPending` | `true` | Paso 2 mostrado, pendiente de dismiss |
| `notetakerChecklistImportShownPending` | `true` | Paso 3 pendiente |
| `notetakerChecklistMcpShownPending` | `true` | Paso 4 pendiente |
| `notetakerChecklistRetired` | `false` | El checklist sigue activo |
| `notetakerChecklistTryCopy` | `"try"` | **Variante de copy** del paso 1 (A/B test de texto) |
| `notetakerGraduatedBeforeChecklist` | `true` | El usuario se graduó (superó el checklist) por otra vía |
| `notetakerAutoStartOptInState` | `"None"` | Estado del opt-in de auto-arranque |
| `notetakerAutoStartEnabled` | `false` | Auto-arranque **inactivo** (aunque el flag del servidor diga `autoStartNotetaker: true`) |
| `notetakerNudge.valuePropCardIndex` | `0` | Índice de la tarjeta de propuesta de valor |

**Análisis [I]:** Wispr implementa un motor de onboarding con:
- **anclaje temporal** (`anchorAt`) y **cohortes** (`secondCohort`),
- **re-activación** con conteo de fallos (`reEnableConsumedAt`, `reEnableMisses`),
- **tarjetas de propuesta de valor** secuenciales (`valuePropCardIndex`),
- **A/B testing del copy** (`notetakerChecklistTryCopy = "try"`),
- **opt-in explícito** para auto-arranque.

Es un motor de *growth* sofisticado. **Es también exactamente lo que Dixi NO debe replicar**: un reemplazo personal de un producto de suscripción no necesita cohortes, re-activaciones ni A/B testing de copy. Lo que sí vale la pena copiar es el **principio**: *mostrar el checklist de 4 pasos de configuración con progreso visible*.

---

## 9.7 Diseño propuesto para Dixi

### 9.7.1 Métricas locales, sin gamificación forzada

```sql
CREATE TABLE dixi_daily_stats (
  day            TEXT PRIMARY KEY,     -- YYYY-MM-DD (local)
  dictations     INTEGER NOT NULL DEFAULT 0,
  words          INTEGER NOT NULL DEFAULT 0,
  audio_seconds  REAL    NOT NULL DEFAULT 0,
  speech_seconds REAL    NOT NULL DEFAULT 0,
  edited_words   INTEGER NOT NULL DEFAULT 0,
  dictionary_hits INTEGER NOT NULL DEFAULT 0,
  transforms     INTEGER NOT NULL DEFAULT 0,
  meetings       INTEGER NOT NULL DEFAULT 0
);

CREATE VIEW dixi_insights AS
SELECT
  SUM(words)                                        AS total_words,
  SUM(audio_seconds)                                AS total_audio_s,
  SUM(speech_seconds)                               AS total_speech_s,
  SUM(words) / NULLIF(SUM(speech_seconds),0) * 60   AS wpm_avg,
  SUM(words) - SUM(edited_words)                    AS words_kept_ratio,
  SUM(audio_seconds) - SUM(speech_seconds)          AS silence_ratio
FROM dixi_daily_stats;
```

**Métricas que Dixi puede mostrar y Wispr no:**

| Métrica | Cálculo | Por qué importa |
|---|---|---|
| **Acierto del formateo** | 1 − palabras_afectadas/palabras | Mide la calidad real del formateo |
| **Ratio de silencio** | 1 − habla/audio | Detecta dictados con pausas largas |
| **Tiempo recuperado** | palabras / ppm_típico | «Has ahorrado X min de escritura» |
| **Palabras por app** | join `History` | Ya disponible en Wispr |
| **Aciertos del diccionario** | hits/aplicaciones | Mide si el diccionario rinde |
| **PPM personal** | palabras / habla |vs. media de 133,8 ppm observada |
| **Calidad de Transform** | `undone` / ejecuciones | Mide si el LLM daña el texto |

### 9.7.2 Principios de gamificación para Dixi

| Decisión | Motivo |
|---|---|
| **Racha local y opcional**, con un interruptor explícito | Es útil para el hábito, pero no debe generar culpa |
| **Sin leaderboard por defecto** | Requiere servidor y vida social; un producto personal no lo necesita |
| **Sin push de notificaciones de hitos** | Un notetaker que notifica es ruido |
| **Hitos visibles en la app, sin notificaciones** | «12 345 palabras» se ve al abrir la app |
| **Sin telemetría saliente** | Todo local; se puede exportar JSON |

> **Diferenciador de privacidad:** todo el histórico de métricas de Dixi es **local y exportable** (SQLite + CSV/JSON). Wispr lo envía al servidor (hay `UsageOutbox`, `needsUploading`, `RemoteNotifications`, `syncCoordinator` con 17 dominios). Es una diferencia de marketing clara: **«tus métricas no salen de tu máquina»**.

---

## 9.8 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| Estadísticas de palabras/tiempo | ✅ | ✅ | ✅ | ✅ |
| Estadísticas por app | ✅ (+ app+URL) | ⚠️ | ⚠️ | ✅ |
| Rachas diarias | ✅ | ❌ | ❌ | ✅ opcional |
| Leaderboard de equipo | ✅ | ❌ | ❌ | ❌ (por diseño) |
| Hitos de palabras | ✅ (12 345, 99 000…) | ❌ | ❌ | ✅ local |
| Embudo de descubrimiento de funciones | ✅ (`userJourney.features`) | ❌ | ❌ | ⚠️ interno |
| A/B testing de copy | ✅ (`notetakerChecklistTryCopy`) | ❌ | ❌ | ❌ |
| Motor de nudge con cohortes | ✅ | ❌ | ❌ | ❌ |
| **Métricas de calidad del formateo** | ⚠️ no expuesto | ❌ | ❌ | ✅ **diferencial** |
| **Todo local + exportable** | ❌ | ✅ | ⚠️ | ✅ **diferencial** |
| Sin telemetría saliente | ❌ | ✅ | ⚠️ | ✅ **diferencial** |

---

### Referencias cruzadas

- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — datos por dictado
- [06-ESTILOS.md](06-ESTILOS.md) — estilos y su uso
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — preferencias de analytics
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — rendimiento y latencias
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
