# 15 — Integración con MiniMax: estrategia, límites y diseño del provider

> Investigación sobre la documentación pública de MiniMax (platform.minimax.io), sin ninguna llamada de pago, sin registro y sin uso de credenciales.
> Marcadores: **[V]** verificado en documentación oficial · **[I]** inferido · **[NV]** no verificado / requiere verificación por el titular de la cuenta.
> Informe completo: `docs/dixi-reverse-engineering/evidencia/research-minimax.md`.

---

## 15.1 ⚠️ Corrección a una premisa de la auditoría

> **La premisa de partida («el M Plan no incluye audio») es incorrecta según la documentación vigente.** [V]
>
> - La guía oficial de **speech-to-text** dice literalmente: *"To use speech recognition, either **Pay-as-you-go** or **subscribe to M Plan**"*. [V]
> - Los tres tiers de M Plan incluyen **«image and audio models»**. [V]
> - El endpoint ASR **`asr-1.0`** es alcanzable con una **Subscription Key**. [V]
>
> **Consecuencia para Dixi:** MiniMax **sí puede ser el motor de ASR remoto** de Dixi, no sólo el de LLM. Eso abre una arquitectura **híbrida** nueva: ASR local por defecto (privacidad) + ASR MiniMax opcional (calidad, multilingüe, coste por hora).

---

## 15.2 Veredicto

> **CONDICIONAL — SÍ.** Una aplicación de escritorio personal **puede** llamar a MiniMax con una M Plan *Subscription Key* desde una app de terceros.

**Condiciones verificadas en la documentación:** [V]

1. La key **no está restringida a clientes IDE/CLI**: el FAQ oficial de M Plan dice que sirve con *"**any AI tool that supports the OpenAI-compatible or Anthropic-compatible protocol"*.*
2. Existe una guía oficial **«Other Tools»** con configuraciones paso a paso para **aplicaciones de escritorio de terceros**: *Cherry Studio, Chatbox, Zed, TRAE, Xcode, Open WebUI, Dify, LangChain, OpenHands, nanobot*. [V]
3. El plan está declarado para **«individual, interactive developer use»** y la propia documentación recomienda *pay-as-you-go* para producción. [V]
4. La métrica usa **ventanas compartidas de 5 horas y semanales** por tier, con **throttling dinámico en horas pico** y topes absolutos por tier **no documentados**. [V]
5. **La misma key puede escalar silenciosamente** de uso incluido → **Credits comprados**. [V]
6. La key **debe guardarse por usuario y nunca exponerse en código de cliente/renderer** — MiniMax advierte que las keys expuestas **pueden deshabilitarse automáticamente**. [V]

**Para una app personal de un solo usuario como Dixi, las condiciones 3–6 son compatibles con el uso previsto**, siempre que la key viva en el almacén de credenciales de Windows y el consumo sea transparente.

---

## 15.3 Matriz de compatibilidad de endpoints

| Superficie | Base URL | Endpoint | Modelos | Auth |
|---|---|---|---|---|
| **Anthropic-compatible** (documentada como «recomendada») | `https://api.minimax.io/anthropic` | `POST /v1/messages` | `MiniMax-M3.1-Flash`, `MiniMax-M3` | `Authorization: Bearer <KEY>` |
| Anthropic — conteo de tokens | mismo | `POST /v1/messages/count_tokens` | ídem | Bearer |
| **OpenAI-compatible** (Chat Completions) | `https://api.minimax.io/v1` | `POST /v1/chat/completions` | `MiniMax-M3.1-Flash`, `MiniMax-M3` | Bearer |
| OpenAI Responses | `https://api.minimax.io/v1` | `POST /v1/responses` | referenciado | Bearer |
| **Nativo MiniMax** | `https://api.minimax.io` | `POST /v1/text/chatcompletion_v2` | ídem | Bearer |
| **Speech-to-Text (ASR)** | `https://api.minimax.cn/v1/speech_to_text` ⚠️ *host tal como aparece en la guía* | `POST` multipart `file` + `model=asr-1.0` | `asr-1.0` | Bearer |
| **T2A (síntesis)** | — | `speech-t2a-http` / `speech-t2a-websocket` | `speech-2.8-hd`, `speech-2.8-turbo`, `speech-2.6-*`, `speech-02-*` | Bearer |

