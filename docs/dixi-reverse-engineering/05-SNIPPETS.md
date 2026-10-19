# 05 — Fragmentos (Snippets)

> Fuente: flag `isSnippet` de la tabla `Dictionary`, sección «Fragmentos» del sidebar (`focus-overlay-target-sidebar-snippets`), flag de servidor `snippets` (migración `snippets`, 2025-08-05) y preferencias de la aplicación.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 5.1 Qué es un fragmento en Wispr Flow

Un **fragmento** (*snippet*) es una frase almacenada que se **inserta expandida** al detectarla durante el dictado. En el modelo de datos de Wispr **no es una entidad nueva**: es una entrada de `Dictionary` con `isSnippet = 1`. [V]

```sql
-- entrada de diccionario marcada como fragmento
phrase            = 'mi firma'          -- el disparador
replacement       = 'Juan Pérez\nIngeniero de Software\n+54 11 1234-5678'  -- la expansión
replacementHtml   = '<p>…</p>'          -- versión con formato
isSnippet         = 1
source            = 'default'          -- de fábrica
frequencyUsed     = 0
```

**Ventajas del diseño unificado [I]:**

- Un solo motor de sustitución (búsqueda de frase → sustitución) sirve para términos y fragmentos.
- El fragmento hereda todo el aparato del diccionario: favoritos, contadores de uso, sincronización, borrado lógico, ámbito de equipo.
- El usuario ve los fragmentos en una **sección propia del sidebar**, pero el almacenamiento no se duplica.

---

## 5.2 Evidencia observada

| Evidencia | Valor | Fuente |
|---|---|---|
| Entradas con `isSnippet = 1` | **3** (las 3 con `source = 'default'`) | `Dictionary` [V] |
| Fragmentos de usuario (`isSnippet=1` + `source='user_edits'` o `manualEntry=1`) | **0** | `Dictionary` [V] |
| Sección en el sidebar | «Fragmentos», `focus-overlay-target-sidebar-snippets` | UIA [V] |
| Icono propio | ✂ ( tijeras ) | UIA/DOM [V] |
| Migración | `snippets` (2025-08-05) | `resources/migrations/` [V] |
| Migración relacionada | `unique-phrase` (2025-08-07) | [V] |
| Flag de servidor | `snippets` — no listado como `false` en el payload ⇒ **activo** | `feature-flags-cache.json` [V] |

> **Interpretación [I]:** los 3 fragmentos de fábrica son frases de **firma/identidad** (el caso de uso más común: firma de correo). El usuario auditado no creó fragmentos propios ⇒ el caso de uso dominante de Wispr es «dictar el correo y que salga mi firma», no «expandir plantillas de texto».

**`unique-phrase` (2025-08-07)** sugiere que Wispr tiene un concepto de **frase única** — posiblemente frases que sólo se expanden en un contexto determinado o que se deduplican para no repetirse. [I] / [NV] sobre su semántica exacta.

---

## 5.3 Mecánica de expansión (reconstrucción)

Con base en el modelo de datos, el sidecar de traza `noun` y la sección de UI:

```
1. El ASR transcribe el audio
2. El formateador busca en el diccionario (términos y fragmentos) coincidencias
3. Si hay coincidencia de un fragmento:
     - se inserta replacementHtml (si existe) o replacement
4. Se registra en History:
     numDictionaryReplacements  += 1
5. Sidecar de traza: 'noun' (si fue un sustantivo) y/o traza de expansión
```

**Variables**: Wispr **no** tiene una columna de variables en `Dictionary` (`replacement` es texto plano). Por tanto, **los fragmentos de Wispr son estáticos**: no soportan parámetros tipo `{nombre}`. [V para la ausencia del campo; I para la consecuencia]

> ✅ **CONFIRMADO contra la documentación pública [V]:** la documentación de Snippets de Wispr confirma explícitamente que **no hay variables dinámicas** (ni siquiera insertar la fecha actual). Los límites publicados son:
>
> | Parámetro | Límite |
> |---|---|
> | Longitud del **disparador** | **60 caracteres** |
> | Longitud de la **expansión** | **4 000 caracteres** (desktop) |
> | **Variables dinámicas** | **No soportadas** (ni fechas) |
>
> Lo que sí ofrece Wispr es un disparador **por texto exacto**: escribís la frase clave y se expande. El caso de uso real es la **firma**, no las plantillas parametrizadas.
>
> **Contraste con el diccionario:** el campo `phrase` de `Dictionary` es `VARCHAR(255)` pero la UI pública limita a **60 caracteres** ⇒ el límite real es de producto, no de esquema. [V]

> **Implicación para Dixi:** los fragmentos con **variables** (p. ej. `Hola {nombre}, te escribo sobre {asunto}`) son una **mejora directa** que Wispr no ofrece y que se puede implementar con un parser de plantillas trivial (`{campo}`) sobre el mismo motor de sustitución.

