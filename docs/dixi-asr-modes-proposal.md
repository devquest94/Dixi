# Propuesta de dos modos de dictado — Dixi

Estado: **propuesta para aprobacion. Los dos modos NO estan implementados.**

Este documento diseña la seleccion explicita del motor de dictado **antes de
empezar a grabar**, a partir de lo medido en
`docs/dixi-nemotron-streaming-evaluation.md` y de lo verificado en
`docs/dixi-streaming-stability-report.md`.

Tres correcciones importantes respecto a la primera version de este
documento:

1. **No existe un umbral de duracion demostrado.** La idea de que a partir de
   "14-18 s" el streaming cambiaba de regimen era un artefacto de la maquina
   que se uso para medir, no una propiedad del motor (seccion 2).
2. **El atajo `Ctrl+Alt+Espacio` no se usa ni se sugiere.** Esta reservado a
   la integracion con mRemoteNG (seccion 5.3).
3. **La cola ya no se acota descartando audio.** La recuperacion se hace
   terminando la grabacion en una sola pasada sobre el audio original completo
   (seccion 7).

---

## 1. Por que dos motores y no uno

Los dos motores no compiten en lo mismo:

| | Whisper Base | Nemotron 3.5 Streaming |
|---|---|---|
| Que hace | Decodifica la grabacion entera al final | Decodifica mientras hablas |
| Espera tras soltar | Proporcional al audio: ~170 ms por segundo de voz | El vaciado del retraso pendiente |
| Mediana (22,34 s de voz) | 1 246 ms | 278-429 ms |
| Cola adversa | Acotada: es la decodificacion entera | **18 891 ms y 68 806 ms** con el equipo cargado |
| RAM del arbol | 198 MB | 1 384 MB |
| Carga en frio | 171-513 ms | 1 165-2 493 ms |
| Modelo en disco | 81 MB | 716 MB |
| Idiomas | 99 | 28 |
| Traduce a ingles | Si | No |

El resumen honesto: **Nemotron es mucho mejor cuando la maquina llega**, y
**mucho peor cuando no llega**. Whisper es predecible: su espera es el coste
de decodificar, y se puede llamar antes de empezar.

Con la recuperacion implementada (seccion 7), la diferencia ya no es de
"cuanto se espera" sino de "cuanto se ve mientras se habla" y de "cuanto
ocupa el motor".

---

## 2. La duracion NO decide el modo

La primera version de este documento proponia un modo "Rapido" para dictados
cortos y un modo "Prolongado" para dictados largos, con un punto de corte
medido. **Ese punto de corte no existe.**

Lo que se vio en las mediciones iniciales (14,40 s de audio costaban ~6 s y
17,82 s costaban ~325 ms) era la capacidad de la maquina de ese momento, no
una propiedad del motor:

- Con el mismo audio de 14,40 s, la espera fue de **3 481-6 021 ms** con la
  maquina saturada y de **228-1 286 ms** con la maquina libre.
- El barrido de duraciones (de 4 s a 38 s, sobre el mismo material) no mostro
  ningun salto en 14-18 s.
- El bisectionamiento del contenido (cortar el audio por la mitad y repetir)
  no mostro ninguna transicion.

Lo que si se verifico en el codigo nativo (`model.cpp`): el coste por
fragmento es **lineal** y el `stream_finalize` mete **todo** el audio pendiente
en **un solo fragmento**, cuya atencion es **cuadratica** en lo pendiente
(`model.cpp:3774`). Por eso el final explota cuando la maquina ya va
saturada, y por eso el disparador es la capacidad de la maquina.

**Consecuencia para el diseno**: el modo no se deduce de lo que el usuario va
a decir. El usuario lo elige (o lo decide el ajuste), y el cambio de motor
solo tiene sentido **antes** de empezar a grabar, porque cambiarlo en mitad de
una grabacion dejaria el audio a medias en dos motores.

---

## 3. Modo "Whisper" (Whisper Base)

Es el modo por defecto y no hay que cambiar nada.

- Es el que usa la app hoy.
- Su espera es conocida de antemano: el usuario sabe que tras un dictado de
  20 s habra unos 2 s de espera.
- No depende de que el equipo este libre: la decodificacion ocurre al final
  y la espera crece de forma proporcional, sin saltos.
