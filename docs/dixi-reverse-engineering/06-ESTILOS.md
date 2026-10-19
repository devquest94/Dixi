# 06 — Estilos de escritura y «Voces» (per-context profiles)

> Fuente: `prefs.user.userVoices`, `prefs.user.personalizationStyles`, `prefs.user.autoCleanup`, tabla `UserVoicePreferences`, columna `History.personalizationStyleSettings`, flag `new-insights-and-voice-profile` y el árbol de accesibilidad de la sección «Estilo».
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 6.1 El concepto central: Wispr llama «Voice» a lo que otros llaman «estilo»

**Terminología de Wispr:** una **Voice** es un **perfil de estilo vinculado a un conjunto de aplicaciones**. La sección del sidebar se llama **«Estilo»** y el modelo interno usa `userVoices`. [V]

Esto es una decisión de producto brillante: en lugar de "configurá un estilo por app", Wispr agrupa en **4 perfiles semánticos** y deduce la app automáticamente.

---

## 6.2 Las 4 voces integradas (verificadas)

```json
"userVoices": {
  "work": {
    "id": "work", "source": "builtIn", "name": "Work Messages",
    "appNames": ["Slack", "Microsoft Teams"],
    "autoCleanupLevel": "light", "stylePreference": "formal"
  },
  "email": {
    "id": "email", "source": "builtIn", "name": "Email",
    "appNames": ["Gmail", "Microsoft Outlook", "Superhuman"],
    "autoCleanupLevel": "light", "stylePreference": "formal"
  },
  "personal": {
    "id": "personal", "source": "builtIn", "name": "Texting",
    "appNames": ["Messages", "WhatsApp", "WeChat", "Messenger"],
    "autoCleanupLevel": "light", "stylePreference": "formal"
  },
  "other": {
    "id": "other", "source": "builtIn", "name": "Others",
    "appNames": [],
    "autoCleanupLevel": "light", "stylePreference": "formal"
  }
}
```

| Voice | Nombre visible | Apps asignadas | Limpieza | Estilo |
|---|---|---|---|---|
| `work` | **Work Messages** | Slack, Microsoft Teams | light | formal |
| `email` | **Email** | Gmail, Microsoft Outlook, Superhuman | light | formal |
| `personal` | **Texting** | Messages, WhatsApp, WeChat, Messenger | light | formal |
| `other` | **Others** | *(ninguna — fallback)* | light | formal |

**Detalles de diseño [V]:**

1. `source = "builtIn"` ⇒ las 4 son **predefinidas**, pero el esquema admite `source` distintos ⇒ Wispr contempla **voces creadas por el usuario**.
2. `appNames` es una lista de **nombres de aplicación legibles**, no identificadores ⇒ Wispr hace matching por nombre de proceso/app, coherente con lo observado en `History.app` (`ms-teams`, `chrome`, `notepad++`).
3. **`appNames` vacío en `other`** ⇒ `other` es el perfil de **fallback** cuando la app no está en ninguna lista.
4. El perfil `personal` (Texting) tiene el estilo `formal` ⇒ el usuario puede **sobrescribir** el valor por defecto (si no, lo esperable sería `casual`). Es una Confirmación de que `stylePreference` es por voz, no global. [I]
5. `email` incluye **Superhuman** ⇒ Wispr reconoce clientes de correo de terceros, no sólo el cliente por defecto.

---

## 6.3 Los estilos: 5 grupos + un desplegable de 5 opciones

Del árbol de accesibilidad de la sección «Estilo» (`focus-overlay-target-sidebar-style`):

- **5 grupos de estilos conmutables** (pulsables), cada uno con:
  - un encabezado,
  - un botón **«Modificar»** (`Modificar`),
  - un botón de **copia/duplicado** (icono `⧉`),
  - un botón **«Eliminar»**,
  - un botón **«Probar con una muestra»** (`Probar con una muestra`) ⇒ **campo de prueba con estilo aplicado**.
- **1 desplegable de estilo** (`DropdownButton`, `aria-haspopup="listbox"`, `aria-expanded="false"`) con **5 opciones**.

Combinado con `personalizationStyles` = `{work, email, other, unset, personal}` (5 claves) ⇒ **los 5 estilos son las categorías**, y la tabla de estilos por categoría es `personalizationStyles`. [V] / [I]

