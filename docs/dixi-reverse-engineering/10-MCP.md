# 10 — MCP (Model Context Protocol) y superficie de herramientas

> Fuente: flag `ask-anything-mcp-hint`, flag `connectors`, checklist «Configurar MCP» del hub, `prefs.user.notetakerMCPVisibilityState`, `prefs.user.notetakerMcpCopyPromoSeen`, `prefs.user.notetakerChecklistMcp*`, tabla `InstructHistory` (con `routesPayload` y tool calls), tablas `NotetakerChats`/`NotetakerChatAgentStates`, `prefs.user.commandModeEnabled`, `prefs.user.pressEnterCommandEnabled`.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 10.1 Estado de la superficie MCP en Wispr Flow 1.6.1174

| Señal | Valor | Confianza |
|---|---|---|
| Flag `ask-anything-mcp-hint` | **enabled** | [V] |
| Sección del checklist «Configurar Notetaker» | **«Configurar MCP»** (paso 4) | [V] |
| `prefs.user.notetakerMCPVisibilityState` | `"ModalDismissed"` ⇒ el modal de MCP **ya se mostró y se cerró** | [V] |
| `prefs.user.notetakerMcpCopyPromoSeen` | `false` ⇒ la promoción de copy de MCP **no se vio** | [V] |
| `prefs.user.notetakerChecklistMcpDone` | `false` ⇒ **no configurado** | [V] |
| `prefs.user.notetakerChecklistMcpShownPending` | `true` ⇒ pendiente de mostrar | [V] |
| Flag `connectors` | **disabled** ⇒ los conectores (unión con servicios externos) están apagados | [V] |
| `prefs.user.commandModeEnabled` | `false` ⇒ Command Mode existe, apagado | [V] |
| `prefs.user.pressEnterCommandEnabled` | `false` ⇒ modo «Enter para comandar», apagado | [V] |

> **Conclusión [V]:** en esta instalación el **MCP está presente en la UI pero no configurado**, y los **conectores están desactivados por el servidor**. Es decir: Wispr **prepara la superficie MCP en el cliente** y la habilita por rollout progresivo del lado servidor. Esto es coherente con `feature-flags-cache.json`, donde los flags controlan fases del producto.

---

## 10.2 Qué es el MCP aquí — VERIFICADO contra la documentación pública

> **Corrección importante:** la primera hipótesis de esta auditoría-era ambigua («¿servidor o cliente?»). La documentación pública de Wispr Flow lo aclara. [V] (ver `research-wispr-docs.md`)

### 10.2.1 Wispr Flow es un SERVIDOR MCP remoto, de sólo lectura

| Atributo | Valor verificado |
|---|---|
| **URL del servidor MCP** | `https://api.wisprflow.ai/connect/mcp` |
| **Autenticación** | **OAuth de ámbito de cuenta** — **no requiere API key** |
| **Permisos** | **Sólo lectura** (read-only) |
| **Instalación** | Un clic hacia **Claude, ChatGPT, Gemini y Cursor** |
| **Datos expuestos** | Resúmenes de reuniones · Notas · Transcripciones · Asistentes · Tareas · **Scratchpad** · Calendario · Notas compartidas |

> **Conclusión [V]:** el MCP de Wispr es un **puente de lectura hacia tu cuenta de Wispr**, para que un asistente externo pueda consultar tus reuniones y notas. **No expone el dictado ni la escritura.** Es un diseño prudente y coherente con «sólo lectura».
>
> **Esto invalida parcialmente la lectura del checklist** («Configurar MCP» como paso 4 del *notetaker*): el MCP de Wispr es de **cuentas**, no de reuniones. Se ofrece en el checklist del notetaker porque es el contexto donde más valor aporta (consultar la última reunión desde el chat), no porque sea una función del notetaker. [I]

### 10.2.2 Lo que la instalación local revela y la web no

| Evidencia local | Interpretación |
|---|---|
| `prefs.user.notetakerMCPVisibilityState = "ModalDismissed"` | El modal de configuración MCP **ya se mostró y se cerró** sin completarse [V] |
| `notetakerChecklistMcpDone = false` | El paso quedó pendiente [V] |
| `notetakerMcpCopyPromoSeen = false` | La promoción de copy no se vio [V] |
| Flag `connectors` **disabled** | Los **conectores** (otra cosa: uniones con servicios externos de terceros) están apagados por el servidor [V] |
| Tabla `InstructHistory` + `routesPayload` + `instruct-history-tool-calls` | Wispr **además** tiene un motor de instrucciones con llamadas a herramientas, **interno**, no expuesto por MCP [V] |

