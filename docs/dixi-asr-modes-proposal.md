# Propuesta de dos modos de dictado — Dixi

Estado: **propuesta para aprobacion. Nada implementado.**

Este documento diseña la seleccion explicita del modo de dictado **antes de
empezar a grabar**, a partir de lo medido en
`docs/dixi-nemotron-streaming-evaluation.md`. No se implementa aqui el
cambio automatico por duracion: el encargo lo deja expressly para mas
adelante.

---

## 1. Por que dos modos y no uno

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

El resumen honesto: **Nemotron es mucho mejor cuando la maquina llega**,
y **mucho peor cuando no llega**. Whisper es predecible: su espera es el
coste de decodificar, y se puede llamar antes de empezar.

---

## 2. El modo "Rapido" (Whisper Base)

Es el modo por defecto y no hay que cambiar nada.

- Es el que usa la app hoy.
- Su espera es conocida de antemano: el usuario sabe que tras un dictado de
  20 s habrá unos 2 s de espera.
- No depende de que el equipo este libre: la decodificacion ocurre al final
  y la espera crece de forma proporcional, sin saltos.

**When to use it**: dictaciones cortas, equipos con otra cosa corriendo,
cuando se necesita traducir a ingles, o cuando se habla un idioma fuera de
los 28 de Nemotron.

## 3. El modo "Prolongado" (Nemotron Streaming)

Es el modo para dictaciones largas, con la condicion de que el equipo este
libre.

- El texto **aparece mientras se habla** (la vista previa en vivo ya existe
  en la app), asi que el usuario ve el texto construirse.
- Al soltar la tecla solo queda la cola: 278-429 ms de mediana.
- **Requisito previo**: el retraso pendiente tiene que estar acotado (seccion
  6). Sin eso, la cola adversa de 68 s es inaceptable para un uso
  cotidiano.

---

## 4. Como se selecciona el modo (antes de grabar)

### 4.1 Opcion A — un interruptor en la bandeja, mas un atajo por modo

- La bandeja muestra el modo armado: **Rapido** o **Prolongado**.
- Un clic en el modo lo cambia. El atajo de dictado existente (ctrl+space)
  arranca en el modo armado.
- Ademas, dos atajos configurables:
  - `ctrl+space` → arranca en modo Rapido (Whisper).
  - `ctrl+alt+space` → arranca en modo Prolongado (Nemotron).

**A favor**: un solo gesto, sin navegar; el usuario elige el modo con el
mismo gesto con el que dicta. El atajo nuevo es opcional: si no se
configura, el modo se cambia desde la bandeja.

**En contra**: un atajo mas que puede chocar con otro; el usuario tiene que
saber de antemano cual de los dos quiere.

### 4.2 Opcion B — un ajuste, y el atajo existente lo respeta

- En ajustes: **Modo de dictado: Rapido | Prolongado**.
- El atajo de dictado arranca siempre en el modo configurado.
- La bandeja muestra el modo activo.

**A favor**: cero atajos nuevos, cero conflictos, un solo sitio donde mirar.

**En contra**: cambiar de modo exige entrar en ajustes o en la bandeja: un
gesto de mas en cada cambio.

### 4.3 Opcion C — las dos, con la bandeja mandando

- Ajuste con el modo por defecto (Rapido).
- La bandeja permite cambiarlo al instante (gana al ajuste).
- Un atajo opcional por modo para quien prefiera un solo gesto.

**Recomendacion: la C**, con la bandeja comovia rapida y el atajo opcional
como atajo. Es lo que menos friccion añade sin imponer un segundo atajo por
defecto.

### 4.4 Que se ve en pantalla

- El icono de la bandeja cambia de tono segun el modo (Rapido: el de ahora;
  Prolongado: un punto o un borde). Es la unica senal de que el modo cambio.
- Un aviso breve al cambiar de modo: "Modo prolongado (Nemotron)". Solo si
  el cambio no es obvio (por ejemplo, al cambiar desde la bandeja).
- Al empezar a grabar, la superposicion muestra el modo. Si el modelo
  todavia no esta cargado, la superposicion dice "preparando el motor" en vez
  de fingir que esta listo: con Nemotron la carga son 1,2-2,5 s y el usuario
  tiene que saberlo.

---