---

## 5.4 Cómo se invocan

**No verificado directamente.** La evidencia disponible:

- El sidebar tiene su propia sección de Fragmentos, lo que implica **gestión** (crear/editar/borrar) más que invocación.
- La tabla `Polish` tiene una columna `promptName` y `shortcutKey` ⇒ **para los Transforms**, Wispr sí soporta **atajos por prompt**. [V] Por analogía, es probable que los fragmentos también puedan asignarse a un atajo, pero **[NV]**.
- Los fragmentos de fábrica (`source='default'`) son 3 ⇒ Wispr incluye un **catálogo de fragmentos precargados** expandible desde la app.

---

## 5.5 Diseño propuesto para Dixi

```sql
CREATE TABLE dixi_snippets (
  id            TEXT PRIMARY KEY,
  trigger_phrase TEXT NOT NULL,          -- disparador
  expansion     TEXT NOT NULL,          -- texto expandido
  expansion_html TEXT,                  -- variante con formato
  variables     TEXT,                   -- JSON: [{"name":"nombre","required":true,"prompt":"¿Nombre?"}]
  scope         TEXT NOT NULL DEFAULT 'personal',
  language      TEXT,                   -- evita colisiones es/en
  app_scope     TEXT,                   -- JSON: ["slack","gmail"]
  is_builtin    INTEGER NOT NULL DEFAULT 0,
  is_starred    INTEGER NOT NULL DEFAULT 0,
  frequency     INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL,
  modified_at   TEXT NOT NULL
);
```

**Motor de expansión con variables (diseño):**

```rust
pub fn expand(snippet: &Snippet, ctx: &DictationContext) -> Expansion {
    let mut out = snippet.expansion.clone();
    for var in snippet.variables() {
        let value = match ctx.resolve(&var.name) {
            Some(v) => v,
            None if var.required => {
                // Si la variable requerida no está en el contexto,
                // Dixi la pide por voz antes de pegar.
                return Expansion::NeedsInput(var.clone());
            }
            None => String::new(),
        };
        out = out.replace(&format!("{{{}}}", var.name), &value);
    }
    Expansion::Text(out)
}
```

**Variables resolubles sin intervención:**

| Fuente | Ejemplo |
|---|---|
| Contexto AX del campo | nombre del destinatario en un correo |
| App + URL detectadas | `chrome`, `gmail.com` |
| Hora/fecha | «hoy», «mañana» |
| Texto previo del campo | párrafos anteriores |
| **Voz del usuario** | «Dixi, inserta la firma» |

**Puntos de diseño a favor de Dixi:**

1. **Variables con prompt por voz**: «inserta la firma de Mario» →Dxipide el nombre por voz.
2. **Fragmentos por aplicación**: el mismo disparador puede expandir distinto texto en Gmail y en Slack.
3. **Fragmentos encadenados**: la expansión puede invocar a otra expansión (con límite de profundidad para evitar ciclos).
4. **Fragmentos por comando**: `!firma`, `!firma:carlos` — trigger por prefijo, sin ambigüedad con el texto normal.

---

## 5.6 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| Fragmentos (snippets) | ✅ (como entradas de diccionario) | ❌ | ✅ (tabla propia) | ✅ |
| Fragmentos de fábrica | ✅ (3 observados) | ❌ | ❌ | ✅ (catálogo ampliable) |
| Fragmentos con variables | ❌ (no hay campo) | ❌ | ❌ | ✅ **diferencial** |
| Fragmentos por aplicación | ❌ | ❌ | ❌ | ✅ **diferencial** |
| Trigger por comando (`!x`) | ❌ | ❌ | ❌ | ✅ **diferencial** |
| Fragmento = entrada de diccionario | ✅ | n/a | ❌ | Opcional (tabla propia) |
| Favoritos | ✅ | ❌ | ❌ | ✅ |
| Contador de uso | ✅ | ❌ | ❌ | ✅ |

---

## 5.7 Brecha priorizada

| # | Brecha | Impacto | Dificultad Dixi |
|---|---|---|---|
| 1 | Snippets con variables | **Alto** — es la función que más se echa de menos en el flujo de correo | Baja (parser `{campo}`) |
| 2 | Snippets por aplicación | Medio-alto | Baja |
| 3 | Trigger por comando | Medio | Baja |
| 4 | Catálogo de fragmentos de fábrica ampliable | Medio | Media (contenido) |

---

### Referencias cruzadas

- [04-DICCIONARIO.md](04-DICCIONARIO.md) — motor de sustitución compartido
- [07-TRANSFORMS.md](07-TRANSFORMS.md) —_transforms y prompts personalizados_
- [12-UX-UI.md](12-UX-UI.md) — sección Fragmentos del sidebar
- [14-ALTERNATIVAS-OPEN-SOURCE.md](14-ALTERNATIVAS-OPEN-SOURCE.md) — snippets de OpenWhispr
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
