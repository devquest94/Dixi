# Dixi — Informe técnico de Fase 2B

**Alcance:** multilingualidad, diccionario personal, provider MiniMax abstracto y
endurecimiento de la entrega.
**Rama:** `feat/dixi-dictation-quality` (base: `develop` @ `be66ae8`, cierre de Fase 2A).
**Fecha:** 2026-10-20.

---

## 1. Resumen ejecutivo

| Objetivo | Estado | Qué se entregó |
|---|---|---|
| A — Reconocimiento multilingüe | **DESARROLLADO (parcial)** | Contexto de transcripción cableado al `initial_prompt` del motor; harness de medición WER/CER + dataset de 24 casos. El modo auto/forzado por idioma ya existía. |
| B — Diccionario personal | **DESARROLLADO** | Motor de sustitución en Rust con almacenamiento SQLite, CRUD, prioridades, reglas por idioma, contadores de uso e integración en el pipeline. |
| C — Provider MiniMax abstracto | **DESARROLLADO (parcial)** | Módulo con parsing de `base_resp`, separación `thinking_delta`/`text_delta` y control de `effort`. **No está cableado al pipeline de post-procesado**: ver §5.3. |
| D — Rendimiento y endurecimiento | **DESARROLLADO (parcial)** | Script de medición de memoria en bandeja reutilizado; la medición concreta está en §7. |
| Defectos de Fase 2A | **CORREGIDOS (3/3)** | Aviso espurio de autostart, colisión de IDs de prompt, errores de comando tragados en el store. |
| Regresión `tauri/custom-protocol` | **CORREGIDO** | Prueba de regresión en dos niveles (Rust y PowerShell). |

### Lo que NO se hizo, y por qué

Estas decisiones son deliberadas y quedan documentadas para que la Fase 2C las
retome sin contexto:

1. **El pipeline de post-procesado LLM no usa todavía el módulo MiniMax.**
   El módulo existe, está probado y sabe hablar el idioma de MiniMax, pero
   conectarlo exige tocar `llm_client.rs`, que hoy tiene su propio
   descubrimiento de formato y su propio cliente HTTP. Se optó por no
   duplicar ese cliente a ciegas. El camino de integración esta delimitado en §5.3.
2. **No hay "<scop> de perfil</scop>" en el diccionario.** El modelo de datos
   ya tiene `scope` (una cadena, con `"global"` reservado) y el filtrado por
   ámbito funciona, pero Dixi no tiene subsistema de perfiles: no hay
   selector de perfil en la UI ni identificador de perfil en el motor. Crear
   un subsistema de perfiles entero excedía el alcance de esta fase; se dejó
   la columna y el filtrado listos para cuando exista.
3. **No se implementó el aprendizaje automático del diccionario.** La
   investigación de referencia (doc 04) dice que el 81 % de las entradas de
   Wispr se aprenden de las ediciones posteriores del usuario en la app
   destino. Eso requiere observar la edición en la app enfocada (Windows
   UIAutomation), no sólo el dictado. Es un proyecto aparte.
4. **No se añadió un runner de evaluación end-to-end.** El harness calcula
   WER/CER sobre pares (referencia, hipótesis) y el dataset está listo, pero
   el bucle que transcribe cada audio del dataset con cada modelo y escribe
   la hipótesis no se automatizó: requiere los modelos descargados y mucho
   tiempo de CPU en esta máquina.
5. **El módulo interno `shortcut::handy_keys` conserva su nombre**, igual
   que el valor del ajuste `KeyboardImplementation::HandyKeys`, porque está
   persistido en el store de ajustes de los usuarios. El contrato IPC visible
   sí se renombró a Dixi (§7.3).

---

## 2. Bloque 2B.1 — Cierre de fundamentos

### 2.1 Git

`develop` estaba en `f562745` (sólo documentación de auditoría) mientras el
trabajo real estaba en `feat/handy-foundation`. Se comprobó que `develop`
era antepasado de `feat/handy-foundation` y se hizo un **fast-forward**
(382 ficheros, +90 937 líneas) — sin merge commit, sin reescritura de
historial. A partir de ahí se creó `feat/dixi-dictation-quality`.