> ⚠️ **El host del endpoint ASR es ambiguo** [NV]: la guía de speech-to-text imprime `api.minimax.cn` en todos sus ejemplos, mientras el resto de la documentación usa `api.minimax.io`; el README del MCP-JS usa un tercer par (`api.minimaxi.chat` global / `api.minimax.chat` China) y se contradice. **Hay que confirmar contra la región de la cuenta antes de implementar.**

### 15.3.1 Modelos (verificados)

| Modelo | Contexto | Notas |
|---|---|---|
| `MiniMax-M3.1-Flash` | **1 000 000 tokens** (entrada + salida) | **Siempre razona**; devuelve HTTP 400 si se envía `thinking: disabled` o `reasoning_effort: none` |
| `MiniMax-M3` | **1 000 000 tokens** | Razonamiento **apagado por defecto**; se activa con `thinking: adaptive` |
| `MiniMax-M3.1-Flash-Preview` | — | Retirado; se sirve con `MiniMax-M3.1-Flash` |
| `MiniMax-M2.5` / `M2.7` | — | No disponibles a nuevos usuarios desde 2026‑10‑19, **retiran 2026‑11‑18**; en M Plan **auto-enrutan a M3.1 Flash** |
| `MiniMax-M2.1`, `M2`, `M2-her`, `MiniMax-Text-01` | — | **Retirados**; `M2.1`/`M1` **no** son utilizables |
| `asr-1.0` | ≤ 500 s y ≤ 50 MB por request | ASR |
| `speech-2.8-hd` / `-turbo` | — | TTS |

> **Consecuencia práctica [I]:** la API de MiniMax es un **drop-in de protocolo**. Como `MiniMax-M3.1-Flash` es alcanzable por ambas superficies (OpenAI y Anthropic), Dixi puede implementar **un solo** `OpenAiCompatibleProvider` y **uno** `AnthropicCompatibleProvider` y cubrir MiniMax, OpenAI/Azure/OpenRouter y la mayoría de gateways locales, **sin un cliente específico de MiniMax**.

---

## 15.4 Límites y cuotas (todos VERIFIED salvo lo marcado)

| Recurso | Límite | Estado |
|---|---|---|
| **LLM RPM** (`M3.1-Flash`, `M3`) | **200** | [V] |
| **LLM TPM** | **10 000 000** | [V] |
| LLM — contexto | **1 000 000 tokens** (entrada + salida combinadas) | [V] |
| LLM — `max_tokens` (Anthropic) | soportado; el razonamiento **cuenta** contra él (ej. 131 072) | [V] |
| Contexto de cliente recomendado | **512 K (524 288)** — ~20 % más barato que 1M con rendimiento similar | [V] |
| **ASR** — conexiones concurrentes | **2** | [V] |
| **ASR** — TPM | **30 000 segundos de audio** | [V] |
| **ASR** — por request | **≤ 500 s** y **≤ 50 MB** (`400` si >500 s, `413` si >50 MB) | [V] |
| ASR — streaming de audio | **no soportado** | [V] |
| T2A — RPM | 60 (Voice Cloning 60, Voice Design 20) | [V] |
| Imagen (`image-01`) — RPM | 10 | [V] |
| Video (`MiniMax-H3`) — RPM | 300 (30 in-flight) | [V] |
| Música — RPM / CONN | 120 / 20 | [V] |
| Ventanas de cuota | **5 horas + semanal** por tier, compartidas; throttling dinámico | [V] |
| Topes absolutos por tier | **no documentados** | [NV] |
| Créditos | 1 000 Credits = **$1** (= precio de lista pay-as-you-go), válidos 1 año; **el uso del plan se consume primero** | [V] |
| Consulta de saldo | `GET https://www.minimax.io/v1/token_plan/remains` | [V] |
| Precio ASR (referencia pay-as-you-go) | **$0,38 / hora** | [V] |
| ¿Los 200 RPM / 10M TPM son iguales para Subscription Keys? | la página de límites **no diferencia** | [NV] — verificar empíricamente |

