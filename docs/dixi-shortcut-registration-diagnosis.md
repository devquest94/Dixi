# Diagnóstico: el registro de los atajos globales

**Rama:** `fix/dixi-selection-capture`
**Síntoma:** la GUI de Dixi muestra «El atajo ctrl+alt+c no se pudo registrar:
otra aplicación ya lo tiene», y sin embargo el atajo **sí** funciona: se
registró y se despachó.

Este documento explica qué pasaba, por qué, y qué se cambió.

---

## 1. Lo que dice la evidencia

El log de la sesión real del usuario
(`%LOCALAPPDATA%\com.dixi.app\logs\dixi.log`, 21-10-2026 02:09-02:12):

```
[02:09:07][dixi_app_lib::shortcut::handy_keys][DEBUG] Registered handy-keys
    shortcut: capture_selection -> Hotkey { modifiers: Modifiers(CTRL_LEFT |
    OPT_LEFT | CTRL_RIGHT | OPT_RIGHT), key: Some(C) }
[02:09:48][dixi_app_lib::shortcut::handy_keys][DEBUG] handy-keys event:
    binding=capture_selection, hotkey=ctrl+alt+c, state=Pressed
```

El atajo se registró y se despachó seis veces. El backend nunca dijo que
hubiera un problema con el registro. **El que mintió fue el estado que la
interfaz leía.**

## 2. La causa raíz: el estado se escribía en un solo sitio, y ese no era el del arranque

El hotfix anterior añadió un booleano, `CAPTURE_SHORTCUT_REGISTERED`, para
que la interfaz pudiera avisar cuando el atajo no se registra. El booleano se
escribía **solo** en `shortcut::register_shortcut` de
`src-tauri/src/shortcut/mod.rs`.

Pero el atajo de captura se registra en el arranque por otra ruta:

| Ruta | ¿Pasa por `shortcut::register_shortcut`? |
|---|---|
| `shortcut::init_shortcuts` → `tauri_impl::init_shortcuts` | **No** — llama a `tauri_impl::register_shortcut` |
| `shortcut::init_shortcuts` → `handy_keys::init_shortcuts` | **No** — llama a `state.register` directamente |
| `register_all_shortcuts_for_implementation` (cambio de backend) | **No** — llamaba a las implementaciones |
| `secure_input.rs` (sombras de Secure Input) | No, y **a propósito** (ver 5) |

Y el arranque es justo la ruta que decide si el atajo está en efecto. El
resultado era que el booleano se quedaba en `false` —su valor inicial—
desde el primer instante, mientras el atajo estaba registrado y funcionaba.

Peor aún: `init_shortcuts` **sí** escribía en el log
(`Registered handy-keys shortcut: capture_selection`), así que el log y la
interfaz se contradecían. El log decía «registrado» y la interfaz decía
«no se pudo registrar».

## 3. El segundo defecto: el aviso culpaba a otra aplicación sin pruebas

El mensaje se escribía cuando `registered == false`, **sin mirar por qué**.
Y el motivo que se decía era siempre el mismo: «otra aplicación ya lo
tiene».

Eso no era cierto, y no puede serlo con el backend que Dixi usa en
Windows:

- **handy-keys** (el backend por defecto en Windows) vigila el teclado con un
  **hook de bajo nivel** (`SetWindowsHookExW`) y guarda su propia lista de
  combinaciones **en memoria**. Nunca pide al sistema que reserve la
  combinación. Su `manager.register()` solo comprueba su propio mapa, así
  que su error `Hotkey already registered` significa **«Dixi registró esta
  combinación dos veces»**, no que otro programa la tenga.
- **Tauri** (`tauri-plugin-global-shortcut` → `global-hotkey`) sí usa
  `RegisterHotKey`. Ahí el sistema puede negarse, y el error del sistema es
  `ERROR_HOTKEY_ALREADY_REGISTERED`, que el plugin traduce a
  `HotKey already registered: ...`. **Ese sí es un conflicto real.**

O sea: con handy-keys, **ninguna otra aplicación puede entrar en conflicto**.
El aviso era falso por construcción, y mandaba al usuario a cambiar los
atajos de un programa que no tenía nada que ver.

## 4. El tercer defecto: la interfaz leía el estado una sola vez

El panel de Transformaciones leía el estado al montarse y no lo volvía a
mirar. Aunque el estado se hubiera escrito bien, un cambio posterior —cambiar
la combinación, reiniciar Dixi, el arranque— no se habría visto. Y al
reiniciar Dixi, el panel se monta y lee: si el registroTodavía no había
ocurrido, el falso negativo volvía.

## 5. Lo que se cambió

### 5.1 Un solo sitio decide el estado

Se crea `src-tauri/src/shortcut/capture_state.rs`, con el estado, la
clasificación del fallo y sus pruebas. Todas las rutas de registro pasan ahora
por `shortcut::register_shortcut_with`, que es la que escribe el estado,
lo registra en el log y avisa a la interfaz:

- `tauri_impl::init_shortcuts` usa la ruta común.
- `handy_keys::init_shortcuts` usa la ruta común (y gestiona su estado
  **antes** de registrar, que es lo que la ruta común necesita).