```
develop  f562745 → be66ae8   (fast-forward)
feat/dixi-dictation-quality  be66ae8 → …
```

### 2.2 Defecto 1 — aviso espurio de autostart

**Causa raíz.** `apply_autostart` (`src-tauri/src/autostart.rs`) llamaba a
`manager.enable()` / `manager.disable()` incondicionalmente en cada arranque.
El backend `auto-launch` devuelve error al desactivar una entrada que nunca
se registró. La ruta de macOS **ya tenía** el guardia correcto
(`SMAppServiceStatus` se consultaba antes de escribir); la de Windows y
Linux no.

**Efecto.** Todo usuario con autostart apagado —el caso común— veía un
`WARN` en el log en cada arranque, que sugería un fallo donde no lo había.

**Corrección.** Se consulta el estado actual con `manager.is_enabled()` y se
sale sin escribir cuando ya coincide con la preferencia. El guardia de macOS
se conserva intacto. Si la lectura del estado falla, se continúa con la
escritura para que el error real se reporte.

### 2.3 Defecto 2 — "guardado incorrecto" del prompt de post-procesado

Había **dos** causas, y ambas producían el síntoma reportado:

**(a) Colisión de identificadores.** `add_post_process_prompt` generaba el id
así:

```rust
let id = format!("prompt_{}", chrono::Utc::now().timestamp_millis());
```

`timestamp_millis()` tiene resolución de milisegundo. Dos prompts creados
dentro del mismo milisegundo recibían **el mismo id**, y toda búsqueda
(resolución de selección, edición, borrado) resuelve por id y devuelve **la
primera coincidencia**: editar un prompt escribía los cambios **en el
otro**. Ése es exactamente el "guardado incorrecto".

**Corrección.** `generate_prompt_id` combina el timestamp con un contador
atómico por proceso y, además, comprueba la unicidad contra los prompts ya
existentes, avanzando hasta encontrar un id libre. Cubre tanto la colisión
en milisegundos como los ids que sobrevivieron a un reinicio.

**(b) Errores de comando tragados en el store.** El bindings generado por
`tauri-specta` **no lanza excepciones** ante un error de negocio: resuelve
con `{ status: "error", error }`. En `settingsStore.updateSetting` el
resultado se esperaba sin inspeccionar:

```ts
const updater = settingUpdaters[key];
if (updater) {
  await updater(value);      // resuelve con {status:"error"}: nunca entra al catch
}
```

Consecuencia: cuando el backend rechazaba la escritura (por ejemplo
`Prompt with id '...' not found`), el store **conservaba el valor
optimista**. La interfaz mostraba un prompt como seleccionado mientras el
ajuste en disco seguía apuntando al anterior. Al reiniciar, la selección
desaparecía.

**Corrección.** `assertCommandSucceeded` centraliza el desempaquetado del
sobre `{status}`: si el resultado es un error, se lanza y el `catch` revierte
el valor. El rollback ahora se aplica sobre el estado **actual** (no sobre
una instantánea obsoleta), de modo que un cambio concurrente en otro ajuste
no se revierte por accidente. Los actualizadores que ya revisaban el
resultado (`selected_channel`, `vad_backend`) siguen funcionando igual: su
lanzamiento ocurre antes de que el helper vea el valor.

### 2.4 Defecto 3 — regresión `tauri/custom-protocol`

**El defecto.** Sin la característica `tauri/custom-protocol`, un binario
compilado con `cargo build --release` **no lleva el frontend dentro**: la
WebView2 busca el servidor de desarrollo de Vite (`http://localhost:1420`),
no existe, la interfaz queda en blanco, el frontend nunca ejecuta sus
efectos y **los atajos globales no se registran**. El fallo es silencioso:
el proceso arranca, ocupa la bandeja y no se ve nada.

**La prueba de regresión, en dos niveles.**