- Ocupa 198 MB en vez de 1 384 MB.
- Cubre 99 idiomas y traduce a ingles.

**Cuando usarlo**: dictaciones cortas, equipos con otra cosa corriendo,
cuando se necesita traducir a ingles, o cuando se habla un idioma fuera de
los 28 de Nemotron.

## 4. Modo "Nemotron" (Nemotron 3.5 Streaming)

Es el modo con el texto en vivo, con la condicion de que el equipo pueda
seguir el ritmo.

- El texto **aparece mientras se habla** (la vista previa en vivo ya existe
  en la app), asi que el usuario ve el texto construirse.
- Al soltar la tecla solo queda la cola: 278-429 ms de mediana.
- Si la maquina no llega, el retraso pendiente crece y el cierre se paga de
  forma super-lineal. **La recuperacion de la seccion 7 lo acota sin perder
  audio**, pero el tiempo hasta el texto final es mayor que con Whisper.
- Ocupa 1 384 MB: con el equipo justo, el propio streaming es un problema.

**Cuando usarlo**: dictaciones largas o en las que conviene ver el texto
construirse, con el equipo razonablemente libre.

---

## 5. Como se selecciona el modo (antes de grabar)

### 5.1 Opcion A — un interruptor en la bandeja, mas un atajo por modo

- La bandeja muestra el modo armado: **Whisper** o **Nemotron**.
- Un clic en el modo lo cambia. El atajo de dictado existente (`ctrl+space`)
  arranca en el modo armado.
- Ademas, un atajo configurable por modo (no hay ninguno por defecto).

**A favor**: un solo gesto, sin navegar; el usuario elige el modo con el
mismo gesto con el que dicta. El atajo nuevo es opcional: si no se configura,
el modo se cambia desde la bandeja.

**En contra**: un atajo mas que puede chocar con otro; el usuario tiene que
saber de antemano cual de los dos quiere.

### 5.2 Opcion B — un ajuste, y el atajo existente lo respeta

- En ajustes: **Motor de dictado: Whisper | Nemotron**.
- El atajo de dictado arranca siempre en el modo configurado.
- La bandeja muestra el modo activo.

**A favor**: cero atajos nuevos, cero conflictos, un solo sitio donde mirar.

**En contra**: cambiar de modo exige entrar en ajustes o en la bandeja: un
gesto de mas en cada cambio.

### 5.3 Opcion C — las dos, con la bandeja mandando

- Ajuste con el modo por defecto (Whisper).
- La bandeja permite cambiarlo al instante (gana al ajuste).
- Un atajo opcional por modo para quien prefiera un solo gesto.

**Recomendacion: la C**, con la bandeja comovia rapida y el atajo opcional
como atajo. Es lo que menos friccion añade sin imponer un segundo atajo por
defecto.

### 5.4 El atajo que NO se usa

`Ctrl+Alt+Espacio` esta **reservado a la integracion con mRemoteNG** (copiar la
transcripcion al portapapeles para pegarla en la sesion remota). No se usa
para el modo, no se sugiere y no se ofrece como valor por defecto de ningun
atajo nuevo. Si en el futuro se quisiera un atajo por modo, seria
configurable y sin valor por defecto, y tendria que comprobarse contra la
integracion remota antes de ofrecerse.

El atajo de dictado que ya existe (`ctrl+space`, y `ctrl+shift+space` para el
post-procesado) **no cambia**: arranca en el modo configurado.

### 5.5 Que se ve en pantalla

- El icono de la bandeja cambia de tono segun el modo (Whisper: el de ahora;
  Nemotron: un punto o un borde). Es la unica senal de que el modo cambio.
- Un aviso breve al cambiar de modo: "Modo Nemotron". Solo si el cambio no es
  obvio (por ejemplo, al cambiar desde la bandeja).
- Al empezar a grabar, la superposicion muestra el modo. Si el modelo
  todavia no esta cargado, la superposicion dice "preparando el motor" en vez
  de fingir que esta listo: con Nemotron la carga son 1,2-2,5 s y el usuario
  tiene que saberlo.

---

## 6. Los riesgos de tener los dos motores disponibles

### 6.1 Memoria

