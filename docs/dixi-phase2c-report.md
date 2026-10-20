# Dixi — Informe técnico de Fase 2C

**Alcance:** perfiles y estilos de escritura, fragmentos (snippets) con variables
dinámicas, transforms con IA, diff por palabras, deshacer e historial local, y
el cierre de los defectos que quedaban de la Fase 2B.
**Rama:** `feat/dixi-productivity` (base: `develop` @ `9904624`, cierre de Fase 2B).
**Fecha:** 2026-10-20.

---

## 1. Resumen ejecutivo

| Objetivo | Estado | Qué se entregó |
|---|---|---|
| Perfiles y estilos de escritura | **DESARROLLADO** | Seis estilos y cuatro perfiles integrados, detección de la aplicación en primer plano, precedencia de cuatro niveles, reglas de formato deterministas y migración de instalaciones existentes. |
| Fragmentos con variables dinámicas | **DESARROLLADO** | `{fecha}`/`{hora}` locales, variables preguntadas al usuario, colisiones de disparador con resolución determinista, sin recursión, por aplicación y por idioma, favoritos, contador de usos, import/export. |
| Transforms con IA | **DESARROLLADO** | Catorce transforms de serie y los propios, reutilizando el pipeline de MiniMax (mismo cliente HTTP, mismo `base_resp`, mismo razonamiento descartado), con el texto original intacto ante cualquier fallo. |
| Diff, deshacer e historial | **DESARROLLADO** | Diff por palabras, historial local con retención, y el flujo de seleccionar → transformar → ver el diff → aceptar. |
| Selección de otra aplicación | **DESARROLLADO** | Lectura y reemplazo con verificación previa, campos de contraseña jamás tocados y portapapeles siempre restaurado. |
| Secretos en disco | **CORREGIDO** (Fase 2B) | DPAPI, con migración forzada al leer. |
| Lint del proyecto | **CORREGIDO** (Fase 2B) | Los artefactos generados quedan fuera; los errores reales del código fuente, arreglados. |

### Lo que NO se hizo, y por qué

1. **El aprendizaje automático del diccionario.** La referencia (doc 04) dice
   que el 81 % de las entradas de Wispr se aprenden de las ediciones
   posteriores del usuario en la app destino. Eso exige observar la edición
   en la app enfocada — Windows UIAutomation — y correlacionarla con el
   dictado. Es un proyecto aparte, con su propio riesgo de privacidad, y no
   cabía en esta fase.
2. **Las transformaciones semánticas locales.** Por indicación explícita, las
   reglas locales son **mecánicas** (mayúsculas, punto final, espacios,
   puntuación). Cualquier cosa que necesite entender el significado es un
   transform, y necesita un provider LLM **habilitado explícitamente**.
3. **Sin llamadas reales a MiniMax.** Toda la validación es con un servidor
   HTTP local simulado. No se consumió cuota real.
4. **El perfil no fuerza todavía el idioma.** El campo `language` del perfil
   existe y se filtra, pero el motor no recibe un idioma forzado por perfil:
   el idioma se sigue resolviendo con el modelo.

---

## 2. La decisión estructural: un solo almacén

Perfiles, fragmentos, transforms e historial **no son un sistema aparte**: un
perfil cambia cómo se aplica el diccionario y cómo se expanden los fragmentos.
Son facetas del mismo objeto.

Por eso `PersonalStore` es **un único fichero SQLite** (`dixi.db`) con las
migraciones versionadas de todo, en lugar de cuatro bases y cuatro juegos de
migraciones. El diccionario se apoya en el mismo almacén y conserva su API
pública: sus once pruebas de extremo a extremo siguen valiendo sin cambios.

El motor de emparejamiento también es uno (`text_engine`): el emparejamiento
más largo, el desempate por prioridad, la conservación de mayúsculas y la
regla de una sola pasada. El diccionario lo usa, los fragmentos lo usan, y las
reglas de formato de un perfil son mecánicas, no semánticas.

**Un defecto real que esto destapó:** el desempate por prioridad del motor
estaba mal — comparaba contra la regla equivocada. Lo detectaron las pruebas
al aislar el motor; con el algoritmo duplicado dentro del diccionario, el
error habría quedado escondido.

---

## 3. Perfiles y estilos de escritura

### 3.1 El modelo