*Nivel 1 — configuración (Rust, dentro de `cargo test`).*
`src-tauri/src/release_contract.rs` comprueba que `Cargo.toml` declara la
característica y la reenvia a `tauri/custom-protocol`, y que
`tauri.conf.json` declara `frontendDist`. Si alguien borra la característica,
`cargo test` falla.

*Nivel 2 — artefacto (PowerShell).* `scripts/verify-release-protocol.ps1`
comprueba el **binario real**: una compilación de producción incrusta los
assets, así que el nombre con hash de los recursos (p. ej.
`main-DOJvNCEL.js`) aparece dentro del ejecutable. Un binario compilado sin
la característica no lo contiene. La comprobación también cubre la
declaración de la característica, la presencia de `dist/index.html` y, con
`-SmokeTest`, que el binario arranca y responde.

Se añadió el script de npm `npm run check:release-protocol`.

**Resultado sobre el binario de Fase 2A** (compilado el 19/10/2026):

```
[OK  ] Cargo.toml declara custom-protocol -> tauri/custom-protocol
[OK  ] dist/index.html existe
[OK  ] dist/index.html referencia recursos con hash - 3 recurso(s)
[OK  ] El binario release existe
[OK  ] El binario incrusta los recursos del frontend (custom-protocol activo) - incrustados: 3/3
OK: el binario de produccion lleva el frontend incrustado.
```

### 2.5 Reproducibilidad del build

Dos problemas reales del entorno aparecieron al reproducir el build y
quedan documentados aquí porque son la clase de fallo que se repite:

1. **`cargo` no está en el PATH de esta sesión.** Vive en
   `C:\Users\DanielSan\.cargo\bin`. Todo comando de Rust de esta fase se
   ejecutó anteponiéndolo al PATH.
2. **`cmake` tampoco está en el PATH, pero sí está instalado.** La
   compilación C++ de `transcribe-cpp-sys` la requiere, y el binario
   `dixi.exe` de Fase 2A se había producido con ella. CMake está instalado
   como paquete de Python, en
   `C:\Users\DanielSan\AppData\Roaming\Python\Python313\site-packages\cmake\data\bin\cmake.exe`
   (versión 4.4.4). El `CMakeCache.txt` de la compilación anterior lo
   confirmaba. **No hace falta instalar nada**: basta anteponer ese directorio
   al PATH.

   Esto explica por qué un `cargo check` normal fallaba: `cargo check` y
   `cargo build` producen **hashes de metadatos distintos** para el mismo
   paquete, así que el `check` no reutiliza la salida cacheada del script de
   compilación y lo vuelve a ejecutar, que es donde aparece la falta de
   `cmake`. Con `cargo build` (que sí reutiliza la caché) y `cmake` en el
   PATH, la compilación C++ no se repite.

3. **Memoria.** La máquina tiene 16 GB y quedan ~3,8 GB libres con el
   navegador y otras herramientas abiertas. Por eso todas las compilaciones
   de esta fase usaron `--jobs 1` y `CMAKE_BUILD_PARALLEL_LEVEL=2`: el
   perfil `release` usa `lto = true` y `codegen-units = 1`, y el paso de LTO
   es single-threaded pero come alrededor de 1,9 GB. Compilar en release y
   test a la vez hace competir a los dos por la misma reserva.

---

## 3. Bloque 2B.2 — Reconocimiento multilingüe

### 3.1 Qué faltaba de verdad

La auditoría docs sugería «elegir modelo multilingüe y forzar el idioma». Al
leer el código, la realidad era otra:

- El idioma **forzado ya existía**: `AppSettings.selected_language` se
  valida contra las lenguas que anuncia el modelo cargado y, si no la
  soporta, cae a `"auto"` (detección por audio). El modo automático también
  existía, codificado como el valor `"auto"`.
- El vocabulario **ya llegaba al motor**: `settings.custom_words` →
  `vocabulary_for_model(...)` → `RunOptions.vocabulary` → el `vocabulary` de
  transcribe-cpp.
