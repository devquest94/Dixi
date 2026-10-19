# 07 — Transforms (Polish): el motor de transformación con IA

> Fuente: tabla `Polish` (20 columnas), `prefs.user.polish*`, atajos `polish` / `polish_prompt_1` / `view_diff`, tabla `InstructHistory`, tabla `UserContext`, flag `auto-polish-on-short-text`.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 7.1 Qué es «Transforms»

**Transforms** (internamente **Polish**) es la función de **transformación de texto mediante IA**: toma el texto ya transcrito y lo modifica según una instrucción («hazlo más corto», «traducí», «convertilo en una lista», «resumí»). Es una **segunda pasada de LLM**, distinta del formateo.

**Evidencia de superficie:**
- Sección propia del sidebar: **«Transforms»** (`focus-overlay-target-sidebar-transforms`), icono ✎ (lápiz). [V]
- Tabla `Polish` con 20 columnas. [V]
- Atajos: `Alt+1+`` = `polish`, `Alt+2+`` = `polish_prompt_1`, `Alt+P+`` = `view_diff`. [V]
- `prefs.user.polishEnabled = true`, `polishAutoPaste = true`. [V]
- Migración `create_polish_table` (2026-01-06) + 5 migraciones de ampliación hasta 2026-03-10. [V]

---

## 7.2 El modelo de datos de `Polish` (20 columnas)

```sql
CREATE TABLE Polish (
  id                    UUID PRIMARY KEY,
  polishInitialText     TEXT,        -- texto de entrada
  polishedText          TEXT,        -- texto de salida
  polishInitialWordCount INTEGER,    -- palabras antes
  polishedWordCount     INTEGER,     -- palabras después
  app                   VARCHAR(255),-- app donde se ejecutó
  processingTime        FLOAT,       -- segundos
  status                TEXT,        -- pending | success | error
  polishUndone          TINYINT(1) NOT NULL DEFAULT 0,   -- el usuario lo revirtió
  instruction           TEXT,        -- la instrucción dada
  needsUploading        TINYINT(1) NOT NULL DEFAULT 1,
  createdAt             DATETIME NOT NULL,
  updatedAt             DATETIME NOT NULL,
  modelVersion          VARCHAR(50), -- versión del modelo usado
  diffCount             INTEGER,     -- nº de cambios
  feedback              TEXT,        -- feedback del usuario
  usedProvider          TEXT,        -- PROVEEDOR usado (Claude/GPT/…)
  promptName            VARCHAR(255),-- nombre del prompt (si es un prompt guardado)
  shortcutKey           VARCHAR(255),-- atajo asociado
  instructHistoryId     VARCHAR(36)  -- enlace al historial de instrucciones
);
```

**Análisis de diseño [V] / [I]:**

1. **`usedProvider` + `modelVersion`** ⇒ Wispr **abstrae el proveedor**: registra qué modelo concreto produjo cada resultado. Esto permite cambiar de proveedor sin migración y auditar calidad por modelo. **Es exactamente la abstracción que Dixi necesita** (ver [15-INTEGRACION-MINIMAX.md](15-INTEGRACION-MINIMAX.md)).
2. **`diffCount` + `polishUndone` + `view_diff`** ⇒ Wispr **muestra el diff** entre original y resultado, y puede **deshacer** la transformación. Sin esto, un LLM que reescribe mal destruye el trabajo del usuario.
3. **`promptName` + `shortcutKey`** ⇒ los prompts guardados son **primeros Citizen**: tienen nombre y atajo. `Alt+2+`` = `polish_prompt_1` ⇒ es el 2.º prompt del usuario, ranurado por posición.
4. **`instructHistoryId`** ⇒ enlaza con `InstructHistory` ⇒ el flujo de Transform y el de instrucciones conversacionales comparten historial.
5. **`diffCount` como métrica de calidad** ⇒ se puede medir «cuánto modifica el modelo» y correlacionarlo con `feedback` y `polishUndone`.
6. **`status`** ⇒ el procesamiento es **asíncrono y persistido** (puede fallar y reintentarse; `needsUploading` indica sincronización pendiente).

---

## 7.3 La superficie de instrucciones conversacional