**Los 5 estilos observados** (por el `value` del desplegable y el estado por categoría): `formal`, `casual`, `default` (para `unset`), + 2 más no exponibles sin interacción adicional. **[NV]** sobre los nombres exactos de los 2 restantes.

> **Verificado contra la documentación pública [V]:** los presets de estilo publicados son **Formal · Casual · very casual · Excited!** ⇒ el desplegable de 5 opciones de la app = esos 4 + `default` (para `unset`). El valor `formal` observado en las 4 Voces **confirmado**.

**`styleDetectionEnabled = true`** ⇒ Wispr **detecta el estilo automáticamente** en vez de obligar a elegirlo. [V]

**Separación clave [V]:** `stylePreference` (dentro de cada Voice) ≠ `personalizationStyles` (mapa categoría→estilo). Son **dos niveles**:

```
App detectada  →  Voice (work/email/personal/other)  →  estilo (personalizationStyles[voice])
                                                        +  nivel de limpieza (voice.autoCleanupLevel)
```

---

## 6.4 Niveles de limpieza automática

```json
"autoCleanup": { "level": "light", "diffShownCount": 0, "migrated": true }
```

- `level` = `"light"` en toda la instalación (las 4 voces y el global). [V]
- `diffShownCount = 0` ⇒ **nunca se ha mostrado un diff de limpieza**. [V]
- `migrated = true` ⇒ el esquema de niveles es una **migración de un esquema anterior** (Wispr cambió de un modelo de limpieza a otro y migró las preferencias). [V]

**Interpretación [I]:** el nivel de limpieza controla la agresividad con que se eliminan muletillas y relleno («eh», «o sea», «mmm»), se unifican mayúsculas, se quitan repeticiones, etc. Los niveles previsibles son `light` / `medium` / `heavy` (o `off`).

**Dato de calidad [V]:** en 3 160 dictados, `numWordsCorrected` media 1,5 y p90 4 ⇒ con limpieza `light` Wispr toca ~1,5 palabras por dictado. Es **conservador** a propósito: la limpieza agresiva destruye texto técnico.

---

## 6.5 Estilos a nivel de aplicación y el modelo de datos

### 6.5.1 Preferencias por voz (tabla `UserVoicePreferences`)

```sql
CREATE TABLE UserVoicePreferences (
  id              UUID PRIMARY KEY,
  preference      TEXT NOT NULL,   -- la preferencia (p.ej. "formal")
  filter          TEXT,            -- el FILTRO al que se aplica
  needsUploading  TINYINT(1) NOT NULL DEFAULT 1,
  createdAt       DATETIME NOT NULL,
  updatedAt       DATETIME NOT NULL
);
```

El par **(preferencia, filtro)** es el corazón del sistema: una preferencia con un **filtro** (app, URL, tipo de contenido, voz, etc.). Es un modelo **genérico de reglas** que escala a estilos, snippets, transforms y comandos sin cambiar el esquema. [V] / [I]

Tabla vacía en la instalación auditada ⇒ el usuario no ha definido preferencias manuales de voz. [V]

### 6.5.2 Registro por dictado

`History.personalizationStyleSettings` (JSON) ⇒ **el estilo efectivamente aplicado en cada dictado**, versionado junto al evento. Esto permite: (a) saber por qué un dictado salió de una manera, (b) reproducirlo, (c) medir calidad por estilo. [V]

### 6.5.3 Preferencia por aplicación

`prefs.user.appTranscriptionFormats` = `{}` (vacío) ⇒ **el usuario no ha configurado formatos por app**, pero el mecanismo existe: `defaultTranscriptionFormat = "Apply AI formatting"` es el valor por defecto, y `appTranscriptionFormats` permite sobrescribir por aplicación. [V]

**Valores de `defaultTranscriptionFormat` observados en el sistema:** `"Apply AI formatting"` (por defecto), y en el árbol de accesibilidad aparece **«Transcripción en bruto»** (raw transcription) como alternativa. [V] ⇒ El usuario puede elegir, globalmente o por app, entre **texto con formato IA** y **transcripción literal**.

### 6.5.4 Muestreo y evaluación

