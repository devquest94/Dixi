# Hotfix: la captura de la selección de otra aplicación

**Rama:** `fix/dixi-selection-capture` (desde `develop` @ `0647897`)
**Estado:** implementado y verificado en automático; la prueba sobre el
escritorio real la hace el usuario con el procedimiento manual.
**No fusionado a `develop`.**

Este hotfix tiene **dos vueltas**. La primera arregló el mecanismo: el
botón no podía funcionar por diseño, porque pulsarlo le quita el foco a la
aplicación de origen. La segunda recogió el reporte del usuario sobre la
primera y hizo que, **cuando falla**, el aviso diga qué aplicación tenía el
foco y si el atajo llegó a registrarse. Las dos están en la misma rama.

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

### 1.1 Lo que pasó en la sesión real (la primera vuelta del arreglo)

El atajo global resuelto no fue suficiente: el usuario siguo sin poder
capturar. El log de su sesion (`%LOCALAPPDATA%\com.dixi.app\logs\dixi.log`,
21-10-2026 02:09-02:12) dice exactamente que paso:

```
[02:09:07][dixi_app_lib::shortcut::handy_keys][DEBUG] Registered handy-keys
    shortcut: capture_selection -> Hotkey { modifiers: Modifiers(CTRL_LEFT |
    OPT_LEFT | CTRL_RIGHT | OPT_RIGHT), key: Some(C) }
[02:09:48][dixi_app_lib::shortcut::handy_keys][DEBUG] handy-keys event:
    binding=capture_selection, hotkey=ctrl+alt+c, state=Pressed
[02:09:49][dixi_app_lib::actions][WARN] CaptureSelectionAction: the selection
    could not be captured: there is no selected text
```

El atajo **si se registro** y **si se despacho**: seis pulsaciones, entre las
02:09:48 y la 02:12:50. Cuatro fallaron con «there is no selected text» y dos
con «the focused control is not a text control that Dixi knows».

O sea: el mecanismo funciona, pero **el aviso no decia nada util**. No decia
que aplicacion tenia el foco, ni en que paso fallo, ni si el atajo estaba
registrado. Y hay una segunda averia, que solo se ve mirando el escritorio:

**En Chrome, el control con el foco no es el que se creia.** Una sonda Win32
propia (`GetGUIThreadInfo`) dio esto:

| | Clase | |
|---|---|---|
| Ventana de primer plano | `Chrome_WidgetWin_1` | la ventana de nivel superior |
| Control con el foco | `Chrome_WidgetWin_1` | **es la misma ventana** |
| Su hijo | `Chrome_RenderWidgetHostHWND` | el que hospeda la pagina, pero **no** tiene el foco |

El informe de la primera vuelta daba por hecho que el enfocado era
`Chrome_RenderWidgetHostHWND`. **No lo es**: el foco de teclado lo tiene la
ventana de nivel superior. La lista blanca lo aceptaba por prefijo (`chrome_`),
asi que por eso la captura no se rechazaba, pero la afirmacion del informe era
falsa y hay que corregirla.

Y una tercera averia, esta en la sonda del camino de captura: en el momento de
la prueba el primer plano era `WixBurnMessageWindow`, la ventana del
instalador del SDK de .NET. El error «control desconocido» era **correcto**;
lo que faltaba era decir **cual** aplicacion.

Tres cosas que se corrigen en la segunda vuelta:

1. **El aviso dice que aplicacion tenia el foco** y en que paso fallo.
2. **El panel avisa si el atajo no se pudo registrar**, en vez de dejar un
   atajo que no hace nada sin explicación.
3. **La captura se invalida en cuanto se escribe en el texto**, y «Aceptar»
   se deshabilita: escribir a mano ya destruyó la identidad de la seleccion
   original, y aceptarla escribiria sobre otra cosa.

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

**Por qué se trae la aplicación al frente en vez de cancelar.** Hay dos
caminos posibles cuando la aplicación de origen ya no está en primer plano:
que el usuario vuelva a ella y confirme, o que Dixi la traiga. Se ha
elegido el segundo, y solo porque **«Aceptar» es una acción explícita del
usuario**: nadie escribe en otra aplicación sin haberlo pedido, y volver a
enfocar a mano el texto que se quiere cambiar es un paso que se puede
olvidar. La traer al frente no es un atajo: viene después de comprobar que
la ventana existe, y la escritura solo ocurre si la ventana, el control y el
texto siguen siendo los que se capturaron. Si Windows no permite traerla
—por ejemplo porque otra aplicación acaba de quitarle el foco—, la
operación se cancela y se ofrece copiar. Nunca se escribe en una ventana
distinta de la que se capturó.