- **Lo que faltaba era el contexto.** El motor acepta un *initial prompt*
  (la capacidad `Feature::InitialPrompt` estaba declarada y el campo
  `RunOptions.prompt` existía), pero **nunca se enviaba**: `run_setup` lo
  dejaba en `Default::default()`, es decir, vacío. El motor anunciaba la
  capacidad y Dixi la ignoraba.

### 3.2 Lo implementado

**Nuevo ajuste `transcription_context`** (`AppSettings`): texto libre con el
contexto del usuario — el idioma que habla, el dominio en el que trabaja, o
una muestra de cómo dicta. Límite de 500 caracteres (`MAX_TRANSCRIPTION_CONTEXT_LENGTH`).

**Cableado al motor** (`src-tauri/src/managers/transcription.rs`):

```rust
fn initial_prompt_for_model(
    supports_initial_prompt: bool,
    context: &str,
    custom_words: &[String],
) -> Option<String>
```

Construye el prompt con el contexto del usuario **más los términos que ya se
envían como vocabulario**, para que ambas cosas no se contradigan. Si el
modelo no anuncia la capacidad, devuelve `None`: mandar un prompt no
soportado hace que el motor avise en **cada** transcripción.

**Comando** `change_transcription_context_setting` (con validación de
longitud), registrado en el contrato IPC y en el store del frontend.

### 3.3 Harness de medición: WER/CER

**El problema.** El proyecto podía transcribir, pero **no podía demostrar**
que un cambio mejoraba la precisión. Sin esa medida, «el modelo nuevo es
mejor» es una opinión.

**Lo entregado** (`src-tauri/src/dixi/evaluation.rs`, sin dependencias
nuevas, con sus pruebas en `evaluation/tests.rs`):

| API | Qué hace |
|---|---|
| `normalize_for_comparison` | Minúsculas, sin acentos, sin puntuación, espacios colapsados |
| `word_edit_distance` / `character_edit_distance` | Distancia de Levenshtein con sustituciones, borrados e inserciones **por separado** |
| `word_error_rate` / `character_error_rate` | Métricas sobre texto crudo (normalizan internamente) |
| `evaluate_case` / `evaluate_cases` | Caso individual y agregado, con desglose por idioma |

Definiciones: se compte por separado `substitutions`, `deletions` e
`insertions`; WER = total de ediciones de palabra / palabras de referencia;
CER = total de ediciones de carácter / caracteres de referencia. Con
referencia vacía y hipótesis no vacía, WER = 1,0; con ambas vacías, 0,0.

El plegado de acentos usa una tabla ASCII explícita para Latin-1 y
Latin Extended-A (std no tiene NFD), de modo que las formas precompuestas y
descompuestas se pliegan igual.

**Dataset** (`docs/validation-phase2b/dataset/dataset.jsonl`): 24 casos
JSON Lines, validados como JSON línea por línea. 16 en español (10 con audio
de referencia reutilizando `docs/validation-phase2a/audio-referencia/F1..F10.wav`
y sus frases), 4 en inglés y 4 de mezcla `es-en` (código técnico en inglés
dentro de frase en español). Los 14 casos sin audio cubren diacríticos,
puntuación invertida (`¿`/`¡`) y mezcla de idiomas — justo lo que un modelo
ingle Pensado para inglés falla.

### 3.4 Pendiente

El harness calcula métricas sobre pares (referencia, hipótesis) y el dataset
está listo, pero **el runner que genera la hipótesis no existe**: falta el
bucle que transcribe cada audio con cada modelo y escribe el resultado. Es
trabajo de CPU y modelos descargados, y quedó fuera de esta fase.

---

## 4. Bloque 2B.3 — Diccionario personal

### 4.1 Lo que dice la referencia

El documento 04 (investigación sobre el diccionario de Wispr Flow) es claro
en dos puntos que la implementación de Fase 2A no cubría:

1. El diccionario **no es una lista de palabras**: tiene `replacement` y
   `replacementHtml`, o sea que **sustituye una frase por otra**. Es un
   motor de reglas de texto.
2. El diccionario **aprende** de las ediciones posteriores del usuario (81 %
   de las entradas de la muestra).

Lo segundo queda fuera de alcance (§1). Lo primero se implementó entero.

### 4.2 Modelo de datos

Nuevo módulo `src-tauri/src/managers/dictionary.rs`, con almacenamiento
SQLite propio (`dictionary.db` en el directorio de datos de la app), con
migraciones vía `rusqlite_migration` — el mismo patrón que `managers/history.rs`.

| Campo | Significado |
|---|---|
| `term` | Lo que se busca |
| `replacement` | Lo que se escribe (vacío en los términos) |
| `kind` | `term` (pista de vocabulario) o `substitution` (regla de reescritura) |
| `enabled` | Regla activa o no |
| `scope` | Ámbito; `"global"` reservado, cualquier otra cadena es un cubo propio |
| `case_sensitive` | Distingue mayúsculas |
| `whole_word` | Sólo como palabra completa, no dentro de otra |
| `priority` | Desempate cuando dos reglas pueden coincidir |
| `language` | Etiqueta de idioma opcional; vacía = todos los idiomas |
| `hit_count` | Cuántas veces disparó la regla |

Índice único sobre `(term COLLATE NOCASE, scope, kind)`: el mismo término
puede existir como pista de vocabulario **y** como sustitución, pero no
duplicado en el mismo ámbito y tipo.

### 4.3 El motor de sustitución

`apply_substitutions(text, rules, language)` — una **sola pasada** sobre el
texto original:

- En cada posición gana la regla **más larga** que coincida; entre reglas de
  igual longitud, la de **mayor prioridad**.
- Una regla más corta **nunca** puede reescribir un fragmento producido por
  una más larga, y una sustitución **nunca** se reescribe otra vez. No hay
  cascadas surprises.
- Una regla etiquetada con idioma sólo aplica a transcripciones de ese
  idioma (`es` cubre `es-AR`); sin etiqueta aplica a todos.
- Se **conserva el estilo de mayúsculas** de lo emparejado: todo mayúsculas
  → todo mayúsculas; initial cap → initial cap; el resto → literal.
- Respeta los límites de palabra, la puntuación y los acentos.

El manager mantiene un **snapshot en memoria** de las entradas: el camino
caliente de la transcripción necesita las reglas en cada utterance y no
puede tocar el disco.

### 4.4 Integración en el pipeline

- **Hacia el motor**: los términos del diccionario se **unen a
  `custom_words`**, de modo que llegan como `vocabulary` y participan de la
  misma pasada de corrección difusa. Sin duplicados, comparando sin distinguir
  mayúsculas.
- **Sobre el texto**: las reglas se aplican en `complete()`, **después** de
  todas las transformaciones locales (limpieza de fillers, conversión de
  script chino, traducción), para que una regla siempre vea el texto final.
  Cada regla que dispara incrementa su contador, que es la base de la vista
  «términos más usados».

### 4.5 Comandos IPC

`list_dictionary_entries`, `add_dictionary_entry`, `update_dictionary_entry`,
`delete_dictionary_entry`, `set_dictionary_entry_enabled`,
`import_dictionary_entries` (devuelve cuántas se añadieron de verdad; las
duplicadas no son un fallo), `get_dictionary_vocabulary` y
`apply_dictionary_substitutions` (para previsualizar una regla antes de
activarla).

### 4.6 Interfaz

Sección nueva «Diccionario» en la barra lateral, con: buscador, filtro por
tipo, lista de entradas (término, sustitución, idioma, contador de usos,
interruptor de activación, editar y borrar), alta y edición con validación,
importación/exportación JSON y un **visor de previsualización** que pasa una
frase por el motor y muestra el resultado.

---

## 5. Bloque 2B.4 — Provider MiniMax

### 5.1 Por qué MiniMax necesita su propio módulo