> **Implicación de diseño para Dixi [I]:** el límite de **ASR CONN = 2** y **TPM 30 000 s de audio** es **holgado** para un único usuario dictando (30 000 s ≈ 8,3 h de audio por minuto de ventana). El límite de **LLM RPM 200** también. La restricción real es la **ventana compartida de 5 h/semanal** y el **escalamiento silencioso a Credits**.

---

## 15.5 Texto largo y streaming

**Texto largo.** No hace falta *chunking* hasta 1 M de tokens: entrada + salida comparten el presupuesto de 1 000 000. El chunking sólo es necesario por encima de eso o al superar el **límite de 64 MB del cuerpo de la request**. `POST /anthropic/v1/messages/count_tokens` permite estimar tokens de entrada sin generar. MiniMax recomienda configurar el **cliente a 512 K** y compactar el historial antes, porque 1 M encarece ~20 % sin ganancia medida de precisión. [V]

**Streaming.** [V]
- Anthropic-compatible: eventos `content_block_start` / `content_block_delta`, con `delta.type = "thinking_delta" | "text_delta"`.
- OpenAI-compatible: `data:` SSE con chunks `choices[].delta.content`.
- El **razonamiento se emite como delta separado** ⇒ un cliente de escritura debe poder distinguir `thinking_delta` de `text_delta` y **no insertar el razonamiento en el campo de texto**.

**Control del razonamiento (importante para una UI de escritura).** [V]
- `MiniMax-M3.1-Flash` **siempre razona** y devuelve **HTTP 400** si se envía `thinking: {"type":"disabled"}` o `reasoning_effort: "none"`.
- La profundidad se ajusta con `output_config.effort` (Anthropic) / `reasoning_effort` (OpenAI): `low | medium | high | xhigh | max` — **default `max`**, el más lento y caro.
- `MiniMax-M3` razona **apagado** por defecto; `thinking: adaptive` lo activa.
- **Ignorados** por la API: Anthropic `top_k`, `stop_sequences`, `mcp_servers`, `context_management`, `container`; OpenAI `presence_penalty`, `frequency_penalty`, `logit_bias`. `n` debe ser `1`. `function_call` no soportado (usar `tools`).

> **⚠️ Riesgo de UX [I]:** con `M3.1-Flash` y `effort` por defecto `max`, **cada transform costoso y lento**. Para Dixi, el provider debe:
> 1. Usar `MiniMax-M3` (razonamiento apagado) para transforms de reescritura simple.
> 2. Fijar `output_config.effort = "low"` o `"medium"` por defecto.
> 3. **Nunca** insertar `thinking_delta` en el texto de salida.

---

## 15.6 Manejo de errores y señales de cuota

> **🔴 La trampa más importante de esta integración [V]:** en los endpoints de texto, el error se reporta **dentro del cuerpo de una respuesta HTTP 200**, como `base_resp: { status_code, status_msg }` (`status_code: 0` = éxito).
> **Un cliente que sólo inspecciona el status HTTPtratará los fallos como éxitos.** Este es un bug que pasaría desapercibido en producción y corrompería texto en el campo del usuario.

| Señal | Significado | Acción en Dixi |
|---|---|---|
| `base_resp.status_code == 0` | Éxito | — |
| `1002` / HTTP `429` | Rate limit | Reintento con backoff exponencial + jitter |
| `2056` | **Cuota del plan agotada** | Estado duro: «la cuota del plan se agotó, se renueva en la ventana de 5 h» → ofrecer upgrade / credit pack / key pay-as-you-go |
| `1008` / HTTP `402` | Facturación / saldo insuficiente | Estado de facturación; **no** reintentar |
| `1004` / `2049` / HTTP `401` | Clave inválida | Estado de credenciales; avisar al usuario |
| `1026` / `1027` | Filtro de contenido | Estado de filtro; **no** reintentar idéntico |
| `1039` | Desbordamiento de contexto | Recortar / compactar y reintentar |
| `1000, 1001, 1008, 1013, 2013` | Otros documentados | Genérico |