| Elemento | Evidencia |
|---|---|
| `InstructHistory` | tabla con rutas y payloads de herramientas; migración `instruct-history-tool-calls` (2026-06-12) ⇒ **el historial de instrucciones guarda llamadas a herramientas** ⇒ es un **agente**, no un simple prompt |
| `InstructChatSession` | tabla de sesiones de chat de instrucciones |
| `UserContext` | `writingSamples` (TEXT), `polishPrompts` (TEXT), `modifiedAt` ⇒ **el usuario puede aportar sus propias muestras de escritura y prompts** |
| `prefs.user.instructInternalDataSharingEnabled = true` |Wispr comparte datos internos con el servicio de instrucciones (opt-out) |
| `prefs.user.commandModeEnabled = false` | **Command Mode** existe y está apagado |
| `prefs.user.pressEnterCommandEnabled = false` | modo «Enter para comandar» existe y está apagado |
| `prefs.user.pressEnterCommandPromptShown = false` | el prompt de descubrimiento no se mostró |
| `autoPolishAfterDictation = {"active": false, "promptName": "polish"}` | Polish automático tras dictar, con un prompt asociado |

> **Conclusión de alto valor [I]:** Wispr **no tiene un «Transforms» de una sola pasada**; tiene un **motor de instrucciones con llamada a herramientas** (`InstructHistory.routesPayload`, `instruct-history-tool-calls`) que es la base de un **agente**. Los «Transforms» de la UI son un caso particular: un prompt predefinido aplicado al texto actual.

---

## 7.4 Automatización del pipeline

| Preferencia | Valor | Significado |
|---|---|---|
| `aiFormatting` | `true` | Formateo por IA activado (global) |
| `polishEnabled` | `true` | Transforms habilitados |
| `polishAutoPaste` | `true` | El resultado del Transform **se pega automáticamente** |
| `autoPolishAfterDictation.active` | `false` | (desactivado) aplicar Polish a cada dictado |
| `autoPolishAfterDictation.promptName` | `"polish"` | el prompt que se usaría |
| `defaultTranscriptionFormat` | `"Apply AI formatting"` | el formato por defecto incluye IA |
| `stashedPolishShortcuts` | `{}` | atajos de Polish en espera (migración de un esquema anterior) |
| `polishUseCount` | 0 | nunca usado |
| `polishButtonClickCount` | 0 | nunca se hizo clic en el botón |
| `polishButtonTrackingStartedAt` | 1786037505268 |epoch ms ⇒ **instrumentación de telemetría de botones activa desde 2026-06** |

---

## 7.5 Catálogo real de comandos (VERIFICADO contra la documentación pública)

> **Corrección importante:** una primera hipótesis de esta auditoría asumía un catálogo amplio (resumir/acortar/expandir/traducir/viñetas/tabla…). La documentación pública de Wispr Flow muestra un catálogo **mucho más pequeño**. [V] (ver `research-wispr-docs.md`)

### 7.5.1 Los 2 transforms integrados

| Transform | Qué es | Reglas |
|---|---|---|
| **Polish** | Reescritura del texto con un dial de reglas | **5 conmutables**: *Make more concise* · *Reword for clarity* · *Reorder for readability* · *Add structure for readability* · *Maintain your tone*<br>**+2 reglas Pro apagadas por defecto**: *Clarify main point* · *Refine phrasing for impact* |
| **Prompt Engineer** | Reescritura a partir de una instrucción libre del usuario | — |

### 7.5.2 Las 4 opciones de «re-transform»

Sobre un texto ya procesado, Wispr ofrece 4 re-transformaciones: **Prompt Engineer** · **Turn to list** · **Translate to English** · **Empathize**. [V]

### 7.5.3 Límites y estructura

| Parámetro | Valor | Confianza |
|---|---|---|
| **Slots totales de Transforms** | **10** (2 integrados + **8 personalizados**) | [V] |
| Instrucciones personalizadas por usuario | **5** | [V] |
| Longitud de una instrucción personalizada | **50 palabras** | [V] |
| Muestras por transform | **5**, de **50–500 palabras** cada una | [V] |
| Longitud del texto a transformar | **1–3 000 palabras** (desktop) / 10–3 000 (iOS) | [V] |
| Deshacer | «Undo» en el banner de la Flow Bar + «Undo AI edit» en el Hub | [V] |
| Auto Cleanup (dial **separado**) | **None / Light / Medium** (se eliminó «High») | [V] |