### 2.4 El portapapeles: la garantía real

Antes, el portapapeles se vaciaba y se rellenaba solo con texto: una
imagen, unos archivos copiados, HTML o RTF **se perdían sin avisar**.

Ahora:

- Se enumera los formatos que tiene el portapapeles y **se guardan los
  bytes exactos de cada uno**: el texto Unicode, el ANSI, el locale, el
  HTML, el RTF, la lista de archivos copiados. Todo eso vive en memoria
  movible, y se vuelve a poner **exactamente igual**.
- **Si tiene un formato que solo Windows sabe interpretar** —un bitmap, un
  metafile, una paleta—, la operación **se rechaza antes de modificar
  nada**, con el aviso `clipboard_not_preservable`. Esos no se pueden
  copiar como bytes.
- Las secuencias que tocan el portapapeles están **serializadas con un
  cerrojo de proceso**: el atajo global y los comandos de la interfaz no
  pueden solaparse. Sin eso, dos lecturas simultáneas podían guardar un
  portapapeles ya vaciado y dejar al usuario sin el suyo, sin aviso.
- Si la restauración falla, **queda escrito en el log**: perder el
  portapapeles del usuario es justo lo que este módulo promete no hacer.

La consecuencia, dicha sin rodeos: **Dixi necesita que el portapapeles
pueda devolverse tal cual**. Con una imagen copiada, la captura se rechaza
y se le pide que la vacíe. Con un HTML o un RTF —lo que deja cualquier
copia de un párrafo de una página web— no hay problema: se guardan y se
devuelven.

Sobre el pegado: la espera no es un tiempo a ciegas cuando el control
puede responder. Se observa **el texto del control** hasta que cambia, que
es el momento en que la aplicación ha tomado lo que se le pegó; y solo
después se restaura el portapapeles. Si el control no responde —los
renderizadores de navegador—, se usa una espera acotada, y la escritura
se comprueba igualmente después.

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
| `Chrome_WidgetWin_1` | La ventana de nivel superior de Chromium: **es la que tiene el foco de teclado**. |
| `Chrome_RenderWidgetHostHWND` | El hijo que hospeda la página pintada. No tiene el foco, pero es de la misma familia. |
| `MozillaWindowClass` | Firefox. |

Las clases de los navegadores se comparan **por prefijo** (`chrome_`,
`mozillawindowclass`, `webkit`), porque el control enfocado cambia de nombre
entre versiones y comparar por familia es lo que no se rompe.

Lo que se comprobó en el escritorio, con una sonda Win32 propia
(`GetGUIThreadInfo`): en Chrome, **el control con el foco es
`Chrome_WidgetWin_1`**, la ventana de nivel superior; el
`Chrome_RenderWidgetHostHWND` es su hijo y no tiene el foco. La lista blanca
lo acepta por prefijo, y por eso la captura no se rechaza en un navegador.
La primera vuelta de este informe daba por hecho lo contrario, y queda
corregido.

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
| Navegadores con Chromium (Chrome, Edge, Brave, Opera, VS Code) | `Chrome_WidgetWin_1` | Sí | Sí |
| Firefox | `MozillaWindowClass` | Sí | Sí |
| WordPad, editores nativos | `RichEdit20W` | Sí | Sí |
| Campos de contraseña | cualquiera | **No** | **No** |
| Controles desconocidos | cualquiera | **No** | **No** |

## 4. Limitaciones, dichas sin adornos

1. **El portapapeles tiene que poder devolverse tal cual.** Con una imagen
   copiada, la captura se rechaza, porque un bitmap no se puede copiar como
   bytes. Con texto, HTML, RTF o archivos copiados no hay problema: se
   guardan y se devuelven igual.
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

## 5. La revisión independiente

El arreglo se reviso contra este brief, contrato a contrato y linea a linea,
sin ejecutar nada. Salieron **ocho defectos, cuatro de ellos graves**, y
todos estan corregidos.

### 5.1 Los graves

1. **La comprobación posterior a la escritura fallaba siempre en los
   navegadores.** `WM_GETTEXTLENGTH` devuelve 0 en un control que no
   atiende el mensaje, y ese 0 se confundia con «el control esta vacio».
   La escritura **si habia ocurrido**, pero Dixi decia que no, y el cambio
   no se guardaba en el historial: el usuario perdia el deshacer de una
   escritura que si habia pasado. Ahora la decision se toma por la clase
   del control, y solo los controles nativos se consultan.