**Modulación de contenido** (booleanos separados): `input_sensitive` / `output_sensitive` con `*_sensitive_type` ∈ {1 severo, 2 pornografía, 3 publicidad, 4 prohibido, 5 abuso, 6 violencia/terrorismo, 7 otro}. [V]

**El endpoint ASR usa otro envelope** (estilo OpenAI): `{"type":"error","error":{…},"request_id":"…"}` con HTTP `400/401/402/413/422/429/500` y `error.type` ∈ `bad_request_error | authorized_error | insufficient_balance_error | invalid_request_error | unprocessable_entity_error | rate_limit_error | server_error`. [V]

> **⇒ Dos parsers de error distintos según endpoint.** El cliente de ASR y el de LLM **no** pueden compartir el mismo manejo de errores.

**Entrada de audio al LLM: no soportada.** [V] La API de chat OpenAI-compatible dice *"audio input is not currently supported"* y la Anthropic-compatible sólo expone bloques `text`/`image`/`video`/`tool_use`/`tool_result`/`thinking`. ⇒ **Un pipeline «audio → LLM» exige primero el endpoint ASR y después una llamada de texto.**

---

## 15.7 Audio en M Plan: qué queda dentro y qué no

| Producto | ¿En M Plan? | Nota |
|---|---|---|
| **LLM de texto** (`M3.1-Flash`, `M3`) | ✅ | Consume cuota del plan |
| **ASR** (`asr-1.0`) | ✅ | «either Pay-as-you-go or subscribe to M Plan» |
| **TTS** (`speech-2.8-hd`/`turbo`) | ✅ | |
| **Imagen** (`image-01`) | ✅ | |
| **Video** (`MiniMax-H3`) | ✅ | RPM 300 |
| **Música** | ✅ | RPM 120 |
| Audio Subscription ($5–$999/mes) | ❌ | Producto aparte, no parte de M Plan |
| Video Packages ($1 000–$5 000) | ❌ | Producto aparte |

> **Corrección de la premisa:** el enunciado original de la auditoría asumía que el audio **no** estaba incluido. La documentación vigente dice lo contrario. Hay que verificarlo contra la cuenta concreta antes de comprometerse, pero la evidencia pública es inequívoca.

---

## 15.8 Interfaz de provider recomendada para Dixi

```typescript
export type ProviderId = 'minimax' | 'openai' | 'anthropic' | 'ollama' | 'local';

export interface ProviderConfig {
  id: ProviderId;
  baseUrl: string;
  model: string;
  /** Perfil de effort/thinking del provider. */
  effort?: 'low' | 'medium' | 'high' | 'xhigh' | 'max';
  /** Requests por minuto: no deducibles de la config. */
  rpmLimit?: number;
  tpmLimit?: number;
  /** Credencial: referencia opaca, NUNCA el valor. */
  credentialRef: string;
}

export interface LlmMessage { role: 'system' | 'user' | 'assistant'; content: string; }

export interface LlmRequest {
  messages: LlmMessage[];
  maxOutputTokens?: number;
  temperature?: number;
  stream?: boolean;
  /** Para el endpoint ASR: 0..1 s de silencio inicial. */
  language?: string;
}

export interface LlmChunk {
  type: 'text' | 'thinking' | 'usage' | 'error';
  delta?: string;
  usage?: { promptTokens: number; completionTokens: number };
}

export interface LlmError {
  kind: 'auth' | 'rate_limit' | 'plan_quota' | 'billing'
      | 'context_overflow' | 'content_filter' | 'network' | 'unknown';
  message: string;
  retryable: boolean;
  /** Segundos sugeridos de espera (cuando aplique). */
  retryAfterMs?: number;
}

export interface LlmProvider {
  readonly id: ProviderId;
  readonly model: string;
  complete(req: LlmRequest, signal: AbortSignal): AsyncIterable<LlmChunk>;
  transcribe(req: TranscribeRequest, signal: AbortSignal): Promise<TranscribeResult>;
}
```

**Reglas de implementación no negociables (derivadas de la investigación):**

