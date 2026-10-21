# Dixi — Informe técnico: integración con el escritorio remoto (mRemoteNG / RDP)

**Alcance:** dictar en el equipo personal y pegar el texto dentro de una sesión
RDP abierta con mRemoteNG, sin instalar nada en el equipo remoto y sin tocar
la política de seguridad de la empresa.
**Rama:** `feat/dixi-remote-desktop` (base: `develop` @ `0647897`).
**Fecha:** 2026-10-20.

---

## 1. Resumen ejecutivo

| Objetivo | Estado | Qué se entregó |
|---|---|---|
| Detección de mRemoteNG y de la sesión RDP | **DESARROLLADO** | Se lee del sistema de ventanas: proceso, sesiones RDP abiertas, ventana enfocada y versión instalada. No se lee ninguna conexión guardada. |
| Modo manual de pegado | **DESARROLLADO** | La transcripción va al portapapeles local y espera; el usuario pulsa Ctrl+V en la sesión remota. No se envía ninguna tecla. |
| Portapapeles seguro con estados | **DESARROLLADO** | Texto preparado → pendiente de pegado → confirmación manual → restauración segura del contenido anterior. |
| Interfaz de configuración | **DESARROLLADO** | Sección propia en Ajustes, con el atajo, el estado de la transferencia y lo que se detectó del entorno. |
| Atajo global y bandeja | **DESARROLLADO** | `Ctrl+Alt+Espacio` prepara la última transcripción sin robar el foco; la bandeja ofrece la misma acción. |
| Modo automático | **PARCIAL** | Las guardas están implementadas y **validadas contra una sesión real**. Bloqueado por la misma causa que el modo manual, y desactivado por defecto. |
| Pruebas | **EJECUTADAS** | Pruebas unitarias del Rust más typecheck, lint y paridad de traducciones del frontend. |

### Flags de estado