Dos rarezas de la API de MiniMax rompen silenciosamente el texto del
usuario si se manejan como un OpenAI-compatible cualquiera:

1. **El error viaja dentro de un HTTP 200.** El sobre `base_resp` trae
   `{ status_code, status_msg }`, y `status_code == 0` significa éxito. Un
   cliente que sólo mira el status HTTP trata los fallos como éxitos y
   escribe basura en el campo del usuario. Códigos relevantes: `1002` rate
   limit, `2056` cuota del plan agotada, `1008` saldo insuficiente.
2. **El razonamiento llega separado de la respuesta.** En streaming
   (`thinking_delta` frente a `text_delta`) el razonamiento **nunca** debe
   insertarse en el texto. `MiniMax-M3.1-Flash` razona siempre y devuelve
   HTTP 400 si se manda `thinking: disabled` o `reasoning_effort: none`;
   `MiniMax-M3` razona apagado por defecto, que es lo que pide una limpieza
   de transcripción.

### 5.2 El módulo

`src-tauri/src/minimax.rs` — puro, sin red, sin cliente HTTP:

| API | Qué hace |
|---|---|
| `parse_chat_completion` | Lee `base_resp.status_code` **siempre**, aunque el HTTP sea 200; devuelve el texto descartando el razonamiento |
| `parse_anthropic_messages` | Concatena sólo los bloques `text`, salta los bloques `thinking` |
| `parse_sse_data_line` | Distingue `Text` de `Thinking` de `Done` en una línea SSE |
| `build_request_body` | Cuerpo OpenAI (`reasoning_effort`) o Anthropic (`output_config.effort`), **nunca** `reasoning_effort: none` |
| `MiniMaxError::is_retryable` | Sólo el rate limit es reintentable |
| `classify_http_error` | Traduce un status HTTP en el error de dominio |

25 pruebas, todas con fixtures JSON en línea, sin red. **Cero llamadas
reales**: el módulo no abre sockets.

El provider se registró en `default_post_process_providers()` con
`id: "minimax"`, `base_url: https://api.minimax.io/v1`,
`api_format: OpenAi` y modelo por defecto `MiniMax-M3` (el de razonamiento
apagado).

### 5.3 Lo que falta: el cableado al pipeline

El módulo **no está conectado** al post-procesado. Motivo honesto: `llm_client.rs`
ya tiene su propio cliente HTTP, su propio descubrimiento de formato y su
propia lectura de la respuesta; enchufar el parser de MiniMax ahí exige
decidir si MiniMax es *un formato más* del cliente genérico o *un cliente
propio*. La primera opción es una línea; la segunda, un refactor del cliente.

Lo que sí quedó hecho: la configuración del provider (clave, base URL,
modelo, `effort`), el registro en la lista de providers, el parser probado y
el mapeo de errores. El resto es un trabajo de integración focalizado sobre
`llm_client.rs`, no de diseño.

### 5.4 Credenciales

Las claves de API de los providers se guardan en el store de ajustes
(`post_process_api_keys`), que es un `SecretMap` y **no** se escribe en el
log. No se almacenamiento externo adicional en esta fase.

---

## 6. Bloque 2B.5 — Rendimiento

La medición de memoria en modo bandeja ya existía
(`scripts/measure-dixi-tree.ps1`), y distingue correctamente **RAM privada**
de *working set*, e incluye el árbol completo de procesos (app + WebView2 +
worker de transcripción). Se reutilizó sin cambios.

Las optimizaciones de memoria que proponía la fase anterior (descarga
anticipada del modelo, cierre perezoso del stream) **ya están implementadas**
en el código heredado (`model_unload_timeout`, `lazy_stream_close`), con sus
respectivos ajustes en la UI.

Lo que **no** se midió en esta fase: el consumo real con el modelo
descargado y una transcripción en curso. Requiere el modelo en disco y una
sesión de audio; la máquina no estaba en condiciones de repetir la medición
de Fase 2A sin competir por RAM con la compilación.

---

## 7. Verificación