> **Reconciliación [I]:** el MCP público (remoto, sólo lectura, OAuth) y el motor de instrucciones interno (local, con herramientas, persistido) son **dos mecanismos distintos**. El MCP expone **datos ya producidos**; el motor de instrucciones **produce** datos invocando herramientas. Para Dixi, la lección es clara: **exponer por MCP sólo lectura y datos ya existentes**; el poder de escritura debe quedar en la app, con confirmación humana. Ver §10.5.

---

## 10.3 La superficie «agente» que hay detrás

La tabla `InstructHistory` es la clave. Su nombre y sus campos la delatan:

| Evidencia | Detalle | Confianza |
|---|---|---|
| Tabla `InstructHistory` + `InstructChatSession` | Historial y sesión de instrucciones | [V] |
| Migración `instruct-history-tool-calls` (2026-06-12) | El historial guarda **llamadas a herramientas** | [V] |
| `InstructHistory.routesPayload` | **Payload de rutas** ⇒ el historial serializa invocaciones de herramientas | [V] |
| `NotetakerChatAgentStates.trajectory` | **Trayectoria** del agente (JSON) | [V] |
| `NotetakerChatAgentStates.revision` | Control de concurrencia optimista | [V] |
| `NotetakerChatMessages.content` (JSON) + `metadata` + `feedback` | Contenido estructurado con feedback por mensaje | [V] |
| `prefs.user.instructInternalDataSharingEnabled = true` | Se comparten datos internos con el servicio de instrucciones | [V] |

> **Lectura [I]:** Wispr no implementa un «chat con el notetaker» como un LLM con RAG classical, sino como un **agente con trayectoria persistida, control de concurrencia y herramientas**. Eso es un nivel de sofisticación que ningún proyecto open source evaluado replica, y que excede el alcance de Dixi v1.

---

## 10.4 Qué debería exponer Dixi como MCP

Si Dixi se expone como servidor MCP, el conjunto mínimo útil es pequeño y de altísimo valor:

```json
{
  "tools": [
    {
      "name": "dixi_dictate",
      "description": "Inicia una sesión de dictado en el equipo del usuario y devuelve el texto transcrito y formateado.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "appScope":    { "type": "string", "description": "Contexto opcional (p.ej. 'email formal')" },
          "language":    { "type": "string", "description": "Código ISO del idioma" },
          "styleId":     { "type": "string", "description": "Id del estilo a aplicar" },
          "transformId": { "type": "string", "description": "Transform a aplicar tras dictar" }
        }
      }
    },
    {
      "name": "dixi_search_history",
      "description": "Busca en el historial de dictados por texto, aplicación o rango de fechas.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query":   { "type": "string" },
          "app":     { "type": "string" },
          "since":   { "type": "string", "format": "date-time" },
          "limit":   { "type": "integer", "default": 20 }
        }
      }
    },
    {
      "name": "dixi_transform",
      "description": "Aplica un transform (resumir, formalizar, traducir, viñetas…) a un texto.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "text":         { "type": "string" },
          "transformId":  { "type": "string" },
          "instruction":  { "type": "string" }
        }
      }
    },
    {
      "name": "dixi_list_meetings",
      "description": "Lista las reuniones grabadas con su resumen y participantes.",
      "inputSchema": { "type": "object", "properties": { "since": { "type": "string" } } }
    },
    {
      "name": "dixi_get_meeting_summary",
      "description": "Devuelve el resumen, las tareas detectadas y la transcripción de una reunión.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "meetingId":     { "type": "string" },
          "includeTranscript": { "type": "boolean", "default": false }
        }
      }
    },
    {
      "name": "dixi_dictionary_lookup",
      "description": "Consulta las entradas del diccionario (términos y fragmentos) del usuario.",
      "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } }
    }
  ]
}
```

**Transportes posibles para Dixi:**

| Transporte | Complejidad | Recomendación |
|---|---|---|
| **stdio** (el LLM lanza el proceso) | Baja | ✅ **Por defecto** — como `mcp-server-*` de referencia |
| **HTTP/SSE local** (`127.0.0.1:PORT`) | Media | ✅ Para clientes que no lanzan procesos (Claude Desktop con URL, VS Code) |
| **Named pipe (Windows)** | Baja-media | Opcional |

> **Decisión recomendada:** implementar **stdio** primero (es lo que espera la mayoría de clientes MCP de escritorio) y **HTTP local** como alternativa. El servidor MCP debe hablar con el **proceso principal de Dixi** (que ya tiene el historial en SQLite y el motor de transforms), no con una copia.

---

## 10.5 Seguridad y privacidad del MCP (requisito de diseño, no de Wispr)

Un servidor MCP local expone las capacidades del usuario a un LLM. Controles mínimos:

| Control | Implementación |
|---|---|
| **Confirmación humana** | Toda invocación que **escriba** (dictar, transformar, pegar) exige confirmación, salvo que el cliente MCP declare un permiso de escritura explícito |
| **Allowlist de herramientas** | Configurable; por defecto sólo lectura |
| **Sin red saliente** | El servidor MCP es local y **no** llama a la nube por sí mismo; usa el mismo provider configurado en Dixi |
| **Auditoría** | Toda invocación se registra en `dixi_mcp_calls` (tool, args hash, resultado ok/error, timestamp) |
| **Exclusión de audio** | Ninguna herramienta devuelve audio crudo |
| **Secretos** | La clave del provider **nunca** se expone al cliente MCP; vive en el almacén de credenciales de Windows |

```sql
CREATE TABLE dixi_mcp_calls (
  id         TEXT PRIMARY KEY,
  tool       TEXT NOT NULL,
  args_hash  TEXT NOT NULL,     -- hash, no el argumento completo (puede contener PII)
  ok         INTEGER NOT NULL,
  error      TEXT,
  duration_ms INTEGER,
  created_at TEXT NOT NULL
);
```

---

## 10.6 Lo que no se pudo verificar

| Ítem | Estado |
|---|---|
| Catálogo exacto de herramientas MCP que expone Wispr | **[NV]** — requiere configurar MCP o documentación pública |
| Nombre y contenido del schema MCP de Wispr | **[NV]** |
| Si Wispr es servidor, cliente o ambos | **[I]** — probablemente ambos, con el servidor como uso principal |
| Permite el usuario elegir qué herramientas se comparten | **[NV]** |

> La investigación de documentación pública de Wispr Flow (en curso al cierre de esta auditoría) puede cerrar estas preguntas; véase [15-INTEGRACION-MINIMAX.md](15-INTEGRACION-MINIMAX.md) y la nota de estado en §10.8.

---

## 10.7 Qué aporta el MCP a Dixi en términos estratégicos

El MCP es, hoy, **la vía por la que un notetaker personal deja de ser una isla**:

```
┌──────────────┐   MCP (stdio/HTTP local)   ┌────────────────────────┐
│  Claude Code │ ─────────────────────────▶ │  dixi-mcp-server       │
│  Claude      │                            │   ├─ dictar             │
│  Desktop     │                            │   ├─ buscar historial   │
│  VS Code     │                            │   ├─ meetings + resumen │
│  Cursor      │                            │   └─ transforms         │
│  5ire / n8n  │                            └───────────┬────────────┘
└──────────────┘                                        │
                                                        │ lee/escribe
                                          ┌─────────────▼─────────────┐
                                          │  dixi-core (SQLite local)  │
                                          └───────────────────────────┘
```

**Valor concreto:** un LLM puede «recordar lo que dicté la semana pasada», «resumir mis 3 reuniones de hoy», «convertir este párrafo en una lista». Eso convierte a Dixi de *teclado* a **memoria personal operable**.

**Riesgo:** el MCP es también una vía de exfiltración si se conecta a un LLM en la nube. Por eso el diseño de §10.5 (confirmación, auditoría, sin audio) es **no negociable**, y es una ventaja de privacidad frente a un producto que ya sube el audio a su nube.

---

## 10.8 Comparación con alternativas

| Capacidad | Wispr Flow | Handy | OpenWhispr | **Dixi (objetivo)** |
|---|---|---|---|---|
| MCP servidor | ✅ (UI presente, no configurado) | ❌ | ⚠️ (servidor **no está en el repo** ⇒ no portable) | ✅ v1 |
| MCP cliente (conectores) | ⚠️ flag `connectors` disabled | ❌ | ⚠️ | Opcional |
| Agente con trayectoria persistida | ✅ (`trajectory`, `revision`) | ❌ | ❌ | Fase 3 |
| Llamadas a herramientas en el historial | ✅ (`instruct-history-tool-calls`) | ❌ | ❌ | ✅ v1 |
| Control de concurrencia en el chat | ✅ (`revision`) | ❌ | ❌ | ✅ |
| Feedback por mensaje | ✅ | ❌ | ❌ | ✅ |
| Herramientas de dictado vía MCP | [NV] | ❌ | ❌ | ✅ |
| Confirmación humana en escrituras | [NV] | n/a | n/a | ✅ **obligatorio** |
| Auditoría de llamadas MCP | [NV] | n/a | n/a | ✅ |

---

### Referencias cruzadas

- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — capas y protocolo propio `wispr-extension://`
- [07-TRANSFORMS.md](07-TRANSFORMS.md) — motor de instrucciones
- [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md) — chat con el notetaker
- [15-INTEGRACION-MINIMAX.md](15-INTEGRACION-MINIMAX.md) — abstracción de provider
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