| Flag | Valor | Por qué |
|---|---|---|
| `MREMOTENG_DETECTED` | `PASS` | mRemoteNG 1.76.20.24615 instalado en `C:\Program Files (x86)\mRemoteNG\`, con `Interop.MSTSCLib.dll` y `AxInterop.MSTSCLib.dll` (el ActiveX RDP de Microsoft embebido). |
| `CONNECTION_PROTOCOL` | `RDP` | La única conexión configurada en `confCons.xml` tiene `Protocol="RDP"`, y el log del propio cliente registra `Connexión ... via "RDP"`. |
| `REMOTE_DETECTION` | `PASS` | Validado contra la sesión real: el cliente se detecta, hay exactamente una sesión RDP abierta, y el foco dentro de ella se reconoce (`focused: RdpSession`). |
| `REMOTE_MANUAL_MODE` | `PARTIAL` | La mecánica de Dixi está verificada con evidencia real (publicación, lectura a los ~4 s, restauración al confirmar). **El texto no llega al equipo remoto**: el canal de portapapeles de la sesión no lo transporta (ver 2.6). |
| `REMOTE_CLIPBOARD_BRIDGE` | `FAIL` | El texto preparado **no aparece** en el equipo remoto. La causa está del lado del cliente o del servidor del canal RDP, no en Dixi (ver 2.6). |
| `REMOTE_AUTO_MODE` | `PARTIAL` | Las **guardas** están implementadas y **validadas contra una sesión real** (el cliente se detecta, hay una sesión, y el foco dentro de ella se reconoce). Bloqueado por la misma causa que el modo manual, y desactivado por defecto. |
| `CLIPBOARD_STATES` | `PASS` | Los cuatro estados pedidos existen, y el de "restauración segura" distingue los formatos que no se pueden recuperar sin pérdida. |
| `REMOTE_UI` | `PASS` | Sección de Ajustes, atajo, bandeja y traducciones en los 26 idiomas. |
| `REMOTE_TESTS` | `PASS` | Pruebas unitarias del Rust y verificaciones del frontend ejecutadas, con resultado real registrado abajo. |

---

## 2. Lo que se encontró en el entorno (Fase 1)

### 2.1 Estado del repositorio

La rama activa al empezar era `fix/dixi-selection-capture`, con **10 commits sin
mergear en `develop`** (el hotfix de captura de selección). Su sesión había
quedado caída (`crash-recovery`) y el árbol de trabajo estaba limpio: el hotfix
no estaba siendo tocado en ese momento, pero tampoco estaba integrado.

Para no mezclar los dos trabajos:

- Se creó `feat/dixi-remote-desktop` **desde `develop`**, no desde la rama del
  hotfix.
- Se comprobó con `git diff --stat develop..fix/dixi-selection-capture` qué
  archivos toca el hotfix: `settings.rs`, los 26 archivos de idioma, el módulo
  de atajos, el overlay, el gestor de selección, `tray.rs` y el gestor de
  texto. **El hotfix no toca `clipboard.rs` ni `paste_tx/`**, que es donde vive
  la lógica de portapapeles.

Los archivos que este trabajo sí toca y el hotfix también
(`settings.rs`, `shortcut/mod.rs`, `tray.rs` y los 26 idiomas) reciben
cambios **aditivos en regiones distintas**, así que el merge debería resolver
sin conflictos semánticos. Aun así, el merge es una decisión del usuario.

### 2.2 mRemoteNG

| Dato | Valor |
|---|---|
| Instalado | Sí |
| Versión | 1.76.20.24615 |
| Ruta | `C:\Program Files (x86)\mRemoteNG\mRemoteNG.exe` |
| Arquitectura | 32 bits, .NET (WinForms) |
| Motor RDP | **Indigo** (implementación de RDP en .NET), no el ActiveX de Microsoft |

Que mRemoteNG embeba `AxInterop.MSTSCLib` no significa que use ese ActiveX:
es el ensamblado de interoperabilidad del modo opcional con el cliente de
Microsoft. **La prueba real mostró que mRemoteNG habla RDP con Indigo**, una
implementación pura en .NET. La prueba que lo demuestra está en 2.5.

### 2.3 El protocolo de la conexión

Se leyó **solo el atributo `Protocol`** de `confCons.xml`, con un script que
descarta el resto de los campos antes de imprimir:

- Total de nodos con atributo `Protocol`: **1**
- Valores: **`Protocol=RDP` → 1 conexión**

Y el log del propio cliente (`%APPDATA%\mRemoteNG\mRemoteNG.log`) registra
conexiones por RDP de forma repetida.

**No se leyeron** direcciones de servidores, nombres de usuario, dominios,
contraseñas, puertos ni el resto de las conexiones guardadas. El único valor
extraído de la configuración fue el protocolo.

### 2.4 Por qué el portapapeles y no la inyección de teclas

Esta es la decisión de diseño central, y sale de lo anterior:

1. El cliente RDP de mRemoteNG **sincroniza el portapapeles local con la
   sesión remota** por el canal de redirección que el servidor autoriza. Esa
   redirección es el mecanismo sancionado por la política corporativa: no
   hay que tocar `rdpclip.exe` del equipo remoto ni cambiar nada del lado
   servidor.
2. Como la redirección ya lleva el texto, **el modo manual no necesita
   enviar ninguna tecla**. Escribir en el portapapeles local alcanza, y el
   usuario pulsa Ctrl+V dentro de la sesión remota.
3. Por lo tanto **nunca hay que tratar un handle remoto como si fuera local**:
   no se busca un HWND dentro de la sesión remota, no se manda `Ctrl+V` a ciegas
   y no se intenta leer nada del equipo remoto.
4. El riesgo real del modo manual no es la transferencia, es **el momento de
   restaurar el portapapeles**. La sincronización es asíncrona, y una espera
   fija de 250 ms no demuestra que el texto haya llegado. Por eso el texto
   **se queda en el portapapeles hasta que el usuario confirma el pegado**,
   y no se restaura por tiempo.

El modo automático (inyectar el Ctrl+V) queda como una capa opcional encima,
con guardas, y desactivado.

### 2.5 Lo que la prueba real corrigió: cómo se reconoce la sesión RDP

La primera versión de la detección buscaba una ventana cuya clase hubiera
sido registrada por `mstscax.dll` (el ActiveX de Microsoft). **La prueba
real contra la sesión abierta refutó ese supuesto**, en sus dos partes:

| Se suponía | Lo que muestra la máquina real |
|---|---|
| `mstscax.dll` está cargado y su ventana tiene su clase | `mstscax.dll` **nunca se carga**: mRemoteNG habla RDP con **Indigo** |
| La superficie RDP es una ventana identificable por su clase de módulo | Todas las ventanas son WinForms genéricas (`WindowsForms10.Window.8.*`) y `GetClassLongPtr(GCLP_HMODULE)` devuelve **cero** en todas: esas clases se registran sin módulo |

Lo que sí aparece, y es lo que usa la detección final:

| Clase de ventana | Qué es |
|---|---|
| `UIMainClass` / `UIContainerClass` | La ventana del cliente remoto |
| **`IHWindowClass`** | **El Indigo Host: donde se dibuja la sesión RDP** |
| `OPWindowClass` / `OPContainerClass` | El panel de salida de Indigo |
| `BBarWindowClass` | La barra de acoplamiento |

Por eso la detección final cuenta las ventanas `IHWindowClass` visibles del
proceso de mRemoteNG: **cada una es una sesión RDP abierta**, y contarlas
resuelve también el caso de "dos sesiones abiertas" (destino ambiguo).

Además, la sesión RDP se dibuja **dentro** de la ventana del cliente: cuando
el usuario está en la sesión, la ventana enfocada es la del cliente. Por eso
el foco se comprueba con la ventana que tiene el foco real dentro del
proceso (`GetGUIThreadInfo`), y se exige que esté **dentro** de una ventana
`IHWindowClass`.

**Verificación contra la sesión real** (con mRemoteNG abierto y la conexión
establecida), en los tres estados que importan:

| Estado | Resultado | Lectura |
|---|---|---|
| La sesión abierta, el foco en el navegador | `client_running: true`, `rdp_sessions: 1`, `focused: OtherApplication` | El cliente y la sesión se detectan; el foco no está en la sesión. |
| El foco en la sesión RDP | `client_running: true`, `rdp_sessions: 1`, **`focused: RdpSession`** | El usuario está dentro de la sesión. |
| Detalle de la ventana con el foco | `foreground class="UIMainClass" titulo="WorkingDesktop"` \| **`focus class="IHWindowClass" tam=(2048,1152)`** | Confirma el mecanismo: la ventana enfocada es la del cliente, y la que tiene el teclado es el Indigo Host. |

Los cuatro hechos que necesita la integración quedan comprobados contra la
máquina real: **el cliente se detecta, hay exactamente una sesión RDP
abierta, y el foco dentro de esa sesión se reconoce**.

### 2.6 Lo que la prueba real reveló: el texto no cruza al equipo remoto

La mecánica de Dixi funciona y está verificada; **la transferencia no**. El
ciclo completo, tomado del log de la app durante la prueba:

```text
[14:02:16] [manual-paste]  transcript published for a paste by hand
[14:02:16] [remote-desktop] transcript prepared for a manual paste (99 characters)
[14:02:16] [actions]        the transcript is ready to paste in the remote session
[14:03:25] [reliable-paste] clipboard read 4158ms after the trigger
[14:04:12] [remote-desktop] the user confirmed the paste after 51s
[14:04:12] [reliable-paste] restored previous clipboard
```

Y el resultado observado por el usuario: **el texto no apareció en el equipo
remoto** al hacer Ctrl+V dentro de la sesión.

Es decir: la publicación funciona, alguien lee el portapapeles local a los
~4 segundos, y la restauración funciona. Lo que no ocurre es que ese texto
llegue al portapapeles del equipo remoto.

**La causa está del lado del canal RDP, no de Dixi.** Tres datos lo
sostienen:

1. **mRemoteNG no expone ninguna opción de portapapeles para la
   conexión.** Su perfil de RDP tiene `RedirectDiskDrives`, `RedirectKeys`,
   `RedirectPorts`, `RedirectPrinters`, `RedirectSmartCards` y
   `RedirectSound`… y **nada de portapapeles**: la cadena "clip" no aparece
   en ningún punto de `confCons.xml`.
2. **La sesión se ejecutó sobre la implementación .NET del cliente**, no
   sobre el ActiveX de Microsoft. La ventana de la sesión es de clase
   `IHWindowClass` (2048×1152), que es el *host* de Indigo. El cliente de
   Microsoft habría creado una ventana de `mstscax.dll`.
3. **El ActiveX de Microsoft está instalado y registrado en esta máquina**
   (`CLSID {0B87E651-52FD-3B66-AC4B-730A10D7CAFA}` →
   `AxInterop.MSTSCLib.dll`), y mRemoteNG lo embebe con toda su interfaz
   `IMsRdpClientAdvancedSettings`, que es donde vive `fDisableClip`, la
   propiedad que gobierna la redirección del portapapeles.

En RDP, la redirección del portapapeles la decide el **servidor** (la
política del Session Host), y el cliente sólo puede desactivarla. Con la
implementación .NET y sin una opción de cliente para activarla a mano, el
resultado es que el texto no cruza.

**Ninguna de las dos vías disponibles desde Dixi arregla esto**, y ninguna
debe intentar hacerlo: cambiar la política del servidor sería tocar la
infraestructura corporativa, que el encargo excluye de forma expresa.

**Lo que sí queda a mano del usuario**, en su propio equipo y sin tocar la
infraestructura:

- **Probar el cliente ActiveX de Microsoft en mRemoteNG**, si la interfaz
  ofrece esa variante. Está instalado y registrado. Es el camino con más
  probabilidades: el cliente de Microsoft implementa el canal de portapapeles
  completo.
- **Consultar con el área de sistemas** si el Session Host de la sesión
  permite la redirección del portapapeles (política del Session Host,
  "Permitir redirección del portapapeles"). Si la política la prohíbe, no
  hay alternativa del lado del cliente.

Mientras tanto, el modo manual **sí es útil aunque el texto no cruce**: la
transcripción queda en el portapapeles local, que es justamente lo que el
modo manual garantiza. Lo que no puede es llevar el texto al equipo remoto
por sí sola.

**Efecto sobre el modo automático:** el modo automático se apoya en el mismo
canal de portapapeles, así que hereda el mismo bloqueo. Sigue
desactivado por defecto, y sus guardas ya están validadas.



---

## 3. Lo que se implementó

### 3.1 Modo manual de pegado

El flujo completo, sin tocar el motor ASR ni la cadena de transformaciones:

1. El usuario dicta en su equipo personal. Whisper transcribe localmente.
2. Se aplican únicamente las correcciones locales habilitadas, como siempre.
3. Con el atajo `Ctrl+Alt+Espacio` (o el botón, o la bandeja) Dixi toma **la
   última transcripción completada** del historial — el mismo texto que usa la
   acción "copiar la última transcripción" que ya existía.
4. Ese texto se publica en el portapapeles local **sin inyectar ninguna tecla**.
5. El usuario hace clic en el campo remoto y pulsa Ctrl+V. La redirección del
   cliente RDP lo lleva al equipo remoto.
6. El usuario confirma ("Ya lo pegué") y Dixi restaura el portapapeles
   anterior, si todavía le pertenece.

El atajo es **global** a propósito, por la misma razón que el atajo de captura
del hotfix: el foco tiene que quedarse en la ventana remota. Un botón en la
ventana de Dixi obligaría a mover el foco, que es justo lo que hay que evitar.

### 3.2 El portapapeles seguro y sus estados

La lógica de publicación y restauración ya existía en `paste_tx` (transacción
con recibo, con guarda de propiedad y con restauración por formato). Se
**extendió** en vez de duplicarla:

| Estado | Qué significa | Quién lo produce |
|---|---|---|
| **Texto preparado** | La transcripción está en el portapapeles local. | `prepare` |
| **Pendiente de pegado** | Sigue esperando; el cliente RDP todavía no leyó el portapapeles. | `evaluate` |
| **Confirmación manual de pegado realizado** | El usuario confirmó; se restaura el contenido anterior. | `confirm` |
| **Restauración segura del contenido anterior** | Se restauró lo que había, y solo si el portapapeles seguía siendo nuestro. | El worker de `paste_tx` |

Cambios concretos en `paste_tx`:

- **Un disparador nuevo.** `TxTrigger::Chord` (el de siempre: solo cuentan las
  lecturas posteriores al acorde) y `TxTrigger::Publication` (el texto se pega
  a mano: cuenta cualquier lectura posterior a la publicación).
- **Un final nuevo.** `TxState::settle_requested`: el que llama pide cerrar la
  transacción. Antes solo terminaba cuando el acorde se fallaba, cuando la
  propiedad se perdía o cuando se agotaba el tiempo.
- **El final por lectura se desactivó para el modo manual.** Es la parte
  delicate: en el modo manual, que el cliente RDP lea el portapapeles
  **no** significa que el usuario ya haya pulsado Ctrl+V en el lado remoto, y
  restaurar ahí sería exactamente la carrera de tiempo fijo que este módulo
  existe para evitar. El recibo se sigue registrando y se muestra como
  información ("el cliente remoto leyó el texto"), pero no cierra la
  transferencia.
- **Los formatos irrecuperables se reportan.** El snapshot del portapapeles
  conserva los formatos que se pueden copiar byte a byte. Los que no
  (`CF_ENHMETAFILE`, `CF_PALETTE`, `CF_OWNERDISPLAY`, los `CF_DSP*`, y los
  que no caben o no se pueden bloquear) se anotan y **se le dicen al usuario**
  por nombre de formato, sin contenido. El aviso dice exactamente cuáles son,
  para que la decisión de reemplazarlos sea informada.
- **La propiedad se respeta.** Si entre la preparación y la confirmación el
  usuario copió otra cosa, ese contenido gana: la restauración se omite y
  queda registrado en el log.
- **La app al cerrarse no deja el portapapeles vacío.** El texto se publica
  con *renderizado diferido*, así que vive en el proceso de Dixi. Si el
  proceso muere con una transferencia pendiente, Windows descarta ese
  contenido diferido y **el portapapeles del usuario queda vacío** en vez de
  contener lo que tenía antes. Eso lo destapó la prueba real (al reiniciar
  la aplicación a mitad de una transferencia, el portapapeles quedó vacío).
  El arreglo es liquidar la transferencia pendiente en `RunEvent::Exit`,
  antes de que el proceso se vaya, y esperar a que la restauración se
  complete. Con eso, cerrar Dixi con una transferencia abierta devuelve el
  portapapeles a su contenido anterior en vez de vaciarlo.

### 3.3 Modo automático: evaluación y guardas

El modo automático **sí se implementó**, con las condiciones que pedía el
encargo, todas verificadas por una función pura y cubierta por pruebas:

| Guarda | Qué exige | Por qué |
|---|---|---|
| Interruptor general | `remote_desktop_enabled` | Apagado por defecto. |
| Modo automático | `remote_desktop_auto_mode` | Apagado por defecto, y además no aparece como opción lista en la interfaz. |
| Cliente remoto | el proceso `mremoteng.exe` está corriendo | Sin el cliente no hay destino. |
| Sesión RDP | hay exactamente **una** ventana `IHWindowClass` visible | Con dos sesiones el destino es ambiguo: se rechaza. |
| Foco | la ventana con el foco real está **dentro** de una sesión | Se exige que el usuario esté en la sesión, no en la lista de conexiones. |
| Redirección | implícita en que el portapapeles local llega | El cliente RDP la gestiona; Dixi no la puede forzar ni tocar. |
| Re-verificación | el foco se comprueba **otra vez** justo antes de enviar la tecla | El foco puede cambiar entre la decisión y la acción. |
| Confirmación | si algo no se puede confirmar, se cae al modo manual | Una transcripción pegada en la ventana equivocada es peor que una tecla de más. |

**El reconocimiento de la superficie RDP se basa en una clase de ventana
medida contra la máquina real**, no en un nombre supuesto: `IHWindowClass`,
la ventana del Indigo Host donde mRemoteNG dibuja la sesión. La primera
versión del código buscaba el ActiveX de Microsoft y **la prueba real la
refutó** (ver 2.5).

**Lo que no se pudo verificar sin una sesión real:** que el texto llegue al
equipo remoto y que el Ctrl+V aterrice en el campo correcto. Por eso el modo
automático queda **desactivado por defecto** y la interfaz lo marca como no
validado.

### 3.4 La interfaz

Sección propia en Ajustes → *Escritorio remoto*, con:

- Interruptor de la integración.
- Interruptor del modo manual.
- Interruptor del modo automático, con la nota de que todavía no está
  validado.
- Interruptor de restauración del portapapeles.
- Interruptor de la notificación.
- El atajo configurable, reusando el editor de atajos que ya existía.
- Botones: *Preparar la última transcripción*, *Ya lo pegué*, *Cancelar la
  transferencia*.
- El estado de la transferencia, leyendo los eventos que emite el backend.
- Lo que se detectó del entorno: versión instalada, si está corriendo, cuántas
  sesiones RDP hay abiertas, si la sesión remota tiene el foco y por qué el
  modo automático no puede actuar.

El componente solo se monta en Windows: el cliente que se integra es una
aplicación de Windows.

### 3.5 La bandeja

Se añadió la acción *Preparar la transcripción para el escritorio remoto* al
menú de la bandeja, activa solo mientras la integración está encendida. Usa el
mismo camino que el atajo.

---

## 4. Pruebas

### 4.1 Pruebas unitarias del Rust

Las guardas del modo automático son una función pura, así que se prueban
directamente:

| Prueba | Qué cubre |
|---|---|
| `the_integration_is_off_by_default` | El interruptor general bloquea la inserción. |
| `the_automatic_mode_is_off_by_default` | El modo automático bloquea la inserción. |
| `a_client_that_is_not_running_blocks_the_insertion` | Conexión no RDP: el cliente no está corriendo. |
| `a_connection_that_is_not_rdp_blocks_the_insertion` | El cliente corre pero no tiene sesión RDP. |
| `a_remote_window_without_the_focus_blocks_the_insertion` | La ventana remota no tiene el foco. |
| `two_open_sessions_block_the_insertion` | Dos sesiones abiertas: destino ambiguo. |
| `a_single_focused_rdp_session_allows_the_insertion` | El caso bueno. |
| `the_focus_of_another_application_blocks_the_insertion` | Otra aplicación enfocada. |
| `the_timeout_is_bounded` | El tiempo de espera se acota. |

Y en `paste_tx`, el comportamiento nuevo:

| Prueba | Qué cubre |
|---|---|
| `publication_keeps_waiting_even_after_the_reads_go_quiet` | **Sincronización lenta**: una lectura no cierra la transferencia. |
| `publication_counts_reads_from_the_publication` | Los recibos cuentan desde la publicación. |
| `publication_finishes_when_the_caller_settles_it` | La confirmación manual cierra. |
| `publication_finishes_when_the_transfer_expires` | El vencimiento cierra. |
| `publication_finishes_when_the_clipboard_ownership_is_lost` | Perder la propiedad cierra. |

### 4.2 Verificaciones del frontend

Typecheck, lint y paridad de traducciones. El resultado exacto está en la
sección 6.

### 4.3 Lo que NO está verificado

**La transferencia real contra una sesión RDP.** Ninguna de las pruebas
anteriores toca un cliente RDP: son la lógica de decisión y de estado. La
verificación de que el texto llega al equipo remoto, de que el Ctrl+V aterriza
en el campo correcto y de cuánto tarda la sincronización **requiere la sesión
real del usuario**, y está pendiente de su autorización explícita.

---

## 5. Lo que NO se hizo, y por qué

1. **No se instaló nada en el equipo remoto.** Todo pasa por el portapapeles
   local y la redirección que el cliente RDP ya tiene.
2. **No se tocó `rdpclip.exe` del equipo remoto**, ni se forzó la
   redirección. Es una política del servidor, no del cliente.
3. **No se leyeron las conexiones guardadas de mRemoteNG.** Solo el atributo
   `Protocol`, y el script que lo hacía descartaba el resto antes de
   imprimir.
4. **No se modificó el motor ASR ni la cadena de transformaciones.** La
   función toma la última transcripción ya procesada.
5. **No se mezcló con el hotfix de selección.** Rama propia desde `develop`.
6. **No se hizo push ni merge a `master`.** El merge `develop → master` es
   una decisión del usuario.
7. **No se arrastró el motor C++ a una máquina nueva ni se clonó la base de
   datos.** La base `SuperStock` no se tocó: este trabajo no la usa.
8. **No se abrió ninguna consola visible.** Todo se ejecutó con
   `scripts/run-hidden.ps1`.

---

## 6. Resultados de la verificación

Todo lo de esta sección se ejecutó de verdad. Los comandos corrieron con
`scripts/run-hidden.ps1`, sin abrir ninguna consola.

### 6.1 Backend

| Comprobación | Resultado |
|---|---|
| `cargo check --lib` | **PASS** — compila sin errores ni avisos. |
| `cargo test --lib` | **PASS** — **538 pruebas pasan, 0 fallan, 18 ignoradas** (las 18 ya estaban ignoradas antes de este trabajo). Incluye las 14 pruebas nuevas de este trabajo. |
| `cargo build --bin dixi` | **PASS** — el binario de debug se genera. |
| Regeneración de `src/bindings.ts` | **PASS** — el contrato TypeScript se regenera con los 13 comandos nuevos (280 líneas añadidas). |

### 6.2 Frontend

| Comprobación | Resultado |
|---|---|
| `tsc --noEmit` | **PASS** — 0 errores de tipos. |
| `eslint src` | **PASS** — 0 problemas. |
| `npm run test:unit` | **PASS** — las tres suites (`keyboard`, `portableInstaller`, `clipboard`) pasan sus aserciones. |
| `npm run check:translations` | **PASS** — *"All 25 languages have complete translations"*. |
| `node scripts/check-translation-text.mjs` | **PASS** — *7056 cadenas revisadas, sin problemas*. |
| `prettier --check` sobre el componente nuevo | **PASS** — el archivo nuevo se crea con LF y cumple la regla `endOfLine: "lf"`. |

### 6.3 Dos comprobaciones que NO se pudieron ejecutar, y por qué

1. **`cargo fmt -- --check`**: el componente `rustfmt` **no está instalado** en
   esta máquina (`rustup component list --installed` muestra solo `cargo`,
   `rust-std` y `rustc`). No se instaló porque requiere descargar el
   componente. El código se escribió siguiendo el estilo del resto del
   proyecto, pero **el formateo automático no está verificado**.
2. **`prettier --check src` sobre el repositorio completo**: **falla en 194
   archivos**, y **fallaba antes de tocar nada**. La causa es que
   `.prettierrc` exige `endOfLine: "lf"` y los archivos del repositorio
   tienen finales de línea CRLF, mezclados.

   Esto **no es una regresión de este trabajo**, y está comprobado:
   - `git diff --numstat` de los 26 archivos de idioma muestra **55
     inserciones y 2 eliminaciones** por archivo. Si el final de línea
     hubiera cambiado, git mostraría el archivo entero como reescrito.
   - Comparando el contenido de cada archivo de idioma con la salida de
     prettier, **son idénticos salvo por el final de línea** (se comprobó con
     `ar`, `bg`, `de`, `en`, `es` y `fr`).
   - El merge preserva deliberadamente el final de línea original de cada
     archivo.

   Reformatar los 194 archivos sería un cambio masivo ajeno a esta
   funcionalidad, así que **no se hizo**.

### 6.4 Un ajuste colateral que sí hizo falta

`scripts/check-translation-text.mjs` marca como error cualquier palabra
latina dentro de un texto escrito en otro alfabeto, para detectar secciones
copiadas a medio traducir. Las claves nuevas usan `mRemoteNG` y `RDP`, que
son nombres propios y **no deben traducirse**, así que el validador los
flagaba en árabe, hebreo, hindi, nepalí y búlgaro (25 avisos).

El arreglo fue **ampliar la lista blanca del validador** con `mremoteng` y
`rdp`, que es lo que corresponde: son nombres propios, no traducciones
pendientes. Con ese ajuste el validador pasa limpio.

### 6.5 Una nota de coordinación

Durante la ejecución, otro agente estaba trabajando en paralelo en el hotfix
de selección, en un clon aparte (`D:\Codigo\Dixi-hotfix`). Al regenerar sus
propios `bindings.ts` con su binario, **el archivo de este clon quedó
regenerado con el contrato viejo** y perdió los comandos nuevos.

La causa es que la ruta de exportación (`../src/bindings.ts`) es **relativa al
directorio de trabajo** del proceso: ejecutar el binario desde el directorio
equivocado escribe en el repositorio equivocado.

Los bindings de esta rama se regeneraron al final con el binario de este clon
y verificados. **Al integrar el hotfix hay que regenerar los bindings otra
vez**, porque cada rama tiene su propio conjunto de comandos.

---

## 7. Riesgos y límites conocidos

| Riesgo | Estado | Mitigación |
|---|---|---|
| El texto queda en el portapapeles local si el usuario nunca confirma | Aceptado | Vence solo (3 minutos por defecto, configurable) y la restauración es segura. |
| La app se cierra con una transferencia pendiente | **Corregido** | El texto se publica con renderizado diferido, así que al morir el proceso Windows lo descarta y el portapapeles quedaba vacío. Ahora la transferencia se liquida en `RunEvent::Exit` y se espera a que la restauración se complete. |
| Formatos del portapapeles que no se pueden restaurar | Parcial | Se conservan los que sí se pueden; los demás se nombran en el aviso. |
| **La redirección del portapapeles de la sesión RDP no transporta el texto** | **Confirmado, fuera del alcance de Dixi** | El texto queda en el portapapeles local, que es lo que el modo manual garantiza, pero no llega al equipo remoto. La causa está en el cliente o en la política del servidor (ver 2.6). |
| El modo automático pega en el campo equivocado | Mitigado, no eliminado | Requiere foco verificado dos veces y una sola sesión; desactivado por defecto. Además hereda el bloqueo del canal de portapapeles. |
| Un gestor de portapapeles que lee al publicar | Mitigado | Se registra como recibo, pero no cierra la transferencia ni restaura el portapapeles. |

---

## 8. Archivos tocados

### Backend (`src-tauri/`)

| Archivo | Qué cambió |
|---|---|
| `src/remote_desktop/mod.rs` | Nuevo. Detección, máquina de estados, comandos de dominio y pruebas. |
| `src/remote_desktop/detect.rs` | Nuevo. Reconocimiento de mRemoteNG y de las superficies RDP. |
| `src/commands/remote_desktop.rs` | Nuevo. Comandos Tauri y los interruptores de configuración. |
| `src/paste_tx/mod.rs` | Disparador nuevo, cierre por confirmación y pruebas. |
| `src/paste_tx/windows.rs` | Publicación sin acorde, formatos irrecuperables, aviso de cierre. |
| `src/settings.rs` | Seis opciones nuevas y el atajo `prepare_remote_paste`. |
| `src/actions.rs` | La acción del atajo. |
| `src/shortcut/mod.rs` | Registro condicional del atajo y refresco al cambiar el interruptor. |
| `src/tray.rs` | El elemento de bandeja y el texto de la transcripción, ahora compartido. |
| `src/lib.rs` | Módulo, estado gestionado, comandos y manejador de la bandeja. |

### Frontend (`src/`)

| Archivo | Qué cambió |
|---|---|
| `components/settings/remoteDesktop/RemoteDesktopSettings.tsx` | Nuevo. La sección de Ajustes. |
| `components/settings/index.ts` | Exporta el componente. |
| `components/Sidebar.tsx` | La sección, solo en Windows. |
| `stores/settingsStore.ts` | Los seis actualizadores de opción. |
| `i18n/locales/*/translation.json` | Las claves nuevas en los 26 idiomas. |
| `bindings.ts` | Regenerado con los comandos nuevos. |

---

## 9. La prueba real: qué se comprobó y qué falta

La prueba se hizo contra la sesión RDP real del usuario, con la
autorización de él. Esto es lo que quedó verificado:

| Paso | Resultado | Evidencia |
|---|---|---|
| La detección del cliente y de la sesión | **PASS** | `client_running: true`, `rdp_sessions: 1` |
| La detección del foco dentro de la sesión | **PASS** | `focused: RdpSession` con la ventana enfocada en `IHWindowClass` |
| El atajo global prepara la transcripción | **PASS** | `[manual-paste] transcript published` + `[remote-desktop] transcript prepared for a manual paste (99 characters)` |
| El portapapeles local recibe el texto | **PASS** | hash `EC9B4F21…` contra la línea base `B0972068…` |
| El cliente remoto lee el portapapeles | **PASS** | `clipboard read 4158ms after the trigger` |
| El usuario confirma el pegado | **PASS** | `[remote-desktop] the user confirmed the paste after 51s` |
| La restauración del portapapeles | **PASS** | `[reliable-paste] restored previous clipboard` |
| La guarda de propiedad | **PASS** | `[reliable-paste] clipboard changed externally; leaving it untouched` |
| **El texto llega al equipo remoto** | **FAIL** | El usuario confirmó que el texto no apareció en la sesión RDP. |

Lo que falta para que la función cumpla su objetivo final **no está en el
código**: está en el canal de portapapeles de la sesión RDP, y su causa está
documentada en 2.6.

### Procedimiento para reintentarlo cuando el canal esté resuelto

1. Con una sesión RDP abierta en mRemoteNG y el foco en la sesión remota.
2. Dictar en el equipo personal.
3. Con `Ctrl+Alt+Espacio`, preparar la última transcripción.
4. Comprobar en la sesión remota que el texto llegó.
5. Confirmar con "Ya lo pegué" y comprobar que el portapapeles local volvió a
   su contenido anterior.
6. Anotar cuánto tardó la sincronización. **La medición que sí se tiene es
   de ~4,1 s** entre la publicación y la lectura del portapapeles, con lo
   que el valor por defecto de 3 minutos es holgado.