### 7.1 Qué se ejecutó y qué salió

| Comprobación | Comando | Resultado |
|---|---|---|
| Pruebas unitarias Rust | `cargo test --features custom-protocol --lib` | **365 PASS**, 0 FAIL, 18 ignoradas |
| Doctests Rust | `cargo test --features custom-protocol --doc` | **1 PASS**, 0 FAIL |
| Pruebas de frontend | `npm run test:unit` | **3 guiones, todas las aserciones pasan** |
| Tipos del frontend | `npx tsc --noEmit` | **0 errores** |
| Lint del frontend | `npx eslint src/components/settings/dictionary src/components/Sidebar.tsx` | **exit 0**, sin avisos |
| Build del frontend | `npm run build` | **exit 0**, 2118 módulos, 18,04 s |
| Paridad de traducciones | `node scripts/check-translations.ts` | **✓ 25 idiomas completos** |
| Dataset de evaluación | validación JSON línea a línea | **24/24 líneas JSON válidas** (es 16, en 4, es-en 4) |
| Regresión `custom-protocol` | `npm run check:release-protocol` | **OK** (ver §7.2) |

El recuento de pruebas **sube de 304 (Fase 2A) a 366**: 61 pruebas nuevas
por el diccionario, el módulo MiniMax y el harness de evaluación, más el
doctest del harness.

### 7.2 La regresión de `tauri/custom-protocol`, en detalle

```
[OK  ] Cargo.toml declara custom-protocol -> tauri/custom-protocol
[OK  ] dist/index.html existe
[OK  ] dist/index.html referencia recursos con hash - 3 recurso(s)
[OK  ] El binario release existe - D:\Codigo\Dixi\src-tauri\target\release\dixi.exe
[OK  ] El binario incrusta los recursos del frontend (custom-protocol activo) - incrustados: 3/3
[OK  ] El binario arranca y regenera los bindings

OK: el binario de produccion lleva el frontend incrustado.
```

La sexta comprobación es la de humo (`-SmokeTest`): el binario de release
**arranca** y regenera los bindings IPC. Tras hacerlo,
`git diff src/bindings.ts` quedó **vacío**: el fichero generado es
reproducible byte a byte desde el binario de producción. Ése es el
resultado que hace útil la herramienta: el contrato IPC entre Rust y el
frontend se puede regenerar y comprobar sin tocar una línea a mano.

Binario de producción de esta fase: `dixi.exe`, 31 884 288 bytes,
compilado en 9 min 10 s con `cargo build --release --features custom-protocol`.

### 7.3 Un defecto de Fase 2A que sólo apareció al regenerar los bindings

Al regenerar `src/bindings.ts` desde el registro de comandos de Rust
aparecieron dos funciones que el frontend ya no encontraba:

```
src/components/settings/DixiKeysShortcutInput.tsx(66,20): error TS2551:
  Property 'stopDixiKeysRecording' does not exist ...
  Did you mean 'stopHandyKeysRecording'?
```

**Causa.** Durante el renombrado de marca de la Fase 2A, `src/bindings.ts`
—un fichero generado— se editó **a mano** para renombrar dos funciones de
TypeScript (`startHandyKeysRecording` → `startDixiKeysRecording`), pero los
comandos de Rust siguieron llamándose `start_handy_keys_recording` y
`stop_handy_keys_recording`. Las cadenas de invocación (`TAURI_INVOKE`)
**no** se tocaron, así que en ejecución nunca hubo rotura: era un desajuste
de nombres entre la capa de Typescript y la de Rust, invisible en
ejecución yVisible sólo al tipar.

**Por qué importa.** Un fichero de bindings editado a mano deja de
regenerarse sin perder cambios. Eso convertía el nuevo
`--export-bindings` en una operación peligrosa: regenerar habría revertido
el renombrado y roto el frontend.

