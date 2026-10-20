# Hotfix: la captura de la selección de otra aplicación

**Rama:** `fix/dixi-selection-capture` (desde `develop` @ `0647897`)
**Estado:** implementado y verificado en automático; la prueba sobre el
escritorio real la hace el usuario con el procedimiento manual.
**No fusionado a `develop`.**

---

## 1. El diagnóstico

El botón «Usar el texto seleccionado» fallaba **siempre** con «No hay texto
seleccionado en la aplicación que tiene el foco». La causa no era la
captura: era el **foco**.

La captura de una selección en otra aplicación solo es posible mientras esa
aplicación tiene el foco, porque Windows solo expone la selección del
control que tiene el **foco de teclado** dentro de la ventana que está en
primer plano. Al pulsar un botón de Dixi, la ventana de Dixi pasa a primer
plano y recibe el foco. En el instante en que el backend va a leer la
selección, el control con el foco ya no es el del navegador: es el de Dixi.

La lectura de la selección no puede hacerse de otra forma: la única vía
general que hay en Windows para leer el texto seleccionado de un control
arbitrario es pedir una copia (`Ctrl+C`) y leer el portapapeles. UIAutomation
podría leer el texto de un control sin copiar, pero no funciona con el
control de un renderizador de navegador, que es donde vive el texto de un
documento.

Es decir: **el botón no podía funcionar por diseño**, no por un fallo
puntual. La única forma de capturarlo sin perder el foco es hacerlo desde
algo que no lo robe.

## 2. La solución

### 2.1 La captura: un atajo global

Se añade el atajo global **`capture_selection`** (por defecto
`Ctrl+Alt+C`, configurable en Ajustes → General → Atajos). Un atajo global
**no quita el foco**: la aplicación de origen sigue siendo la que está en
primer plano y con el foco de teclado mientras Dixi lee su selección.

El atajo:

1. comprueba que el control con el foco no es un campo de contraseña,
2. comprueba que es un control de texto conocido,
3. pide la copia y lee el texto,
4. guarda el **contexto** de la selección: el texto, la ventana, el control
   y el nombre de la aplicación,
5. avisa a la interfaz con el evento `selection-captured`.

Como alternativa, el panel ofrece un botón con **cuenta atrás de 4
segundos**: al pulsarlo, el usuario vuelve a la aplicación de origen,
selecciona el texto, y al terminar la cuenta atrás Dixi captura (ya con la
aplicación de origen en primer plano).

No hay captura continua, ni vigilancia del teclado, ni lectura de ventanas
arbitraria, ni vigilancia del portapapeles: la captura ocurre **solo** ante
una acción explícita del usuario.

### 2.2 El contexto: lo que hace posible escribir después

La selección por sí sola no sirve para escribir: en el momento de aceptar,
lo único que se podría hacer es pegar en lo que tenga el foco, y eso nunca
es lo que el usuario quiere. Por eso la captura guarda el **contexto**:

| Dato | Para qué |
|---|---|
| `text` | Saber qué se leyó, y contra qué comparar antes de escribir. |
| `window` | Saber **dónde** escribir, y devolver esa aplicación al frente. |
| `control` | Comprobar que sigue siendo el mismo control. |
| `app_name` | Mostrarle al usuario de dónde salió el texto. |
| `captured_at` | Saber cuándo se leyó. |

El historial de cambios guarda el mismo contexto (`window_handle` y
`control_handle` en la tabla `text_history`), y por eso un cambio se puede
deshacer en su sitio y no en cualquier otro.

### 2.3 «Aceptar»: cómo vuelve a la aplicación de origen

El botón «Aceptar» es una **acción explícita del usuario**, y a partir de
ahí la escritura se hace con estas comprobaciones, en este orden. Si alguna
falla, **no se escribe nada**:

1. **La ventana existe** (`IsWindow`). Si la aplicación se cerró, se
   cancela.
2. **La ventana vuelve al frente** (`SetForegroundWindow`, y `SW_RESTORE`
   si estaba minimizada). Se espera a que el foco llegue (hasta 800 ms). Si
   Windows no lo permite, se cancela: no se escribe en otra ventana.
3. **El control con el foco es el mismo** que el capturado. Si no, se
   cancela.
4. **El control no es un campo protegido** (ver 2.5).
5. **La selección es la misma**: se vuelve a leer el texto seleccionado y se
   compara con el que se capturó. Si el usuario escribió o movió la
   selección mientras tanto, se cancela.
