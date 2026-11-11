# Procedimiento manual del hotfix de la captura

Diez minutos. Es la lista corta de este arreglo; la larga está en
`docs/validation-phase2c/ACCEPTACION_MANUAL.md`.

Lo que hay que tener abierto a la vez: **el Bloc de notas** (el destino) y
**Dixi**.

---

## 0. Preparación

1. Lanzá Dixi desde el binario de este hotfix:

   ```
   D:\Codigo\Dixi\src-tauri\target\release\dixi.exe
   ```

2. Abrí el **Bloc de notas** y escribí una línea:

   ```
   hoy vamos a revisar el informe del trimestre
   ```

3. **Antes de nada, copiá algo al portapapeles** (por ejemplo, copiá esa
   línea con `Ctrl+C`). Más abajo se explica por qué esto importa.

---

## 1. La captura (la vía normal)

1. En el Bloc de notas, **seleccioná toda la línea** con el ratón.
2. **Sin tocar el Bloc de notas, pulsá `Ctrl+Alt+C`.**

   > Ese es el atajo global de captura. Si no hace nada, mirá
   > Ajustes → General → Atajos: puede que otra aplicación lo tenga
   > cogido. Cambialo ahí. El panel de Transforms avisa cuando el atajo
   > **no** se pudo registrar.

3. Abrí Dixi → **Ajustes → Transforms**. El texto capturado tiene que
   estar, con el nombre de la aplicación de donde salió.

**Lo que hay que ver:** el texto capturado es el del Bloc de notas, **no**
el de la ventana de Dixi. Si saliera el texto de Dixi o un error, el
problema es que el atajo no se registró.

**Si el aviso dice «no hay texto seleccionado»**, ahora el aviso dice también
**qué aplicación tenía el foco**:

> No hay texto seleccionado en la aplicación que tiene el foco. La
> aplicación que tenía el foco era notepad.exe.

Eso es la prueba de que el atajo va a la aplicación correcta. Si en cambio
dice «*la aplicación que tenía el foco era el propio Dixi*», el atajo se
pulsó con Dixi delante: volvé al Bloc de notas y pulsalo otra vez.

## 2. La cuenta atrás (la alternativa)

1. En Dixi → Ajustes → Transforms, pulsá **«Capturar ahora»**.
2. Tenés 4 segundos: **volvé al Bloc de notas**, seleccioná la línea y
   esperá.
3. Al terminar la cuenta atrás, Dixi tiene que mostrar el texto capturado.

---

## 3. Aceptar (escribir de vuelta)

1. Con el texto capturado, elegí un transform y pulsá **«Ejecutar»**.
2. Mirá el **diff por palabras**.
3. Pulsá **«Aceptar»**.

**Lo que hay que ver, en este orden:**

- el Bloc de notas **cambia solo**, con el texto transformado;
- el **portapapeles sigue igual** que antes (si habías copiado la línea,
  `Ctrl+V` en un lugar vacío tiene que pegar la línea, no otra cosa);
- en **Ajustes → Historial de texto** aparece el cambio, con el texto
  anterior y el nuevo.

**Un caso a proposito:** escribí algo a mano en el área de texto de
Transforms, después de capturar. «Aceptar» tiene que quedar **deshabilitado**
y salir un aviso de que la captura ya no vale: la selección original ya no
está, y aceptar escribiría sobre otra cosa. «Copiar» sí que sigue valiendo.

---

## 4. Que no se escriba cuando no debe

These son los casos en los que Dixi tiene que **cancelar y no escribir nada**.

| Qué haces | Qué tiene que pasar |
|---|---|
| Seleccionás la línea, pulsás `Ctrl+Alt+C`, volvés al Bloc de notas y **movés la selección** antes de aceptar | Aviso «la selección cambió». El Bloc de notas **no** cambia. |
| Capturás, **cerrás el Bloc de notas** y después aceptás | Aviso «la aplicación está cerrada». Nada se escribe. |
| Copiás una **imagen** (o unos archivos) al portapapeles y después capturás | Aviso de que el portapapeles tiene contenido que no se puede devolver. El portapapeles **sigue con la imagen**. |
| Con el cursor dentro de un **campo de contraseña** de una página, pulsás `Ctrl+Alt+C` | Aviso de que es un campo de contraseña. **No** se lee nada. |

> El caso de la imagen es a propósito: Dixi solo usa el portapapeles cuando
> tiene texto, porque no sabe devolver una imagen exactamente igual. Si
> tenés una imagen copiada, **se rechaza la operación** en vez de romper tu
> portapapeles.

---

## 5. Revertir

1. En **Ajustes → Historial de texto**, el cambio de antes tiene que estar.
2. Volvé al Bloc de notas, **seleccioná el texto que puso Dixi**.
3. Pulsá **«Revertir»** en el historial.

**Lo que hay que ver:** el Bloc de notas vuelve al texto original, y la
entrada del historial queda marcada como revertida.

> «Revertir» devuelve el Bloc de notas al frente por su cuenta. Si el
> texto que está seleccionado no es el que puso Dixi, no toca nada y lo
> avisa.

---

## 6. El atajo, si estorba

**Ajustes → General → Atajos** tiene una entrada nueva, «Capturar la
selección». Se cambia ahí como cualquier otro atajo, y el cambio se aplica
enseguida.

Si el atajo **no** se pudo registrar, el panel de Transforms lo dice arriba
del todo, con la combinacion concreta. Un atajo global que no se registra
**no da error visible**: sencillamente nunca se dispara.

El aviso dice **por que** fallo, y no culpa a otra aplicacion a ciegas:

| Que dice | Que quiere decir |
|---|---|
| «otra aplicación ya tiene esa combinación» | El sistema se negó a reservarla. Solo con el backend Tauri. |
| «Dixi ya tenía esa combinación registrada» | Un duplicado interno: reiniciá Dixi o cambiá la combinación. |
| «el motor de atajos no estaba listo» | Fallo de arranque: reiniciá Dixi. |
| «no es una combinación válida» | La combinación no existe: cambiá la anterior. |

Y si el atajo **sí** está registrado, no sale ningún aviso.

---

## 7. Si algo falla

Anota **el texto exacto del aviso** y en qué punto de esta lista estabas.
El aviso ahora dice **qué aplicación tenía el foco**, así que esa parte ya
no hay que adivinarla.

Si el fallo es en «Aceptar» o en «Revertir», eso es el camino que estaba
roto: cuéntalo con esas palabras.

Los logs están en **Ajustes → Depuración → Registros en vivo**. La línea
técnica de la captura tiene esta forma, y **no lleva el texto** que
seleccionaste:

```
CaptureSelectionAction: app=chrome.exe class=Chrome_WidgetWin_1 step=copy
    result=error code=nothing_selected
```

| Parte | Qué es |
|---|---|
| `app` | El proceso que tenía el foco. |
| `class` | La clase del control enfocado. |
| `step` | En qué paso falló: `focus`, `control` o `copy`. |
| `code` | El motivo, con el código estable que la interfaz traduce. |