**Corrección.** Se renombraron los dos comandos de Rust a
`start_dixi_keys_recording` / `stop_dixi_keys_recording`, de modo que el
generador produce exactamente los nombres que el frontend espera y
`src/bindings.ts` vuelve a ser un artefacto reproducible. Se eliminaron
también la copia local de los tipos del diccionario y el `cast` que la
interfaz usaba para hablar con comandos que todavía no existían en los
bindings: ahora la interfaz consume los tipos generados.

Lo que **no** se renombró, a propósito:

- El módulo interno `shortcut::handy_keys` y el valor del ajuste
  `KeyboardImplementation::HandyKeys`. Este último **está persistido en el
  store de ajustes de los usuarios**: renombrarlo rompería la instalación
  existente de cualquiera que tenga esa opción configurada. Es un nombre
  interno y el de un valor de configuración, no parte del contrato IPC
  visible.

### 7.4 Limitaciones de esta verificación

- **La medición de memoria en modo bandeja no se repitió.** Requiere el
  modelo descargado y una sesión de audio reales, y la máquina estaba
  ejecutando la compilación de release. La herramienta
  (`scripts/measure-dixi-tree.ps1`) está lista y sin cambios.
- **No se probó el flujo real de la interfaz**: la verificación de frontend
  es de tipos, lint, build y paridad de traducciones. No se ejecutó la
  aplicación con la WebView2.
- **No se hicieron llamadas de red.** El módulo MiniMax se validó con
  fixtures; ningún provider real se contactó.
- **El runner de evaluación end-to-end no existe**: el dataset y el harness
  están listos, pero falta el bucle que genera las hipótesis.
- El aviso `unused import: Emitter` en `src/secure_input.rs` es
  **preexistente** de la Fase 2A: `Emitter` sólo se usa dentro de un
  módulo `#[cfg(target_os = "macos")]`. No se tocó por no ser de esta
  fase.

---

## 8. Archivos

### Rust (nuevos)
- `src-tauri/src/managers/dictionary.rs` — diccionario: modelo, SQLite, migraciones, motor de sustitución.
- `src-tauri/src/commands/dictionary.rs` — comandos IPC del diccionario.
- `src-tauri/src/minimax.rs` — parser y cliente MiniMax abstracto (sin red).
- `src-tauri/src/dixi/evaluation.rs` + `evaluation/tests.rs` — harness WER/CER.
- `src-tauri/src/release_contract.rs` — invariantes de la compilación de producción.

### Rust (modificados)
- `src-tauri/src/autostart.rs` — guardia de estado.
- `src-tauri/src/shortcut/mod.rs` — ids de prompt sin colisión; comando de contexto.
- `src-tauri/src/settings.rs` — `transcription_context`, provider MiniMax, modelo por defecto.
- `src-tauri/src/managers/transcription.rs` — prompt inicial, vocabulario del diccionario, reglas de sustitución.
- `src-tauri/src/lib.rs` — registro de módulos, comandos y estado; export de bindings.
- `src-tauri/src/cli.rs` — `--export-bindings`.
- `src-tauri/src/commands/mod.rs`, `src-tauri/src/managers/mod.rs`, `src-tauri/src/dixi/mod.rs` — módulos.

### Frontend
- `src/components/settings/dictionary/**` — sección del diccionario.
- `src/components/Sidebar.tsx`, `src/components/settings/index.ts` — registro de la sección.
- `src/i18n/locales/*/translation.json` — 26 locales.
- `src/stores/settingsStore.ts` — desenmascarado de errores de comando; contexto de transcripción.
- `src/bindings.ts` — regenerado.

### Scripts y datos
- `scripts/verify-release-protocol.ps1` — regresión `tauri/custom-protocol`.
- `scripts/split-diff.mjs` — divide el diff de un fichero en hunks y aplica
  un subconjunto; sustituye al `git add -p` interactivo.
- `docs/validation-phase2b/dataset/dataset.jsonl` — dataset de evaluación.

### Generado
- `src/bindings.ts` — **regenerado** desde el registro de comandos de Rust
  con `dixi.exe --export-bindings`. Ya no lleva ediciones a mano.