- Con los dos modelos cargados a la vez: **198 MB + 1 384 MB = ~1,58 GB**
  residentes, mas el proceso de la app. En un equipo de 16 GB con el navegador
  abierto, eso ya se nota.
- Los ficheros en disco: 81 MB + 716 MB = 797 MB.
- **Mitigacion**: cargar solo el motor del modo activo. El ajuste
  `model_unload_timeout` ya existe (por defecto `min5`): con los modos
  alternados, cada cambio de modo es una descarga de 1,2-2,5 s.

### 6.2 El coste del cambio de modo

- Cambiar de modo = descargar un modelo y cargar el otro. Con el modelo en
  cache, 1,2-2,5 s para Nemotron.
- Si el usuario alterna mucho, cada intercambio se paga.
- **Mitigacion**: mantener el modelo del modo activo cargado; descargar el
  otro segun `model_unload_timeout`. El cambio de modo desde la bandeja
  puede avisar: "cambiando de motor (2 s)".

### 6.3 La confusion de dos motores distintos

- Dos motores se comportan distinto con la misma voz: Whisper
  alucina en audio corto; Nemotron no. El usuario notara la diferencia.
- Si el usuario no sabe en que modo esta, puede comparar la exactitud de
  un dictado con la del otro y pensar que hay un fallo.
- **Mitigacion**: el modo visible siempre (bandeja + superposicion), y el
  informe de la superposicion dice que motor se uso.

### 6.4 El riesgo de la cola, que era el grave

Con el equipo cargado, la espera tras soltar llego a **68,8 s** (medido).
Es el riesgo que hay que resolver ANTES de ofrecer el modo prolongado.

**Estado**: resuelto sin perder audio (seccion 7). Lo que queda por decidir es
la EXPERIENCIA: si al usuario le conviene mas esperar con el texto en vivo
o que el pegado tarde un poco mas a cambio de un texto completo.

---

## 7. La recuperacion: terminar en una pasada, sin perder audio

Lo implementado en esta etapa (no es una propuesta: esta medido y probado, y
esta descrito en `docs/dixi-streaming-stability-report.md`):

1. La app mide el **retraso pendiente** con una magnitud propia: el audio
   que ha entregado al motor menos el audio que el motor ya ha
   comprometido. El motor no puede ver las colas de la app, asi que su
   `buffered_ms` diria "sano" justo cuando la grabacion se atasca.
2. Si al soltar la tecla el retraso pendiente supera el limite (2 000 ms por
   defecto, ajustable con `DIXI_STREAM_BACKLOG_LIMIT_MS`), **no se finaliza
   el stream**. La superposicion avisa ("terminando la grabacion") y la
   grabacion se termina en **una sola pasada sobre el audio original
   completo**.
3. Esa pasada es **lineal**, procesa **todas** las muestras y devuelve el
   texto completo. No se descarta ni un fragmento de audio.
4. Si el usuario cancela mientras el motor va retrasado, la grabacion **se
   conserva**: queda el WAV en el historial con el texto vacio, para
   transcribirla otra vez. El audio capturado no se pierde en silencio.

La unica decision de producto que queda abierta es si ese limite de 2 s es el
valor adecuado, o si conviene un valor mayor (mas espera, menos pasos por la pasada completa) o
menor (mas interrupciones). El valor por defecto esta muy por encima del
retraso sano observado (90-930 ms).

---

## 8. Lo que NO se implementa en esta etapa

- Los dos modos de dictado.
- El cambio automatico por duracion de la dictacion (ademas, no hay evidencia
  de que exista un umbral: seccion 2).
- El cambio de modelo en caliente durante una grabacion.
- La activacion de Nemotron por defecto.
- Cualquier cambio en la integracion de mRemoteNG.

El modelo activo del usuario, sus atajos, su historial y sus ajustes quedan
intactos.

---

## 9. Lo que hay que decidir

1. **Opcion de seleccion**: A, B o C (recomendada: C).
2. **El valor del limite de retraso** con el que se entra en la pasada
   completa (hoy 2 000 ms).
3. **Si el modo prolongado avisa antes** de entrar en la pasada completa, o
   si avisa solo despues (hoy avisa al entrar).
4. **La sesion guiada**: es lo que convierte esto en una recomendacion con
   numeros de exactitud. Sin ella, el veredicto sobre Nemotron como motor
   principal sigue siendo provisional.