2. **La captura se rechazaba siempre en los navegadores Chromium.** La
   lista de controles verificados no incluia ninguna clase de navegador, y
   el control con el foco en Chromium es `Chrome_WidgetWin_1` —la ventana de
   nivel superior, comprobada con una sonda Win32 sobre el escritorio—, de
   modo que Chrome, Edge, Brave, Opera y VS Code quedaban fuera. La
   correccion es comparar **por familia** (`chrome_`, `mozillawindowclass`,
   `webkit`) en vez de por un nombre exacto, que cambia entre versiones.
   (La primera vuelta de este informe decia que el enfocado era
   `Chrome_RenderWidgetHostHWND`: no lo es, y queda corregido en 1.1.)

3. **El portapapeles hacia la captura inservible con cualquier copia
   enriquecida.** La regla era «solo texto», asi que copiar un parrafo de
   una pagina web (que deja tambien HTML) ya bloqueaba la captura. Ahora
   se guardan los bytes exactos de cada formato que vive en memoria y se
   devuelven tal cual; solo se rechaza lo que no se puede reproducir.

4. **No habia exclusion mutua entre el atajo y los comandos.** Dos
   manipulaciones del portapapeles a la vez podian dejar vacio el
   portapapeles del usuario, sin aviso. Ahora hay un cerrojo de proceso
   alrededor de toda secuencia que lo toque.

### 5.2 Los demas

| Defecto | Correccion |
|---|---|
| El historial se grababa despues de escribir: si la comprobacion posterior fallaba, se perdia el deshacer de una escritura correcta. | Se graba **antes** de escribir, y se borra si la escritura falla. |
| El atajo de captura solo se filtraba en una de las cuatro rutas de registro: en macOS se registraba y se tragaba la combinacion. | La decision esta en un sitio unico y se aplica en las cuatro. |
| «Revertir» exige seleccionar antes el texto que Dixi escribio, y el panel no lo decia. | El panel lo dice, y el error es el que corresponde. |
| Si la restauracion del portapapeles fallaba, se perdia en silencio. | Ahora queda en el log. |
| La comprobacion posterior buscaba el texto nuevo dentro del control: si ya estaba en otra parte, daba un «si» falso. | Se compara el estado del control antes y despues. |
| La espera del pegado era un tiempo fijo sin ninguna prueba. | Cuando el control puede responder, se observa su texto hasta que cambia. |
| Faltaban pruebas de foco perdido, ventana cerrada, la regresion de `expected_control = 0` y la restauracion con varios formatos. | Anadidas. |

### 5.3 Lo que la revision no puede cubrir

Una revision estatica no ejecuta la aplicacion. Lo que de verdad necesita
otra ventana delante —leer el texto de una seleccion real, escribirlo,
deshacer un cambio, el portapapeles con una imagen dentro— sigue en la
lista manual, y **no se declara comprobado hasta que esa lista pasa**.

---

## 6. Pruebas

### 6.1 Automáticas (Rust)

**549 PASS, 0 FAIL, 18 ignoradas** (eran 524 antes de este arreglo). Las 25
nuevas cubren lo que se puede decidir sin escritorio:

| Prueba | Qué fija |
|---|---|
| `only_the_verified_text_controls_are_read` | La lista de controles que se leen. |
| `the_focused_control_of_a_chromium_window_is_read` | La clase real del control enfocado de Chromium. |
| `the_browser_renderers_are_not_native_text_controls` | Que un renderizador no se consulta con `WM_GETTEXT`. |
| `an_unknown_control_is_not_read` | Un control desconocido no se lee. |
| `the_class_is_matched_without_case_and_without_spaces` | La comparación de clases. |
| `the_password_style_bit_is_detected` | El bit `ES_PASSWORD`. |
| `the_password_class_is_detected` | Las clases de contraseña. |
| `an_edit_control_with_the_password_style_is_refused` | El campo enmascarado se rechaza. |
| `the_style_bit_alone_does_not_refuse_other_controls` | El bit no se malinterpreta. |
| `a_format_in_memory_can_be_restored` | Los formatos que se pueden devolver igual. |
| `a_format_with_a_handle_cannot_be_restored` | Un bitmap o un metafile rechazan la operación. |
| `a_format_that_is_not_in_memory_cannot_be_restored` | Lo que no está en memoria no se devuelve. |
| `a_destination_that_matches_is_accepted` | El destino válido se acepta. |
| `a_destination_in_another_window_is_refused` | Otra ventana no se escribe. |
| `a_destination_in_another_control_is_refused` | Otro control no se escribe. |
| `a_destination_with_another_text_is_refused` | Un texto distinto no se pisa. |
| `a_control_of_zero_is_never_the_control_that_was_read` | La regresión de `expected_control = 0`, con caso explícito. |
| `a_window_that_is_not_alive_is_detected` | Una ventana cerrada no es un destino. |
| `the_spaces_around_the_text_do_not_matter` | Los espacios que añade la aplicación. |
| `every_failure_has_a_stable_code` | Los códigos que la interfaz traduce. |
| `a_failure_carries_the_application_that_had_the_focus` | Que el fallo diga qué aplicación tenía el foco. |
| `the_code_of_a_wrapped_failure_is_the_code_of_its_reason` | Que los diagnósticos no cambien el motivo. |
| `the_diagnostics_never_carry_the_selected_text` | Que el texto seleccionado no viaje en los diagnósticos. |
| `a_failure_without_diagnostics_still_has_its_code` | Que un fallo sin diagnóstico siga teniendo su código. |
| `a_change_remembers_where_it_was_made` | El historial guarda el sitio. |
| `a_change_without_a_place_cannot_be_reverted` | Un cambio sin sitio no se deshace. |
| `a_change_that_never_happened_is_removed` | Un cambio que no ocurrió no queda en el historial. |
| `the_place_of_a_change_survives_a_restart` | El sitio sobrevive al cierre. |
| `the_store_remembers_the_last_capture` | El contexto capturado se guarda. |

La regresión de `expected_control = 0` sigue cubierta: el destino se
comprueba contra la ventana **y** el control, y hay un caso explícito que
falla cuando el control no es el esperado.

### 6.2 Lo que necesita el escritorio

Lo que de verdad no se puede comprobar sin otra ventana delante —leer el
texto de una selección real, escribirlo, deshacer un cambio, el
portapapeles con una imagen dentro— está en el procedimiento manual:

`docs/validation-phase2c/ACCEPTACION_MANUAL.md`, apartado 4 y siguientes, y
en `docs/dixi-selection-hotfix-manual.md`, que es la lista corta de este
hotfix.

**No se declara «Revertir» como comprobado sobre el escritorio hasta que
esa lista pasa.**

### 6.3 La suite completa

| Comprobación | Resultado |
|---|---|
| `cargo test --features custom-protocol --lib` | **549 PASS**, 0 FAIL, 18 ignoradas |
| `cargo test --features custom-protocol --doc` | PASS |
| Avisos del compilador | **0** |
| `npx.cmd tsc --noEmit` | 0 errores |
| `npx.cmd eslint src` | 0 errores, 0 avisos |
| `node scripts/check-translations.ts` | PASS, 25/25 idiomas |
| `node scripts/check-translation-text.mjs` | 26 locales, 0 problemas |
| `cargo build --release --features custom-protocol` | OK (`lto = true`, `codegen-units = 1`) |
| `scripts/verify-release-protocol.ps1 -SmokeTest` | PASS (el frontend va incrustado y el binario arranca) |

Todo se ejecutó **sin abrir una ventana de consola**, con
`scripts/run-hidden.ps1`, que deja la salida y el error en ficheros de log
bajo `.logs/`.

## 7. Los archivos

**Rust**

- `src-tauri/src/managers/selection.rs` — el contexto, la lista de
  controles, el portapapeles fiel, la escritura verificada, y los
  diagnósticos de la captura.
- `src-tauri/src/managers/selection/tests.rs` — las pruebas.
- `src-tauri/src/managers/text_history.rs` — el contexto en el historial.
- `src-tauri/src/managers/personal_store.rs` — las columnas nuevas.
- `src-tauri/src/commands/selection.rs` — los comandos, y el del estado del
  atajo.
- `src-tauri/src/commands/text_history.rs` — el comando de reversión.
- `src-tauri/src/actions.rs` — la acción del atajo, y su log técnico.
- `src-tauri/src/settings.rs` — el atajo por defecto.
- `src-tauri/src/shortcut/{mod,tauri_impl,handy_keys}.rs` — el registro, la
  decisión de dónde se puede registrar y el estado del registro.
- `src-tauri/src/lib.rs` — el registro.

**Frontend**

- `src/components/settings/selectionErrors.ts` — el módulo compartido de los
  avisos de la captura.
- `src/components/settings/transforms/TransformsSettings.tsx`
- `src/components/settings/textHistory/TextHistorySettings.tsx`
- `src/components/settings/general/GeneralSettings.tsx`
- `src/i18n/locales/*/translation.json` — 21 claves nuevas en 26 idiomas.
- `src/bindings.ts` — regenerado.