- `register_all_shortcuts_for_implementation` usa la ruta común con la
  implementación que se está aplicando, no con la que está en los ajustes.

Las sombras de Secure Input siguen yendo directas a la implementación de
Tauri, **a propósito**: una sombra es otra combinación —más ancha— que cubre
al atajo mientras Secure Input tiene el teclado. El estado del atajo de
captura describe la combinación configurada, y esa conserva su registro.

### 5.2 El estado dice qué pasó, no solo si pasó

```
registered                        si el atajo esta en efecto
attempted                         si hubo un intento
binding                           la combinacion
backend                           tauri | handy_keys
error_code                        conflict | duplicate | invalid_combination
                                  | not_initialized | backend_unavailable | unknown
error                             lo que dijo el backend
conflicts_with_another_application si el fallo es de otra aplicacion
attempted_at                      cuando se intento
available                         si este sistema puede capturar
```

### 5.3 El motivo se clasifica, y solo uno culpa a otra aplicación

| Código | Qué es | ¿Culpa a otra app? |
|---|---|---|
| `conflict` | El sistema se negó a reservar la combinación. **Solo con el backend Tauri.** | Sí |
| `duplicate` | Dixi ya tenía esa combinación registrada. | No |
| `invalid_combination` | La combinación no se puede interpretar. | No |
| `not_initialized` | El motor de atajos no estaba listo. | No |
| `backend_unavailable` | El motor de atajos no respondió. | No |
| `unknown` | Cualquier otra cosa. | No |

La clasificación es conservadora a propósito: un texto que no significa
claramente una cosa es `unknown`, y `unknown` no acusa a nadie.

### 5.4 Cada intento queda en el log

Con fecha (la del propio log), backend, combinación, resultado y código:

```
Shortcut registration: id=capture_selection backend=handy_keys
    combination=ctrl+alt+c result=ok
Shortcut registration: id=capture_selection backend=tauri
    combination=ctrl+alt+c result=error code=Conflict detail=...
```

No lleva nada privado: una combinación de teclas es un ajuste del usuario.

### 5.5 La interfaz se entera de los cambios

El backend emite el evento `capture-shortcut-status` cada vez que el estado
cambia, y el panel se suscribe. Además lo lee al montarse. Con eso, el aviso
refleja el estado real también después de reiniciar Dixi.

El aviso distingue los casos: si el problema es que otra aplicación tiene la
combinación, lo dice; si el problema es de Dixi, dice eso, y no manda al
usuario a cambiar los atajos de otro programa.

## 6. Pruebas de regresión

En `src-tauri/src/shortcut/capture_state.rs`, sin escritorio:

| Prueba | Qué fija |
|---|---|
| `a_successful_registration_leaves_the_shortcut_registered` | Un registro bien va deja el atajo registrado. |
| `the_state_that_is_read_is_the_state_that_was_recorded` | Lo que se lee es lo que se escribió. |
| `a_real_conflict_is_reported_as_a_conflict` | Un conflicto real del backend Tauri se reporta como conflicto. |
| `the_duplicate_of_handy_keys_is_not_a_conflict_with_another_application` | Un duplicado interno **no** se culpa a otra aplicación. |
| `an_initialization_failure_is_not_reported_as_a_conflict` | Un motor no listo no se culpa a otra aplicación. |
| `an_invalid_combination_is_told_apart_from_a_conflict` | Una combinación inválida se distingue de un conflicto. |
| `an_unknown_failure_accuses_nobody` | Lo desconocido no acusa a nadie. |
| `changing_the_combination_updates_the_state` | Cambiar la combinación actualiza el estado. |
| `before_the_first_attempt_there_is_nothing_to_report` | Sin intento no hay nada que achacar. |
| `releasing_the_shortcut_marks_it_as_not_registered` | Liberar el atajo lo deja como no registrado. |
| `releasing_a_shortcut_that_was_never_registered_says_nothing` | Liberar lo que no se registró no inventa nada. |
| `changing_the_combination_releases_the_previous_one` | El cambio libera la anterior y deja la nueva en efecto. |
| `a_failed_change_falls_back_to_the_previous_combination` | Un cambio fallido vuelve a la anterior, sin conflicto. |

## 7. Lo que queda pendiente de probar a mano

Estas necesitan el escritorio y **no** se han hecho:

| Prueba | Cómo |
|---|---|
| El aviso ya no aparece al arrancar Dixi | Arrancar Dixi y abrir Ajustes → Transformaciones: no debe salir el aviso. |
| El aviso aparece cuando el atajo **sí** falla | Cambiar la combinación a una que otra aplicación tenga (con el backend Tauri) y ver el aviso de conflicto. |
| Cambiar la combinación desde General la registra al instante | Ajustes → General → Atajos → «Capturar la selección» → combination nueva, y comprobar que el atajo nuevo funciona y el viejo no. |
| El atajo captura y el texto llega a Transformaciones | Bloc de notas → seleccionar → `Ctrl+Alt+C` → el texto aparece en Transformaciones. |
| El estado sobrevive a reiniciar Dixi | Reiniciar y volver a abrir el panel: el aviso tiene que seguir reflejando la realidad. |