Un **estilo** dice *cómo* escribir (formal, casual, entusiasta…). Un
**perfil** dice *dónde* (un chat, un correo, un mensaje personal) y apunta a
un estilo, un nivel de limpieza y un conjunto de reglas de formato.

Seis estilos y cuatro perfiles vienen de serie. Los personalizados se crean,
editan y borran; los de serie solo se desactivan.

### 3.2 La detección de la aplicación

Windows aporta la ventana en primer plano, el proceso que la posee y su
nombre de ejecutable. Se lee **lo mínimo**: el nombre del proceso y,
opcionalmente, el título de la ventana. Ningún contenido, ninguna URL, nada
del portapapeles. Un perfil nunca depende del título, así que un título que
filtre un mensaje nunca llega a una regla.

### 3.3 La precedencia

De menos a más específico:

1. la configuración global,
2. el perfil que corresponde a la aplicación detectada,
3. una regla atada a esa aplicación,
4. la anulación manual de la sesión.

La anulación manual **nunca se persiste**: es una decisión de esta sesión.

### 3.4 Las reglas de formato

Mecánicas, y por eso seguras sin modelo: capitalizar las frases, punto
final, espacios duplicados, espacio antes de la puntuación. Se aplican al
final, sobre el texto ya limpio.

El nivel de limpieza del perfil decide si se quitan las muletillas.

---

## 4. Fragmentos con variables dinámicas

### 4.1 Lo que los diferencia

Un fragmento no es un término del diccionario (una pista de vocabulario) ni
una sustitución (una reescritura): es un texto reutilizable que se escribe
cuando se dicta su disparador.

Lo que lo diferencia de Wispr Flow son las **variables dinámicas**. La
expansión lleva marcadores con llaves:

- `{fecha}` y `{hora}` se resuelven **en local**, sin preguntar nada.
- Cualquier otra variable sin valor **se pregunta al usuario**. Si es
  obligatoria, el fragmento **no se expande**: el disparador se queda en el
  texto. Nada se pega a medias.

### 4.2 Las reglas de las semillas

- **Colisiones de disparador.** La resolución es determinista: gana el
  disparador más largo, luego el de mayor prioridad, y por último el
  alfabéticamente menor. Hay una función que devuelve las reglas que
  compiten, para que la orden sea verificable.
- **Sin recursión.** El motor hace una sola pasada, así que un fragmento que
  contiene su propio disparador no se reescribe. Hay una prueba que lo
  demuestra.
- **Sin activación accidental.** Un disparador de palabra completa no
  dispara dentro de `miDisparador` ni de `mi_disparador`.
- **Por aplicación y por idioma.** Un fragmento con aplicaciones solo
  funciona en ellas; uno con idioma, solo en ese idioma (`es` cubre
  `es-AR`).

---

## 5. Transforms con IA

Catorce de serie: mejorar la redacción, corregir la ortografía, resumir,
acortar, expandir, formalizar, casual, convertir en lista, traducir,
explicar, eliminar muletillas, convertir en tabla, extraer tareas y título.

Lo importante es que **no hay un segundo pipeline**: cada transform pasa por
`send_chat_completion_with_schema`, el mismo camino que el posprocesado, con
el mismo cliente HTTP y la misma corrección de MiniMax (el sobre `base_resp`
se lee aunque el HTTP devuelva 200, y el razonamiento nunca llega al
resultado).

**El texto original se conserva en todos los fallos**: si el provider falla,
se agota la cuota, se cae la conexión o la respuesta llega vacía, el
resultado es el texto de entrada y el motivo va aparte. Nunca se pega una
respuesta parcial como si fuera una transformación.

---

## 6. Diff, deshacer e historial

El diff es **por palabras**: por carácter es ilegible en prosa, y por
párrafo esconde los cambios. Cuenta palabras, no tokens.

El historial guarda el texto anterior y el resultado de cada cambio que Dixi
hizo. Solo se guarda lo que Dixi produjo o lo que Dixi sustituyó, nunca el
contenido de una aplicación leído por su cuenta. La retención por defecto es
de 30 días, configurable, y el historial se puede vaciar cuando quieras.

---

## 7. La selección en otra aplicación

Es la parte con riesgo real, y por eso tiene garantías:

- **Un campo de contraseña jamás se lee ni se escribe.** Se rechaza *antes*
  de copiar nada, por el nombre de la clase que lo contiene o por el bit
  `ES_PASSWORD` de un `Edit`.