6. **Se escribe** el texto nuevo, con el portapapeles protegido (ver 2.4).
7. **Se comprueba la escritura**: el control tiene que contener el texto
   nuevo, o —en los controles que no responden con su texto, como los
   renderizadores de navegador— la selección tiene que haber desaparecido,
   que es lo que deja una pulsación de «pegar».

El cambio se registra después en el historial **con su contexto**, para que
se pueda deshacer.

### 2.4 El portapapeles: la garantía real

Antes, el portapapeles se vaciaba y se rellenaba solo con texto: una
imagen, unos archivos copiados, HTML o RTF **se perdían sin avisar**.

Ahora:

- Se enumeran los formatos que tiene el portapapeles.
- Solo se toca si tiene texto: `CF_TEXT`, `CF_UNICODETEXT` y `CF_LOCALE`.
  Los tres se guardan (el texto Unicode, los bytes ANSI y el identificador
  de locale) y se **vuelven a poner tal cual**, con el mismo formato.
- **Si tiene cualquier otra cosa** —una imagen, archivos, HTML, RTF—, la
  operación **se rechaza antes de modificar nada**, con el aviso
  `clipboard_not_preservable`. El portapapeles del usuario no se toca.

La consecuencia, dicha sin rodeos: **Dixi necesita que el portapapeles
tenga texto (o esté vacío) para trabajar con una selección externa**. Si
el usuario tiene una imagen copiada, la captura se rechaza y se le pide que
vacíe el portapapeles. Es la única forma de prometer que el portapapeles no
se daña: restaurar formatos arbitrarios con exactitud exigiría el objeto
OLE del portapapeles, que queda fuera del alcance de este arreglo.

Sobre el pegado: después de la pulsación se espera 400 ms antes de
restaurar el portapapeles, y **la escritura se comprueba después de
restaurarlo**. Si la aplicación no llegó a tomar el texto, la operación lo
dice, en lugar de darlo por bueno.

### 2.5 Los campos protegidos

Un campo de contraseña se rechaza **antes** de pedir la copia, con dos
señales:

- la **clase** del control: `Password`, `PasswordBox` o cualquier clase que
  contenga «password»;
- el **estilo** `ES_PASSWORD` (0x0020), que solo se mira en los controles
  de edición: en un botón o en una lista el mismo bit significa otra cosa,
  así que mirarlo allí rechazaría controles que no están protegidos.

Además, y esto es nuevo, **solo se leen los controles que Dixi conoce**:

| Clase | Qué es |
|---|---|
| `Edit`, `TEdit` | Controles de edición nativos. |
| `RichEdit`, `RichEdit20W`, `RichEdit50W`, `RichEditD2DPT` | Controles enriquecidos. |
| `Chrome_WidgetWin_1` | El renderizador de los navegadores basados en Chromium. |
| `MozillaWindowClass` | Firefox. |

Cualquier otra clase se rechaza con `unsupported_control`. La razón es
que Windows no permite saber de forma fiable si un control cualquiera
contiene un campo protegido sin Windows UI Automation, y un control
desconocido puede ser de una aplicación de terceros, o no admitir una
pulsación sintética.

**Lo que no se ha hecho:** usar UIAutomation y su propiedad `IsPassword`.
Las características del crate `windows` que hacen falta para eso obligarían
a recompilar una biblioteca generada enorme, y en esta máquina esa
compilación agota la memoria. La lista blanca de controles es la
alternativa que el propio diseño admite, y su límite se documenta más
abajo.

## 3. Controles y aplicaciones compatibles

| Aplicación | Control | Lectura | Escritura |
|---|---|---|---|
| Bloc de notas (Windows 10/11) | `RichEditD2DPT` / `Edit` | Sí | Sí |
| Navegadores con Chromium (Chrome, Edge, Opera, VS Code) | `Chrome_WidgetWin_1` | Sí | Sí |
| Firefox | `MozillaWindowClass` | Sí | Sí |
| WordPad, editores nativos | `RichEdit20W` | Sí | Sí |
| Campos de contraseña | cualquiera | **No** | **No** |
| Controles desconocidos | cualquiera | **No** | **No** |

## 4. Limitaciones, dichas sin adornos

1. **El portapapeles tiene que tener texto.** Con una imagen copiada, la
   captura se rechaza. Es el precio de no destruir el contenido del
   usuario.
2. **Un campo de contraseña dentro de un navegador no se puede detectar**
   por Win32: el renderizador no lo expone. La lectura sigue siendo segura
   porque los navegadores no entregan el contenido de un campo enmascarado
   (la copia sale vacía), pero la escritura sí podría llegar a un campo
   enmascarado de una página. UIAutomation resolvería esto y queda como
   mejora pendiente.
