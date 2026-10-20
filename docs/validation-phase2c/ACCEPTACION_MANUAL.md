# Aceptación manual — Fase 2C (perfiles, fragmentos, transforms e historial)

Todo lo de la Fase 2C se comprobó automáticamente, salvo dos cosas que
necesitan **una ventana de Windows delante y tu criterio**: que la interfaz
muestre lo que toca, y que el flujo de *seleccionar → transformar → aceptar*
escriba de verdad en la aplicación de destino.

Esta es la lista para hacerlo a mano. Tardar unos diez minutos.

---

## 0. Preparación

1. Lanzá Dixi desde el binario de esta fase:

   ```
   D:\Codigo\Dixi\src-tauri\target\release\dixi.exe
   ```

2. Abrí el **Bloc de notas** y escribí en una línea:

   ```
   hoy vamos a revisar el informe del trimestre
   ```

   Dejalo abierto, con el cursor dentro. Va a ser la aplicación de destino.

---

## 1. Los cuatro paneles están y están traducidos

En **Ajustes**, en la barra lateral, tienen que aparecer:

| Entrada | Qué comprueba |
|---|---|
| **Perfiles de escritura** | El panel de perfiles, con estilos y perfiles de serie. |
| **Fragmentos** | El panel de fragmentos. |
| **Transforms** | El panel de transforms, con los catorce de serie. |
| **Historial de texto** | El historial de cambios de texto. |

**Traducción:** cambiá el idioma de la aplicación (Ajustes → General → Idioma)
a un idioma que no sea el tuyo —por ejemplo **árabe** o **japonés**— y abrí
los cuatro paneles. Ninguna etiqueta puede quedar en inglés o en el idioma
anterior: tiene que estar traducida entera.

> El nombre de la marca **Dixi** y los nombres de producto (JSON, GitHub,
> macOS) **se quedan en su idioma**: es lo correcto.

---

## 2. Perfiles: la configuración que se va a usar

1. En **Ajustes → Perfiles de escritura**, mirá la tarjeta
   **«Aplicación detectada»**. Con el Bloc de notas en primer plano tiene que
   decir `notepad`. Con otra ventana delante, el nombre cambia.
2. Mirá **«Configuración efectiva»**: dice qué perfil se va a usar, y de dónde
   sale (global, el perfil que coincidió, una regla atada a la aplicación, o
   una anulación manual de esta sesión).
3. Creá un perfil propio: nombre, una aplicación (`notepad`), un estilo, un
   nivel de limpieza y una regla de formato (**«Punto final»**, por ejemplo).
   Guardalo.
4. **Atributos de estilo**, en el mismo panel, dejá el estilo «Formal» con
   las reglas de formato que quieras. Probá **«Punto final»** y **«Espacios
   duplicados»**.

**Lo que hay que ver:** cambiar el perfil cambia la configuración efectiva, y
la origen de la configuración se lee sin ambigüedad.

---

## 3. Fragmentos: las variables

1. En **Ajustes → Fragmentos**, creá uno:

   | Campo | Valor |
   |---|---|
   | Nombre | `Saludo` |
   | Frase de activación | `hola equipo` |
   | Texto | `Hola {nombre}, hoy es {fecha} a las {hora}` |
   | Variable `nombre` | **obligatoria**, pregunta: «¿Cómo te llamás?» |

2. **La vista previa** de abajo: escribí `hola equipo` y pulsá **«Expandir»**.
   Tiene que preguntarte el nombre, y al completarlo mostrar la frase con la
   fecha y la hora del sistema ya puestas.

3. Con dos fragmentos que compartan la **misma frase de activación**,
   fijá prioridades distintas y mirá **«En competencia»**: el que gana es el
   de mayor prioridad, y si empatan, el alfabéticamente menor.

4. Marcá un fragmento con la **estrella** de favorito, **editálo** y guardá.
   La estrella **no se puede perder**: es lo que decide el orden.

**Lo que hay que ver:** `{fecha}` y `{hora}` se rellenan solas, y una variable
obligatoria que falta **impide** la expansión: el texto no se pega a medias.

> Si declarás una variable llamada `fecha` u `hora`, el panel te lo rechaza:
> son variables de la máquina, no tuyas.

---

## 4. Transforms: el flujo completo (lo importante)

1. En el **Bloc de notas**, seleccioná con el ratón toda la frase
   `hoy vamos a revisar el informe del trimestre`.
2. Volvé a Dixi, a **Ajustes → Transforms**, y pulsá **«Capturar la
   selección»**. El texto capturado tiene que ser el de la línea.
3. Elegí un transform (por ejemplo **«Mejorar la redacción»** o **«Resumir»**)
   y pulsá **«Ejecutar»**.
4. Aparece el **diff por palabras**: lo añadido y lo quitado, contados por
   palabras, no por letras.
5. Pulsá **«Aceptar»**.

**Lo que hay que ver:** el Bloc de notas cambia solo, con el texto
transformado, y **sin que el portapapeles quede pisado**: copiá algo en el
Bloc de notas después de aceptar y tiene que seguir funcionando.

### Si el texto no cambia

Es el caso que estaba roto. Si **«Aceptar»** no escribe nada y aparece el
aviso de que la selección cambió, anotá lo que dice el aviso: el aviso
también sale cuando la selección **sí** cambió de verdad, y hay que
distinguir los dos casos.

### Si el transform falla

Con un provider de IA sin clave, el transform falla. Lo que tiene que pasar:
el resultado **no** se acepta, el botón **«Aceptar»** queda deshabilitado y
el texto de la aplicación de destino **no se toca**.

---

## 5. Historial de texto: deshacer

1. Mirá **Ajustes → Historial de texto**: el cambio del punto anterior tiene
   que estar, con el texto anterior y el resultado.
2. Volvé al Bloc de notas, seleccioná el texto que ahora dice Dixi y pulsá
   **«Revertir»** en el historial.

**Lo que hay que ver:** el Bloc de notas vuelve al texto original, y la
entrada del historial queda marcada como revertida.

3. En **Ajustes → Historial de texto**, cambiá la **retención** a `7` días y
   guardá. Después poné `12abc` y pulsá guardar: **tiene que rechazar el
   valor**, no aceptarlo como 12.

---

## 6. Lo que no tiene que pasar nunca

- **Un campo de contraseña no se lee ni se escribe.** Si el Bloc de notas
  estuviera sustituido por una ventana con un campo de contraseña, la captura
  tiene que fallar con un aviso, no devolver nada en claro.
- **El portapapeles queda como estaba.** Antes de usar Dixi, copiá algo; al
  terminar, el portapapeles tiene que seguir teniendo lo mismo.
- **Un fragmento con una variable obligatoria que falta no se pega a medias.**

---

## 7. Si algo falla

Anotá qué se vio y en qué punto. Lo que más ayuda:

- el texto exacto del aviso,
- en qué paso de esta lista estabas,
- si el caso es «Aceptar»/«Revertir» (eso es el camino que estaba roto) o
  es la traducción.

Los logs de la aplicación están en **Ajustes → Depuración → Registros en
vivo**.