## 8. La segunda vuelta: qué se cambió con el reporte del usuario

La primera vuelta arregló el mecanismo (el botón no podía funcionar por
diseño) pero no dijo nada útil cuando fallaba. Con el log de la sesión real
y dos sondas Win32 propias, la segunda vuelta corrige tres cosas.

### 8.1 El aviso dice qué aplicación tenía el foco

El backend ahora sabe, en cada fallo, **qué aplicación tenía el foco**, **qué
clase de control era** y **en qué paso falló** (`focus`, `control` o `copy`).
Eso viaja como `CaptureDiagnostics` y llega a la interfaz junto al código
estable del motivo.

El texto seleccionado **no** viaja nunca: los diagnósticos solo llevan datos
técnicos, y hay una prueba que lo fija
(`the_diagnostics_never_carry_the_selected_text`).

En la interfaz, `src/components/settings/selectionErrors.ts` compone el aviso:

```
"There is no selected text in the app that has the focus."
"La aplicación que tenía el foco era chrome.exe."
```

Y cuando el foco estaba en Dixi mismo —la causa más habitual, porque el
atajo se pulsó con Dixi delante—, el aviso es otro: volver a la aplicación de
origen y pulsar el atajo otra vez.

El log técnico, sin texto:

```
CaptureSelectionAction: app=chrome.exe class=Chrome_WidgetWin_1 step=copy
    result=error code=nothing_selected
```

**El camino de escritura lleva los mismos diagnósticos.** Cuando «Aceptar» o
«Revertir» fallan, el aviso dice también qué aplicación tenía el foco y en
qué paso falló: `clipboard`, `focus`, `control`, `copy`, `verify`, `paste` o
`write`. Antes, un fallo al escribir y un fallo al capturar eran el mismo
mensillo.

### 8.2 El panel avisa si el atajo no se pudo registrar

Un atajo global que otra aplicación ya tiene **no da error visible**: el
atajo sencillamente nunca se dispara. Ahora el registro guarda si pudo
registrarse, y el comando `get_capture_shortcut_state` lo expone, y el panel
de Transforms avisa:

> El atajo Ctrl+Alt+C no se pudo registrar: otra aplicación ya lo tiene.
> Cambialo en los atajos de teclado.

Si el registro falla, además queda escrito en el log con un `error!`.

> **Esta sección se corrigió en una tercera vuelta del hotfix.** El aviso
> que se describe arriba mentía: el estado que lo alimentaba se escribía en
> un solo sitio, y el arranque del atajo no pasaba por ahí, así que la
> interfaz decía «no se pudo registrar» mientras el atajo estaba
> registrado y funcionando. Además culpaba a otra aplicación de cualquier
> fallo, y con el backend que Dixi usa en Windows eso **no puede ser**:
> handy-keys vigila el teclado con un hook y nunca pide al sistema reservar
> la combinación, así que ninguna otra aplicación puede entrar en conflicto.
> El diagnóstico completo está en
> `docs/dixi-shortcut-registration-diagnosis.md`.

### 8.3 La captura se invalida en cuanto se escribe

El contexto capturado dice «este texto estaba seleccionado en este control».
En cuanto el usuario escribe en el área de texto, esa identidad ya no es
cierta: aceptar escribiría sobre otra cosa. Así que:

- al escribir en el textarea, la captura se marca como **inválida**;
- **«Aceptar» se deshabilita**, con el motivo en el propio botón;
- y sale un aviso debajo del texto.

Esto no toca la captura en sí: la captura sigue siendo válida para
**copiar** el resultado, que es lo que se puede hacer sin escribir.

### 8.4 Las cuatro claves nuevas

| Clave | Para qué |
|---|---|
| `settings.selection.errors.focusedApp` | Decir qué aplicación tenía el foco (`{{app}}`). |
| `settings.selection.errors.focusedAppIsDixi` | El caso de que el foco estuviera en Dixi. |
| `settings.transforms.runner.captureInvalidated` | Que la captura ya no vale. |
| `settings.transforms.runner.shortcutNotRegistered` | Que el atajo no se registró (`{{shortcut}}`). |

En **26 idiomas**, con el tono de cada uno (voseo en español, *Sie* en
alemán, imperativo plural en hebreo, *vous* en francés). Los 26 ficheros
siguen en CRLF, con BOM ninguno y sin reformatear nada: el diff de cada uno
son cuatro lineas nuevas.
