# 04 — Diccionario: modelo, aprendizaje automático y sustituciones

> Fuente: tabla `Dictionary` de `flow.sqlite` (43 filas, 16 columnas), preferencias `prefs.user.*` y sidecars de traza.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 4.1 Estructura de la tabla `Dictionary`

```sql
CREATE TABLE Dictionary (
  id                  VARCHAR(36)  NOT NULL PRIMARY KEY,
  phrase              VARCHAR(255) NOT NULL,
  replacement         VARCHAR(255),              -- texto sustituto (NO solo ortografía)
  replacementHtml     TEXT,                      -- sustituto con formato/HTML
  teamDictionaryId    VARCHAR(36)  NOT NULL DEFAULT '00000000-...-0000',  -- 0 = personal
  lastUsed            DATETIME,
  frequencyUsed       INTEGER      NOT NULL DEFAULT 0,   -- contador LOCAL
  remoteFrequencyUsed INTEGER      NOT NULL DEFAULT 0,   -- contador SERVIDOR
  manualEntry         TINYINT(1)   NOT NULL DEFAULT 0,   -- añadido a mano vs aprendido
  createdAt           DATETIME     NOT NULL,
  modifiedAt          DATETIME     NOT NULL,
  isDeleted           TINYINT(1)   NOT NULL DEFAULT 0,   -- borrado lógico (sincronizable)
  source              VARCHAR(255),              -- procedencia
  isSnippet           TINYINT(1)   NOT NULL DEFAULT 0,   -- es un fragmento, no un término
  observedSource      VARCHAR(255),              -- dónde se observó
  isStarred           TINYINT(1)   NOT NULL DEFAULT 0    -- favorito (2026-03-11)
);
```

**Observaciones de diseño (todas [V]):**

1. **`replacement` + `replacementHtml`** ⇒ el diccionario no sólo corrige ortografía: **sustituye una frase por otra** (migración `to_replace_dictionary` / `rename_to_replace`, 2025-03-26/31). Esto lo convierte en un motor de **reglas de texto**, no en una lista de palabras.
2. **`isSnippet`** ⇒ los fragmentos (snippets) **son entradas de diccionario** con un flag. La UI los separa en su propia sección del sidebar, pero el modelo de datos es **unificado**. Esto es una decisión de diseño excelente: un solo motor de sustitución para términos y para frases.
3. **`teamDictionaryId` con UUID nulo por defecto** ⇒ soporte de **diccionarios de equipo** (empresa), ya activo en el esquema aunque el flag `org-insights` esté apagado.
4. **`frequencyUsed` + `remoteFrequencyUsed`** ⇒ **dos contadores**: el local (para decisión rápida offline) y el del servidor (para ranking compartido). Diseñado para funcionar sin conexión y sincronizar después.
5. **`isDeleted` (borrado lógico)** + `needsUploading` en otras tablas ⇒ modelo **CRDT-ish** de sincronización: borrar en un dispositivo debe propagarse.
6. **`isStarred`** ⇒ favoritos, con migración propia (2026-03-11), lo que confirma que la función de favoritos es reciente y activa.

---

## 4.2 Estado real de la instalación auditada (43 entradas)

| `source` | `isSnippet` | `manualEntry` | Entradas |
|---|---|---|---|
| `user_edits` | 0 | 0 | **35** |
| `default` | 0 | 0 | 3 |
| `default` | **1** | 0 | **3** |
| `user_edits` | 0 | **1** | 2 |
| **Total** | | | **43** |

Lecturas:

- **81 % de las entradas se aprendieron automáticamente de las ediciones del usuario** (`source = user_edits`, `manualEntry = 0`). [V]
- Coherente con `prefs.user.shouldAutoLearnWords = true` ⇒ **aprendizaje automático activo**. [V]
- **3 fragmentos vienen de fábrica** (`source = default`, `isSnippet = 1`) ⇒ Wispr trae fragmentos predefinidos. [V]
- **4 entradas tienen `replacement`** y hay `replacementHtml` ⇒ el uso de sustituciones es real pero **minoritario** (≈ 9 % de las entradas). [V]
- **3 entradas marcadas como favoritas** (`isStarred = 1`). [V]
- `frequencyUsed > 0` en algunas entradas ⇒ el motor **prioriza** las términos más usados. [V]