> **[V] `polishInstructions.default = {}` en la instalación auditada ⇒ los transforms integrados NO se almacenan localmente**: se resuelven en el servidor (o en el bundle). Por eso el catálogo de 2 Transform + 4 re-transform **no es observable en la base de datos local**. [I]

### 7.5.4 Qué NO existe (verificado en contra)

**Summarize, rewrite, shorten, expand, bulletize, table y explain NO están documentados como transforms integrados.** [V] La tarea de «resumir» o «convertir en lista» se cubre, en la práctica, con `Turn to list` y con el prompt libre de **Prompt Engineer**.

> **Lectura de producto [I]:** Wispr **delega la composición de comandos al usuario** en vez de ofrecer una biblioteca de comandos. Es la estrategia opuesta a la que asumía esta auditoría, y probablemente la razón por la que el catálogo **público** no crece: cada comando nuevo es superficie de soporte.
>
> **Para Dixi, esto abre una oportunidad real:** un catálogo curado de 13 transforms (resumir, acortar, expandir, formalizar, informal, puntuar, viñetas, tabla, traducir, sin_jerga, explicar, preguntas, accionables) cubre el caso de uso de Prompt Engineer **con un clic**, y es exactamente la clase de feature que un usuario de Wispr no tiene y echa de menos. Ver §7.6.2.

### 7.5.5 Estado de la documentación vs. la app instalada

> **Hallazgo de deriva (drift) [V]:** el sidebar **público** de Wispr Flow **no tiene una entrada «Transforms»**, pero la instalación auditada (1.6.1174) **sí** la tiene, con icono propio ✎. ⇒ La función existe en la app y **aún no está documentada**, o está en rollout progresivo.
>
> El mismo patrón aparece con el **flag `auto-polish-on-short-text`** (selección de texto) y con el **modelo ASR «Canto»** (Sept 2026): la app va por delante de la documentación pública. [I]

---

## 7.6 Diseño propuesto para Dixi

### 7.6.1 Abstracción de proveedor (requisito previo)

```rust
pub enum ProviderKind { MiniMax, OpenAiCompat, AnthropicCompat, LocalLlama, None }

#[async_trait]
pub trait TransformProvider {
    async fn transform(
        &self,
        req: &TransformRequest,
        ctx: &TransformContext,
    ) -> Result<TransformResult, TransformError>;

    fn kind(&self) -> ProviderKind;
    fn model(&self) -> &str;
}

pub struct TransformRequest {
    pub text: String,
    pub instruction: String,
    pub style: Option<&Style>,
    pub language: Language,
    pub max_output_tokens: u32,
}

pub struct TransformResult {
    pub text: String,
    pub model: String,          // p. ej. "MiniMax-M2.1"
    pub provider: ProviderKind,
    pub processing_ms: u64,
    pub diff: Option<Diff>,
}
```

**Contrato de calidad (el que Wispr no tiene):** `TransformResult` **debe** incluir un diff. Un LLM que reescribe sin diff es un **riesgo de pérdida de datos**. [I]

### 7.6.2 Catálogo de transforms

```sql
CREATE TABLE dixi_transforms (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  instruction   TEXT NOT NULL,        -- prompt
  shortcut_key  TEXT,                 -- opcional
  is_builtin    INTEGER NOT NULL DEFAULT 0,
  style_id      TEXT,                 -- estilo a aplicar
  provider_hint TEXT,                 --ProviderKind preferido
  created_at    TEXT NOT NULL,
  modified_at   TEXT NOT NULL
);
```

**Transforms incorporados (propuesta, cubriendo el caso de uso más frecuente):**

| Nombre | Instrucción (es) | Ejemplo |
|---|---|---|
| `resumir` | «Resume en una frase, conservando los datos clave» | 5 párrafos → 1 frase |
| `acortar` | «Reduce un 50 % manteniendo todas las cifras y nombres» | correo largo → versión corta |
| `expandir` | «Desarrolla con un ejemplo y el contexto necesario» | 1 frase → párrafo |
| `formalizar` | «Reescribe en registro formal» | WhatsApp → correo |
| `informal` | «Reescribe en registro casual» | correo → chat |
| `puntuar` | «Corrige la puntuación y las mayúsculas» | — |
| `viñetas` | «Convierte en una lista con viñetas» | párrafo → lista |
| `tabla` | «Convierte en una tabla markdown» | datos → tabla |
| `traducir` | «Traduce a {idioma}» | es→en |
| `sin_jerga` | «Elimina relleno y muletillas» | «eh, o sea» → limpio |
| `explicar` | «Explica para alguien no técnico» | jerga → claro |
| `preguntas` | «Genera 3 preguntas de seguimiento» | correo → agenda |
| `accionables` | «Extrae las tareas con responsable y fecha» | reunión → TODOs |