- **El portapapeles se restaura siempre**, incluso cuando la operación
  falla: lo devuelve un guardia en su `Drop`. El portapapeles del usuario
  queda exactamente como estaba.
- **Antes de escribir se verifica** que la ventana y el control con el foco
  siguen siendo los mismos, y que la selección actual sigue siendo la que
  se leyó. Si algo cambió, no se escribe nada y se ofrece copiar el
  resultado.

El portapapeles se usa porque es la única vía que funciona con controles que
no son de Windows (Chromium, Word, los editores de Electron). Y se
restituye, así que no queda pisado.

Las dos funciones de Windows que hacen falta no están en las características
habilitadas del crate `windows`, y activarlas obligaría a recompilar una
librería generada enorme que **no entraba en memoria** en esta máquina
(el `rustc` murió con OOM). Se declaran a mano con `extern "system"`, y una
prueba comprueba que la estructura coincide con la de Win32.

---

## 8. La revisión independiente: ocho defectos reales

La interfaz de la fase 2C se revisó contra el backend, contrato a contrato, en
lugar de confiar en que «compila». Salieron **ocho defectos reales**, y uno de
ellos era grave.

### 8.1 El grave: «Aceptar» y «Revertir» no escribían nunca

`replace_text_selection` recibía el texto, la ventana y el texto nuevo, pero
el handle del control con el foco venía **hardcodeado a `0`**. La comprobación
que se niega a escribir sobre una selección que ha cambiado compara también
el control, y un control real nunca vale `0`: la comparación fallaba siempre,
la respuesta era siempre «la selección cambió» y **nada se escribía jamás**.

La firma ahora lleva el control (`expected_control`), la interfaz lo pasa, y
hay una prueba que fija que el handle se conserva.

### 8.2 Los siete restantes

| Severidad | Defecto | Corrección |
|---|---|---|
| Alto | Editar un fragmento borraba su marca de favorito, y con ella su prioridad de resolución. | El formulario conserva `is_favorite` y lo envía. |
| Medio | Un transform fallido era indistinguible de uno correcto: se mostraba el texto previo, con diff, y «Aceptar» seguía activo. | El fallo limpia resultado y diff; «Aceptar» queda deshabilitado. |
| Medio | El desplegable de proveedor decía «por defecto» mientras se guardaba un identificador obsoleto. | El proveedor guardado se sigue mostrando aunque ya no esté en la configuración. |
| Medio | Declarar una variable llamada `fecha` u `hora` se guardaba y luego se ignoraba en silencio. | El backend las rechaza: son variables reservadas. |
| Bajo | Cada guardado de un transform escribía `shortcut: null` y borraba el atajo. | El atajo se conserva y se envía. |
| Bajo | La vista previa de fragmentos expandía fragmentos restringidos a otra aplicación. | **No se cambió**, por las razones de 8.3. |
| Bajo | La retención del historial aceptaba `1e3` o `12abc` como días. | Se exige un entero no negativo. |

### 8.3 La vista previa que se dejó como estaba

La vista previa de fragmentos ignora el ámbito de aplicación, porque el
backend trata «aplicación desconocida» como «no se puede descartar». La
sugerencia de la revisión era pasarle la aplicación que tiene el foco, pero
**durante la vista previa esa aplicación es Dixi**, así que la sugerencia
habría roto la vista previa de todos los fragmentos. Se dejó el
comportamiento, que es el documentado, y no se aplicó el cambio.

---

## 9. Verificación

### 9.1 Lo que se comprobó

| Comprobación | Cómo | Resultado |
|---|---|---|
| Pruebas de la librería | `cargo test --features custom-protocol --lib` | **524 PASS**, 0 FAIL, 18 ignoradas |
| Doctests | `cargo test --features custom-protocol --doc` | PASS |
| Avisos del compilador | la misma compilación | **0** (antes quedaban 2) |
| Tipos del frontend | `tsc --noEmit` | 0 errores |
| Lint del proyecto entero | `eslint src` | 0 errores, 0 avisos |
| Pruebas unitarias del frontend | `test:unit` | PASS |
| Paridad de traducciones | `check:translations` | PASS, 26 idiomas |
| Calidad del texto traducido | `check-translation-text` (nuevo) | 210 claves × 26 idiomas, 6705 cadenas, 0 problemas |
| Binario de producción | `cargo build --release --features custom-protocol` | 33.508.864 bytes |
| Frontend incrustado en el binario | `verify-release-protocol` | PASS |
| Consumo de memoria y CPU | 5 condiciones + motor aislado | ver 9.2 |