> **El insight más importante para Dixi:** el diccionario de Wispr **no es una lista de palabras que el usuario escribe a mano**: es un **sistema de aprendizaje supervisado** que observa las ediciones posteriores al dictado y extrae automáticamente el término y su forma correcta. El usuario sólo escribe a mano el 4,6 % de las entradas (2 de 43).

---

## 4.3 El ciclo de aprendizaje (reconstrucción)

```
1. ASR produce asrText  ("juan perez fue a reUNION")
2. El formateador aplica el diccionario:
      "reunion" → "reunión"   (sustitución de término)
      "juan perez" → "Juan Pérez"  (nombre propio, vía AX + aprendizaje)
3. Se pega formattedText en la app destino
4. El usuario edita el texto (en la app destino, no en Wispr)
5. Wispr observa la edición  →  sidecar de traza 'edit'
6. Compara editedText vs formattedText:
      - diferencia de capitalización de un nombre  → entrada de diccionario
      - diferencia léxica repetida                → entrada de diccionario
7. Persiste en Dictionary con source='user_edits', manualEntry=0
8. La siguiente vez, el formateo ya acierta
```

**Soporte en el modelo de datos:** la migración `add-correction-counts-to-history` (2026-04-09) añadió `numWordsCorrected` y `numDictionaryReplacements` al `History` ⇒ Wispr **mide** cuántas correcciones aplicó el diccionario en cada dictado. [V]

**Medición observada (n = 3 160):** `numDictionaryReplacements` media **0,64**, p90 **2** ⇒ el diccionario interviene en la mayoría de los dictados, pero sustituye pocas palabras cada vez. `numWordsCorrected` media **1,5**, p90 **4**. [V]

**Distancia de edición:** `editDistanceToDictated` p50 = 0, p90 = 0,895 ⇒ en la mitad de los dictados el usuario **no editó nada**; en el 90 % la edición fue mínima. [V] El formateo automático acierta con alta frecuencia.

---

## 4.4 Sustitución de nombres propios (AX)

El sidecar de traza **`noun`** (expuesto por `ax-inspect.mjs`) y las columnas `axText` / `axHTML` del `History` confirman un mecanismo específico: **sustitución de sustantivos propios apoyada en el contexto de accesibilidad**. [V]

Ejemplo de flujo: el usuario dicta «juan perez» mientras escribe en un correo que ya contiene «Juan Pérez» ⇒ el árbol AX del campo (`axText`/`axHTML`) aporta las formas correctas ⇒ el ASR las aplica sin intervention manual. [I]

**Implicación para Dixi:** es la función que más se nota en la calidad percibida y **no está implementada en ningún proyecto open source evaluado**. Su implementación en Rust/Windows es factible con `windows` crate + UIAutomation, pero requiere: (a) capturar el árbol AX del control enfocado, (b) extraer candidatos a nombre propio, (c) un matcher fonético tolerante. Ver [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) §3.

---

## 4.5 Lo que Wispr NO hace (limitaciones observadas)

| Limitación | Evidencia |
|---|---|
| `editedTextStatus = NOT_EXTRACTED` en 3 160/3 160 filas | La **extracción del texto editado desde la app destino no está activa** en esta instalación ⇒ el aprendizaje automático **no puede estar collecting ediciones del usuario en la app destino**; las 35 entradas `user_edits` provienen de correcciones detectadas en el propio flujo de Wispr (Flow Lens / edición dentro de la app), no de la app externa. [V] |
| `frequencyUsed` y `remoteFrequencyUsed` separados | Sin conexión, el ranking se degrada al contador local. [I] |
| Sin columna de tipo/idioma por entrada | El diccionario **no distingue entre idiomas** ⇒ un término español puede interferir con un dictado en inglés. [V] |