1. **Parsear `base_resp.status_code` SIEMPRE**, aunque el HTTP sea 200. Un `status_code != 0` es un error.
2. **Separar `thinking_delta` de `text_delta`** y nunca insertar el razonamiento en el texto del usuario.
3. **Fijar `effort` explícitamente** (por defecto `low`/`medium`), nunca depender del default `max`.
4. **Usar `MiniMax-M3`** (razonamiento apagado) para transforms de reescritura; `M3.1-Flash` sólo para razonamiento complejo.
5. **Topear `max_tokens`** teniendo en cuenta que el razonamiento **cuenta** contra él.
6. **Respetar el cap de 512 K de contexto** en el cliente, no 1 M.
7. **Consultar `GET /v1/token_plan/remains`** periódicamente para mostrar el saldo al usuario y **evitar el escalamiento silencioso a Credits**.
8. **Nunca** enviar la key desde el renderer; resolverla en el backend Rust desde el almacén de credenciales.
9. **Idempotencia**: no reintentar `2056`, `1004`, `1026/1027` ni `402`.

---

## 15.9 Almacenamiento seguro de credenciales en Windows (recomendación oficial de Microsoft)

| Opción | Cuándo usarla | Referencia |
|---|---|---|
| **Windows Credential Manager** (`CredWrite`/`CredRead` en `Advapi32`) | **Recomendada para Dixi** | API de Windows `CredMan` |
| **DPAPI** (`CryptProtectData`, scope `CurrentUser`) | Si se necesita cifrar un secreto propio dentro de un archivo | Microsoft: *Data protection API* |
| **`Windows.Security.Credentials.PasswordVault`** | WinRT, desde apps UWP/packaged | Microsoft: *PasswordVault* |

**Diseño recomendado para Dixi:**

```
Credenciales de provider  →  Windows Credential Manager
                              Target: "Dixi/Provider/minimax"
                              UserPersistence: LocalMachine (o Enterprise para roaming)
                              UserName: la propia key
                              
Llave de cifrado de la BD local → DPAPI CurrentUser
```

**Nunca**: escribir la key en `config.json` plano (que es exactamente lo que hace la arquitectura de Wispr con su `session.json`), ni exponerla al renderer por `invoke`.

---

## 15.10 Riesgos y preguntas abiertas

| # | Pregunta | Estado | Acción |
|---|---|---|---|
| 1 | ¿Los 200 RPM / 10M TPM son iguales para Subscription Keys? | **[NV]** | Verificar con una sonda de bajo coste |
| 2 | ¿Qué host real usa el endpoint ASR (`api.minimax.cn` vs `api.minimax.io`)? | **[NV]** | Confirmar según la región de la cuenta |
| 3 | ¿El SDK Anthropic acepta `x-api-key` o sólo `Authorization: Bearer`? | **[NV]** (riesgo bajo) | Comprobación de una sola llamada |
| 4 | ¿Los topes absolutos por tier de M Plan? | **[NV]** | No documentados |
| 5 | ¿El escalamiento silencioso a Credits puede desactivarse? | **[NV]** | Verificar el FAQ de M Plan / soporte |
| 6 | ¿La cuota de la window de 5 h se reinicia por rol o por cuenta? | **[NV]** | Verificar |
| 7 | ¿El contenido de las reuniones y del audio se considera «uso de desarrollo»? | **[NV]** | Riesgo de cumplimiento del «individual, interactive developer use» |

> **Punto 7 es el más material:** el plan se declara para uso de desarrollo individual e interactivo. **Un dictado personal de uso diario está, con suerte, en la zona gris.** La mitigación es straightforward: **Dixi debe funcionar 100 % sin credenciales** (ASR y formateo locales), y la integración MiniMax es **opcional y explícita**. Esta decisión de diseño también es la mejor respuesta de marketing: *«Dixi funciona sin cuenta; si querés más calidad, conectá MiniMax»*.

---

### Referencias cruzadas

- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — pipeline y punto de inserción del provider
- [07-TRANSFORMS.md](07-TRANSFORMS.md) — transforms y el contrato de diff
- [08-REUNIONES-NOTAS.md](08-REUNIONES-NOTAS.md) — ASR para reuniones
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — coste y latencia
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — prioridades