3. **La vuelta a la aplicación de origen depende de Windows.** Si el
   sistema no permite traerla al frente en 800 ms, la operación se cancela
   y se ofrece copiar. No se escribe en ninguna otra ventana.
4. **La captura necesita el atajo global registrado.** Si el atajo entra en
   conflicto con otra aplicación, hay que cambiarlo en Ajustes → General.
5. **El atajo por defecto (`Ctrl+Alt+C`) es una propuesta.** Si Wispr Flow u
   otra aplicación ya lo usa, hay que moverlo. No se ha tocado ninguna
   configuración de Wispr Flow.

## 5. Pruebas

### 5.1 Automáticas (Rust)

**539 PASS, 0 FAIL, 18 ignoradas** (eran 524 antes de este arreglo). Las 15
nuevas cubren lo que se puede decidir sin escritorio:

| Prueba | Qué fija |
|---|---|
| `only_the_verified_text_controls_are_read` | La lista de controles que se leen. |
| `an_unknown_control_is_not_read` | Un control desconocido no se lee. |
| `the_class_is_matched_without_case_and_without_spaces` | La comparación de clases. |
| `the_password_style_bit_is_detected` | El bit `ES_PASSWORD`. |
| `the_password_class_is_detected` | Las clases de contraseña. |
| `an_edit_control_with_the_password_style_is_refused` | El campo enmascarado se rechaza. |
| `the_style_bit_alone_does_not_refuse_other_controls` | El bit no se malinterpreta. |
| `a_clipboard_with_only_text_can_be_restored` | Los formatos que se pueden restaurar. |
| `a_clipboard_with_something_else_is_refused` | Una imagen o archivos rechazan la operación. |
| `a_destination_that_matches_is_accepted` | El destino válido se acepta. |
| `a_destination_in_another_window_is_refused` | Otra ventana no se escribe. |
| `a_destination_in_another_control_is_refused` | Otro control no se escribe. |
| `a_destination_with_another_text_is_refused` | Un texto distinto no se pisa. |
| `the_spaces_around_the_text_do_not_matter` | Los espacios que añade la aplicación. |
| `every_failure_has_a_stable_code` | Los códigos que la interfaz traduce. |
| `a_change_remembers_where_it_was_made` | El historial guarda el sitio. |
| `a_change_without_a_place_cannot_be_reverted` | Un cambio sin sitio no se deshace. |
| `the_place_of_a_change_survives_a_restart` | El sitio sobrevive al cierre. |
| `the_store_remembers_the_last_capture` | El contexto capturado se guarda. |

La regresión de `expected_control = 0` sigue cubierta: el destino se
comprueba contra la ventana **y** el control, y hay un caso que falla
cuando el control no es el esperado.

### 5.2 Lo que necesita el escritorio

Lo que de verdad no se puede comprobar sin otra ventana delante —leer el
texto de una selección real, escribirlo, deshacer un cambio, el
portapapeles con una imagen dentro— está en el procedimiento manual:

`docs/validation-phase2c/ACCEPTACION_MANUAL.md`, apartado 4 y siguientes, y
en `docs/dixi-selection-hotfix-manual.md`, que es la lista corta de este
hotfix.

**No se declara «Revertir» como comprobado sobre el escritorio hasta que
esa lista pasa.**

## 6. Los archivos

**Rust**

- `src-tauri/src/managers/selection.rs` — el contexto, la lista de
  controles, el portapapeles fiel, la escritura verificada.
- `src-tauri/src/managers/selection/tests.rs` — las pruebas.
- `src-tauri/src/managers/text_history.rs` — el contexto en el historial.
- `src-tauri/src/managers/personal_store.rs` — las columnas nuevas.
- `src-tauri/src/commands/selection.rs` — los comandos.
- `src-tauri/src/commands/text_history.rs` — el comando de reversión.
- `src-tauri/src/actions.rs` — la acción del atajo.
- `src-tauri/src/settings.rs` — el atajo por defecto.
- `src-tauri/src/lib.rs` — el registro.

**Frontend**

- `src/components/settings/transforms/TransformsSettings.tsx`
- `src/components/settings/textHistory/TextHistorySettings.tsx`
- `src/components/settings/general/GeneralSettings.tsx`
- `src/i18n/locales/*/translation.json` — 17 claves nuevas en 26 idiomas.
- `src/bindings.ts` — regenerado.