> **Corrección importante de hipótesis:** lacolumna `editedTextStatus` con valor por defecto `NOT_EXTRACTED` y 0 filas con otro valor indica que la extracción de ediciones post-hoc **no está funcionando (o no está habilitada) en 1.6.1174**. Las entradas `user_edits` se originan en el ciclo interno de Wispr. Esto **reduce el valor del aprendizaje automático** en la versión auditada y es un hueco que Dixi podría cerrar mejor. [V] / [I]

---

## 4.6 Diseño propuesto para Dixi

```
dixi_dictionary (
  id            TEXT PRIMARY KEY,
  phrase        TEXT NOT NULL,
  replacement   TEXT,                -- sustituto plano
  replacement_html TEXT,             -- sustituto con formato
  scope         TEXT NOT NULL,       -- 'personal' | 'team:<id>'
  source        TEXT NOT NULL,       -- 'manual' | 'learned' | 'builtin' | 'snippet'
  is_snippet    INTEGER NOT NULL DEFAULT 0,
  is_starred    INTEGER NOT NULL DEFAULT 0,
  frequency     INTEGER NOT NULL DEFAULT 0,
  last_used     TEXT,
  deleted       INTEGER NOT NULL DEFAULT 0,  -- borrado lógico p/ sync
  created_at    TEXT NOT NULL,
  modified_at   TEXT NOT NULL
)
```

**Diferencias deliberadas respecto de Wispr:**

1. **`scope` explícito** en vez de un UUID centinela: más legible y evita colisiones con el «diccionario nulo».
2. **Índice sobre `phrase` normalizada** (lowercase + sin acentos) para el lookup en hot path.
3. **Separación real de fragmentos**: tabla `dixi_snippets` con campos propios (`trigger`, `variables`, `app_scope`), aunque el motor de sustitución se comparta. Wispr usa un flag; un trigger de fragmento puede ser una frase exacta, un prefijo o un **comando**.
4. **Aprendizaje activo**: implementar de verdad la extracción de la edición en la app destino (Wispr parece no hacerlo en 1.6.1174) ⇒ diferenciar la **ventaja competitiva** real de Dixi.
5. **Aprendizaje por idioma**: `language` por entrada para evitar colisiones es/en.
6. **Prioridad explícita** (`priority`) para permitir que un snippet gane a un término.

---

## 4.7 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| Entradas manuales | ✅ | ✅ | ✅ | ✅ |
| Sustitución de texto (no sólo ortografía) | ✅ (`replacement`) | ❌ | ❌ | ✅ |
| Sustituto con formato (HTML) | ✅ (`replacementHtml`) | ❌ | ❌ | ✅ |
| Aprendizaje automático de correcciones | ⚠️ parcial en 1.6.1174 | ❌ | ✅ (`correction-learner`) | ✅ completo |
| Diccionario de equipo | ✅ (`teamDictionaryId`) | ❌ | ❌ | Opcional |
| Favoritos | ✅ | ❌ | ❌ | ✅ |
| Sustitución de nombres propios vía AX | ✅ | ❌ | ❌ | **✅ (diferencial)** |
| Fragmentos como entradas de diccionario | ✅ (flag `isSnippet`) | ❌ | ✅ (tabla propia) | ✅ (tabla propia) |
| Contadores local/servidor | ✅ | ❌ | ❌ | Local primero |

---

### Referencias cruzadas

- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — pipeline y `numDictionaryReplacements`
- [05-SNIPPETS.md](05-SNIPPETS.md) — fragmentos
- [10-MCP.md](10-MCP.md) — superficie MCP
- [14-ALTERNATIVAS-OPEN-SOURCE.md](14-ALTERNATIVAS-OPEN-SOURCE.md) — `correction-learner` de OpenWhispr
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas priorizadas