## 5. Los riesgos de tener los dos motores disponibles

### 5.1 Memoria

- Con los dos modelos cargados a la vez: **198 MB + 1 384 MB = ~1,58 GB**
  residentes, mas el proceso de la app. En un equipo de 16 GB con el navegador
  abierto, eso ya se nota.
- Los ficheros en disco: 81 MB + 716 MB = 797 MB.
- **Mitigacion**: cargar solo el motor del modo activo. El ajuste
  `model_unload_timeout` ya existe (por defecto `min5`): con los modos
  alternados, cada cambio de modo es una descarga de 1,2-2,5 s.

### 5.2 El coste del cambio de modo

- Cambiar de modo = descargar un modelo y cargar el otro. Con el modelo en
  cache, 1,2-2,5 s para Nemotron.
- Si el usuario alterna mucho, cada intercambio se paga.
- **Mitigacion**: mantener el modelo del modo activo cargado; descargar el
  otro segun `model_unload_timeout`. El cambio de modo desde la bandeja
  puede avisar: "cambiando de motor (2 s)".

### 5.3 La confusion de dos motores distintos

- Dos motores se comportan distinto con la misma voz: Whisper
  alucina en audio corto; Nemotron no. El usuario notara la diferencia.
- Si el usuario no sabe en que modo esta, puede comparar la exactitud de
  un dictado con la del otro y pensar que hay un fallo.
- **Mitigacion**: el modo visible siempre (bandeja + superposicion), y el
  informe de la superposicion dice que motor se uso.

### 5.4 El riesgo de la cola, que es el grave

Con el equipo cargado, la espera tras soltar llego a **68,8 s** (medido).
Es el riesgo que hay que resolver ANTES de ofrecer el modo prolongado.

---

## 6. La condicion previa: acotar el retraso pendiente

La mecanica, medida y verificada en el codigo nativo:

1. El motor decodifica en fragmentos de ~1,1 s de audio. Su coste por
   fragmento es constante y el numero de fragmentos es proporcional al
   audio: **el compute es lineal**.
2. La app encola los tramos de audio **sin limite** y el `Finalize` se
   encola **detras** de todos ellos.
3. Al finalizar, el motor mete **todo** el audio pendiente en **un solo
   fragmento**, cuya atencion es cuadratica en lo pendiente
   (`model.cpp:3774`).

De ahi el comportamiento: si el equipo llega, el retraso es cero y la
espera son ~400 ms; si no llega, el retraso crece y al final se paga de
forma super-lineal, y ahi estan los 18,9 s y los 68,8 s.

**Propuesta**: acotar el retraso pendiente en el motor. Cuando el retraso
supere un limite (por ejemplo, 3 s de audio), el motor entrega lo que
tiene en vez de acumular. Coste: se pierde audio, que es peor que tardar.
Contra: acota el peor caso a cambio de perder el final.

**Alternativa, sin perder audio**: que la superposicion muestre el texto
que ya esta decodificado (la vista previa en vivo ya existe) y que el pegado
final espere solo a la cola. El usuario ve el texto antes; solo el "soltar y
pegar" se retrasa. Es una mitigacion de EXPERIENCIA, no de latencia.

**Lo que hace falta medir antes de decidir**: cuantos segundos de retraso
produce la perdida de audio, y si el usuario prefiere perder el final o
esperar. Eso lo puede responder la sesion guiada.

---

## 7. Lo que NO se implementa en esta etapa

- El cambio automatico por duracion de la dictacion.
- El cambio de modelo en caliente durante una grabacion.
- La activacion de Nemotron por defecto.
- Cualquier cambio en la integracion de mRemoteNG.

El modelo activo del usuario, sus atajos, su historial y sus ajustes quedan
intactos.

---

## 8. Lo que hay que decidir

1. **Opcion de seleccion**: A, B o C (recomendada: C).
2. **Que se hace con el retraso pendiente**: acotarlo en el motor (se pierde
   audio) o solo avisar al usuario (se pierde latencia).
3. **Si el modo prolongado exige el equipo descargado**: se puede avisar
   cuando el retraso crece, y dejar que el usuario decida.
4. **La sesion guiada**: es lo que convierte esto en una recomendacion con
   numeros de exactitud, y la necesita antes de decidir el punto 2.