**524 pruebas** son 3 más que en la 2B: las tres que fijan los arreglos de
esta fase (el handle del control con el foco, el rechazo de las variables
reservadas y la resolución de `{fecha}` sin declararla).

### 9.2 El consumo de memoria

Medido sobre una instancia aislada, con la misma metodología que la 2B:

| Condición | 2B privada | 2C privada | Δ |
|---|---|---|---|
| Ventana abierta | 249,6 MB | 263,3 MB | +13,7 MB |
| Solo en la bandeja | 293,6 MB | 258,4 MB | −35,2 MB |
| Modelo descargado | 286,6 MB | 244,9 MB | −41,7 MB |
| Modelo cargado | 243,4 MB | 248,8 MB | +5,4 MB |
| Transcripción activa | 268,5 MB | 260,4 MB | −8,2 MB |
| **Motor con el modelo cargado (pico)** | **506,3 MB** | **505,7 MB** | **−0,6 MB** |

La lectura honesta es que **la fase 2C no cuesta memoria**: el pico del
motor es el mismo, y las diferencias de las cinco condiciones cambian de
signo, así que están dentro del ruido de la muestra (el informe de la 2B ya
lo había medido). Tampoco se puede leer la CPU como una mejora: depende de
la carga de la máquina en ese momento y el motor no se ha tocado.

El detalle, con el desglose por proceso, está en
`docs/validation-phase2c/performance/informe.md`.

### 9.3 Lo que queda por hacer a mano

Dos cosas no se pueden comprobar sin una ventana de Windows delante: que la
interfaz muestre lo que toca, y que el flujo de *seleccionar → transformar →
aceptar* escriba de verdad en la aplicación de destino. La lista está en
`docs/validation-phase2c/ACCEPTACION_MANUAL.md` y tarda unos diez minutos.

### 9.4 Cómo se ejecutó todo

Todo se ejecutó **sin abrir una ventana de consola**, con
`scripts/run-hidden.ps1`, que lanza el proceso con `-WindowStyle Hidden` y
deja la salida y el error en ficheros de log. La única ventana que se abrió
fue la de Dixi durante la condición «ventana abierta» de la medición, que
es justamente lo que se estaba midiendo.

---

## 10. Archivos

### Rust (nuevos)
- `src-tauri/src/managers/personal_store.rs` — el almacén compartido.
- `src-tauri/src/managers/text_engine.rs` — el motor de emparejamiento común.
- `src-tauri/src/managers/profiles.rs` — estilos, perfiles y su precedencia.
- `src-tauri/src/managers/app_detection.rs` — detección de la aplicación.
- `src-tauri/src/managers/snippets.rs` — fragmentos y sus variables.
- `src-tauri/src/managers/transforms.rs` — catálogo de transforms.
- `src-tauri/src/managers/diff.rs` — diff por palabras.
- `src-tauri/src/managers/text_history.rs` — historial local de cambios.
- `src-tauri/src/managers/selection.rs` — lectura y reemplazo de la selección.

### Rust (modificados)
- `src-tauri/src/managers/dictionary.rs` — usa el almacén y el motor comunes.
- `src-tauri/src/managers/transcription.rs` — perfil, fragmentos y reglas de formato en el pipeline.
- `src-tauri/src/lib.rs` — registro de módulos, comandos y estado.
- `src-tauri/src/settings.rs` — migración de las claves en claro.
- `src-tauri/src/commands/*` — los comandos nuevos.

### Frontend
- `src/components/settings/profiles/**`, `snippets/**`, `transforms/**`, `textHistory/**`.
- `src/components/Sidebar.tsx`, `src/components/settings/index.ts`.
- `src/i18n/locales/*/translation.json` — 26 idiomas.
- `src/bindings.ts` — regenerado.

### Scripts
- `scripts/run-hidden.ps1` — ejecución sin ventana visible, con log.
- `scripts/check-translation-text.mjs` — claves presentes en los 26 idiomas,
  sin daño de codificación y sin mezclas de alfabetos.

### Medición
- `docs/validation-phase2c/performance/` — muestras e informe de consumo.