**Diferencial de Dixi: transforms locales.** Con `llama.cpp` + un modelo 3B–7B instruction-tuned, 11 de los 13 transforms de arriba funcionan **sin red** (los que son reescritura de estilo, no resumen semántico profundo). Esto elimina la dependencia de MiniMax para el 80 % del uso.

### 7.6.3 Diff y deshacer

```sql
CREATE TABLE dixi_transform_runs (
  id            TEXT PRIMARY KEY,
  transform_id  TEXT REFERENCES dixi_transforms(id),
  input_text    TEXT NOT NULL,
  output_text   TEXT NOT NULL,
  diff_json     TEXT,            -- diff estructurado (ops line-level)
  diff_count    INTEGER NOT NULL DEFAULT 0,
  model         TEXT NOT NULL,
  provider      TEXT NOT NULL,
  processing_ms INTEGER,
  undone        INTEGER NOT NULL DEFAULT 0,
  feedback      TEXT,
  created_at    TEXT NOT NULL
);
```

**UI mínima obligatoria:** antes de pegar el resultado, mostrar el diff con un botón **«Deshacer»** persistido. Sin esto, un provider malo destruye trabajo.

---

## 7.7 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| Transform de texto por IA | ✅ (Polish) | ⚠️ BYO-key LLM | ⚠️ | ✅ |
| Catálogo de transforms predefinidos | ✅ (catálogo **no verificado localmente**) | ❌ | ❌ | ✅ 13 propuestos |
| Prompts personalizados con nombre | ✅ (`promptName`) | ❌ | ❌ | ✅ |
| Prompt personalizado con atajo | ✅ (`shortcutKey`, `polish_prompt_1`) | ❌ | ❌ | ✅ |
| Diff visible antes de aplicar | ✅ (`view_diff`, `diffCount`) | ❌ | ❌ | ✅ **obligatorio** |
| Deshacer | ✅ (`polishUndone`) | ❌ | ❌ | ✅ |
| Abstracción de proveedor | ✅ (`usedProvider`, `modelVersion`) | ⚠️ | ⚠️ | ✅ |
| **Transforms 100 % locales** | ❌ | ⚠️ | ❌ | ✅ **diferencial** |
| Aplicar automáticamente tras dictar | ✅ (`autoPolishAfterDictation`) | ❌ | ❌ | ✅ |
| Transform sobre texto seleccionado | ✅ (flag `auto-polish-on-short-text`) | ❌ | ❌ | ✅ |
| Agente con llamadas a herramientas | ✅ (`instruct-history-tool-calls`) | ❌ | ❌ | Fase 3 |
| Muestras de escritura del usuario | ✅ (`UserContext.writingSamples`) | ❌ | ❌ | ✅ |

---

## 7.8 Brecha priorizada

| # | Brecha | Impacto | Dificultad |
|---|---|---|---|
| 1 | Diff + deshacer **obligatorios** en todo provider | **Crítico** (pérdida de datos) | Baja |
| 2 | Abstracción de provider con `model` + `provider` registrados | Alto | Baja |
| 3 | Catálogo de transforms locales | Alto | Media (prompt + catálogo) |
| 4 | Transform por selección (no sólo documento completo) | Medio | Baja |
| 5 | Feedback por transform (como `Polish.feedback`) | Medio | Baja |
| 6 | Agente con herramientas | Alto | Alta (Fase 3) |

---

### Referencias cruzadas

- [06-ESTILOS.md](06-ESTILOS.md) — estilos aplicados por el formateo
- [10-MCP.md](10-MCP.md) — superficie de herramientas
- [15-INTEGRACION-MINIMAX.md](15-INTEGRACION-MINIMAX.md) — abstracción de provider
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