- `prefs.user.userJourney.features.personalizedStyles.usageCount = 0` ⇒ el usuario **nunca ha usado la función de estilos personalizados**. [V]
- `polishOnboardingCompleted = false` ⇒ el onboarding de Polish no se completó. [V]
- `userVoicesMigrated = false` ⇒ la migración de las voces antiguas a las 4 `builtIn` **no se ha ejecutado** (el usuario conserva su configuración previa). [V]

---

## 6.6 Señales de producto (flags)

| Flag | Estado | Lectura |
|---|---|---|
| `new-insights-and-voice-profile` | enabled, variante `insights` | Wispr está unificando «perfil de voz» con «nuevos insights» ⇒ el estilo deja de ser un ajuste y pasa a ser un **perfil con métricas**. [I] |
| `styleDetectionEnabled` (pref) | true | Detección automática de estilo |
| `hub-top-apps` | enabled | Apps más usadas (top apps) en el hub |
| `per-app-stats-index` (migración `add-per-app-stats-index`, 2026-03-24) | — | Estadísticas **por aplicación** ⇒ las voces alimentan las estadísticas |

---

## 6.7 Diseño propuesto para Dixi

```sql
CREATE TABLE dixi_styles (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  base_prompt   TEXT,                 -- prompt de sistema para el LLM
  rules         TEXT,                 -- reglas deterministas (JSON)
  cleanup_level TEXT NOT NULL DEFAULT 'light',  -- off|light|medium|heavy
  is_builtin    INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL,
  modified_at   TEXT NOT NULL
);

CREATE TABLE dixi_voice_profiles (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  style_id      TEXT NOT NULL REFERENCES dixi_styles(id),
  cleanup_level TEXT,
  match_rules   TEXT,                -- JSON: {"apps":[...], "urls":[...], "title_regex":[...]}
  is_builtin    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE dixi_profile_overrides (
  id          TEXT PRIMARY KEY,
  profile_id  TEXT NOT NULL,
  scope_kind  TEXT NOT NULL,   -- 'app' | 'url' | 'title' | 'language'
  scope_value TEXT NOT NULL,
  style_id    TEXT NOT NULL,
  cleanup_level TEXT
);
```

**Resolución (orden de precedencia):**

```
1. Override exacto (app+url+título)     → gana
2. Override por app / url
3. Perfil de voz (por coincidencia de app)
4. Perfil 'other' (fallback)
5. Estilo por defecto
```

**Mejoras deliberadas sobre Wispr:**

1. **Estilos deterministas + LLM**: el estilo de Dixi tiene una capa de reglas (mayúsculas, puntuación, listas) que **siempre** se aplica, y el LLM sólo como refinamiento opcional. Así el modo sin-API funciona de verdad.
2. **Estilos importables/exportables** (`.dixi-style` en YAML) — Wispr no lo ofrece.
3. **Match por título de ventana y URL**, no sólo por nombre de app: permite «Slack con el nombre de un cliente» ⇒ estilo formal, «Slack con un canal de memes» ⇒ casual.
4. **Limpieza por idioma** (el español y el inglés tienen muletillas distintas).
5. **Perfil por proyecto/URL** (p. ej. `github.com` ⇒ nunca toques identificadores).

---

## 6.8 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| Perfil de estilo por categoría de app | ✅ 4 voces integradas | ❌ | ⚠️ | ✅ |
| Voces personalizadas | ⚠️ esquema lo permite | ❌ | ❌ | ✅ |
| Estilo por app individual | ✅ (`appTranscriptionFormats`) | ❌ | ❌ | ✅ |
| Estilo por URL / título | ❌ | ❌ | ❌ | ✅ **diferencial** |
| Niveles de limpieza | ✅ (`light` observed) | ❌ | ❌ | ✅ 4 niveles |
| Detección automática de estilo | ✅ | ❌ | ❌ | ✅ |
| Muestreo de estilo ("probar con una muestra") | ✅ | ❌ | ❌ | ✅ |
| Estilo sin LLM (determinista) | ❌ | ❌ | ❌ | ✅ **diferencial** |
| Estadísticas por app | ✅ | ❌ | ⚠️ | ✅ |
| Importar/exportar estilos | ❌ | ❌ | ❌ | ✅ **diferencial** |

---

### Referencias cruzadas

- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — `personalizationStyleSettings` por dictado
- [07-TRANSFORMS.md](07-TRANSFORMS.md) — LLM y limpieza
- [09-STATS.md](09-STATS.md) — métricas por app
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — inventario de preferencias
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
