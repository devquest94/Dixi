# Validacion del ASR en streaming — Dixi

Rama: `feat/dixi-asr-streaming-validation` (desde `develop` @ `40dcb8a`)
Equipo: Intel Core i5-8300H (4 nucleos, 2,30 GHz), CPU unicamente, 16 GB de RAM.

Todo lo que hay aqui **medido**, con el banco de la ruta en vivo
(`scripts/measure-live-latency.ps1`) sobre la voz real del usuario. Donde
falta una medicion, esta escrito; no hay estimaciones.

---

## 1. El hallazgo que cambia el problema

**Con Whisper Base, Dixi no decodifica nada mientras el usuario habla.**

La cadena es esta:

1. `policy::live_decode` decide como se decodifica una grabacion
   (`src-tauri/src/managers/transcription/policy.rs:51`).
2. La decision mira `model.long_form`, que viene de `Feature::LongForm` de
   transcribe.cpp (`src-tauri/src/engine_supervisor/worker.rs:316`).
3. Whisper **si** soporta audio largo en una pasada, asi que
   `live_decode` devuelve `LiveDecode::None`.

Con `None` no hay worker de decodificacion: el audio se entrega entero al
final y se decodifica de una pasada, **despues** de soltar la tecla.

Medido con el banco en vivo (voz real, ruta `batch`, 5 repeticiones):

| Audio | Espera tras soltar (min / p50) | Fragmentos durante la grabacion | Audio pendiente al soltar |
|---|---|---|---|
| 3,30 s | 966 ms / 1 773 ms | 0 | 3,30 s (todo) |
| 22,34 s | 1 299 ms / 1 447 ms | 0 | 22,34 s (todo) |
| 37,44 s | 3 202 ms / 18 179 ms | 0 | 37,44 s (todo) |

El banco toma el **minimo** por etapa de las repeticiones: es el estimador
menos ruidoso del tiempo real del motor. Los percentiles altos de esta
tabla (p95 de 20 s en la dictacion de 37 s) son ruido de la maquina, no del
modelo: durante la corrida estaban compilando en paralelo los builds de
esta misma rama. Por eso la comparacion entre configuraciones se hace
sobre el minimo.

La traza de la app sobre la dictacion real del usuario (22,34 s de voz,
modelo canary) lo dice igual, con las etapas de la traza ahora honestas:

```
[latency] dispatch=50ms audio=10ms model_load=0ms asr=0ms finalize=2726ms
          rules=0ms post=0ms history=13ms insert=178ms total=2980ms
          text=2975ms wall=2989ms
```

`asr=0` y `finalize=2726` no son un error de medicion: son el dato. La
decodificacion ocurre entera en la etapa `finalize`, es decir, **despues**
de que el usuario solto la tecla.

> Antes esta traza imprimia `0ms` para toda etapa que no se habia medido.
> Una etapa que no se midio no es una etapa que no costo nada, y el `0`
> hacia imposible distinguirlas. Ahora una etapa ausente se imprime como
> `-`.

---

## 2. La ley de latencia real

El coste de decodificar con Whisper Base **casi no depende de la duracion**
de la grabacion: hay un coste fijo de unos 0,9-1,2 s por pasada (el
codificador siempre procesa una ventana de 30 s) y encima una parte
proporcional pequena.

Curva medida sobre las siete grabaciones reales del usuario (ruta batch,
minimo de 7 repeticiones):

| Audio (s) | ASR (ms) | ms por segundo de audio |
|---|---|---|
| 3,30 | 1 616 | 490 |
| 6,02 | 995 | 165 |
| 14,40 | 1 290 | 90 |
| 17,82 | 1 234 | 69 |
| 22,34 | 2 102 | 94 |
| 28,03 | 1 650 | 59 |
| 37,44 | 2 713 | 73 |

De ahi sale el suelo de la latencia con Whisper: **una pasada, mas o menos
un segundo**. Para una dictacion de 22 s, la app espera 1,4 s; el suelo
teorico del motor es ~1,0 s. **Con Whisper, la app ya esta cerca de su
suelo**, y la un forma de bajar de ahi es no decodificar el audio entero
en una sola pasada.

---

## 3. El A/B: cortar en fragmentos durante la grabacion

La idea: cortar el audio en pausas mientras se graba, decodificar cada
fragmento por su cuenta, y dejar para despues de soltar la tecla solo el
ultimo fragmento. Eso exige (a) permitir el corte en los modelos que
soportan audio largo, y (b) cortar antes.

Las dos cosas se pueden medir sin recompilar:

| Variable de entorno | Que hace |
|---|---|
| `DIXI_CHUNK_LONG_FORM=1` | Permite cortar tambien a los modelos que manejan audio largo en una pasada (Whisper) |
| `DIXI_CHUNK_MAX_SECONDS` | Longitud maxima del fragmento |
| `DIXI_CHUNK_MIN_SECONDS` | Longitud minima del fragmento |

Con Whisper, la ruta live **si** se activa y los fragmentos se decodifican
mientras el usuario habla. El detalle por fragmento (configuracion de 4 s,
dictacion de 22,34 s, mejor corrida):

| Fragmento | Audio | Decodifico | Termino |
|---|---|---|---|
| 1 | 4,0 s | 956 ms | 15,4 s **antes** de soltar |
| 2 | 4,0 s | 911 ms | 11,5 s antes |
| 3 | 4,0 s | 1 021 ms | 7,4 s antes |
| 4 | 4,0 s | 961 ms | 3,4 s antes |
| 5 | 4,0 s | 960 ms | 961 ms **despues** de soltar |
| 6 | 2,3 s | 1 112 ms | 2 074 ms despues |

Resultado (minimo de 3 repeticiones, voz real, las tres configuraciones
con el mismo numero de repeticiones y en la misma sesion):

| Audio | A: sin cambios | B: fragmentos de 8 s | C: fragmentos de 4 s |
|---|---|---|---|
| 3,30 s | **799 ms** | 960 ms | 1 013 ms |
| 22,34 s | **1 405 ms** | 2 251 ms | 1 947 ms |
| 37,44 s | 3 819 ms | **1 729 ms** | **1 890 ms** |

Una segunda corrida del baseline con 5 repeticiones dio 966 ms, 1 299 ms y
3 202 ms: el orden de magnitud se mantiene, y la diferencia con la tabla
anterior es ruido de la maquina (compilaciones en paralelo).

### Lo que dice la medicion

- **Para dictaciones cortas y medias, cortar es PEOR.** Cada fragmento
  cuesta ~1 s (el coste fijo del codificador), y al soltar quedan uno o dos
  por decodificar: dos pasadas fijas son mas caras que una sola.
- **Para dictaciones largas, cortar es MUCHO mejor**: 3 819 ms -> 1 729 ms
  (menos de la mitad), porque la mayor parte del audio ya estaba decodificado
  cuando el usuario solto la tecla.
- El cuello de botella no es el audio pendiente, es el **numero de
  fragmentos**: la decodificacion va ~1 s por detrás de la voz, asi que al
  soltar siempre hay un fragmento en vuelo.

**Conclusion: con Whisper Base, cortar en fragmentos solo compensa a partir
de dictaciones largas.** No es la via para la dictacion corriente.

Por eso el cambio queda **apagado por defecto**: `DIXI_CHUNK_LONG_FORM=0`.
La politica parametrizada es la herramienta de medicion, no un cambio de
comportamiento. Si el usuario dicta mucho mas de 30 s, conviene
`DIXI_CHUNK_LONG_FORM=1` con `DIXI_CHUNK_MAX_SECONDS=8`.

---

## 4. Nemotron 3.5 ASR Streaming: compatibilidad verificada

La pregunta era si el modelo candidato se puede usar con la version fijada
de transcribe.cpp. **Se verifico contra el codigo, no se supono.**

### El modelo

- `nvidia/nemotron-3.5-asr-streaming-0.6b`, conversion GGUF de
  `handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf` (publicada
  expressly para transcribe.cpp).
- Arquitectura: **FastConformer cache-aware + decodificador RNN-T**, con
  condicionamiento por idioma.
- GGUF: `architecture = parakeet`. **Ya esta en el catalogo de Dixi**:
  `handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf`,
  `supports_streaming: true`, 716 MB, `es` entre los 28 idiomas, revision
  fijada `6d44e540bc31b0de1dbe174a3cea87f53a7f22fb`.

### Por que funciona con transcribe.cpp 0.4.0

El motor fijado es `transcribe-cpp 0.4.0`. En sus fuentes:

- El arch `parakeet` implementa el **Prompt MLP** que exige el
  condicionamiento por idioma, con el mismo calculo que NeMo
  (`EncDecRNNTBPEModelWithPrompt`): concat del vector de idioma con la
  salida del codificador, y dos matricias con ReLU en el medio
  (`transcribe-cpp-sys-0.4.0/src/arch/parakeet/encoder.cpp:643`).
- El **codificador cache-aware** de streaming exige
  `att_context_style=ChunkedLimited`, que es justo la convencion del modelo
  (`encoder.cpp:757`).
- Las extensiones de streaming del binding cubren el menu de contexto
  derecho (`ParakeetStream` con `att_context_right`), que es la opcion que
  elige el `att_context_size` del Nemotron: `[56,3] [56,0] [56,6] [56,13]`.

O sea: **no hace falta ni cambiar el motor ni compilar nada nuevo**.

### Licencia

**OpenMDW-1.1** (`openmdw.ai/license/1-1/`). Uso, modificacion y
comparticion sin restriccion; **ninguna obligacion sobre las salidas**. La
unica obligacion es conservar la licencia y los avisos si se redistribuye
el material del modelo. Para el uso personal de Dixi no impose nada.

### Lo que falta

**La descarga (716 MB) no esta hecha: falta autorizacion expresa.** El
modelo no se ha descargado, no se ha medido y no se ha cambiado el modelo
activo de Dixi.

Lo que se puede decir sin medir, y lo que no:

| | Sin medir | Con medir |
|---|---|---|
| Latencia | El codificador cache-aware no recalcula lo ya decodificado, asi que el coste posterior a soltar deberia ser solo la cola. **Es una expectativa, no un resultado.** | El banco en vivo, en igualdad de condiciones |
| Exactitud | El WER publicado por el propio modelo en FLEURS es 6,3 para espanol (Q8_0). Whisper Base no tiene equivalente comparable aqui. | El guion de `docs/validation-voice/frases-voz.txt` con voz real |
| Memoria | 716 MB en disco. **Medido con Whisper Base**: el arbol de procesos completo (4 procesos: la app, el worker del motor y sus hijos) ocupa **216 MB de mediana y 248 MB de pico**. Nemotron todavia no se ha medido | La misma medicion con el modelo cargado |

**No se recomienda sustituir Whisper Base por Nemotron hasta que el banco
diga que la experiencia real mejora.** La expectativa es buena y la
compatibilidad esta probada, pero una expectativa no es una medicion.

---

## 5. El corpus sintetico no vale para medir exactitud

El corpus de `docs/validation-phase2a/` es audio de espeak-ng. Con clips
cortos, Whisper Base **alucina**, y el texto no tiene nada que ver:

| Clip (s) | Texto (idioma forzado a `es`) |
|---|---|
| 3,00 | `オーラー jetzt初だエビの` |
| 4,08 | `Older esta sueva marche que en español` |
| 9,87 | `Ahora, esta es una screwán emigrano en español.ijades tipo de revise...` |
| 20,65 | `Hola, esta es una nueva en dictano en español. Fijate si pones revisar...` |

Forzar el idioma **no** lo arregla (la fila de 3 s sale en japones con `es`
forzado). El WER de 65,9 % que midi la sesion anterior con este corpus
**mide el corpus, no el modelo**.

**Sobre la misma grabacion real de 22,34 s, Whisper Base transcribe bien:**

> Los íconos de Super Stock hay que mejorarlos y actualizarlos. El ícono se
> guarda en la carpeta de Statics de los íconos y también se guarda en
> Cloudflat R2 y el método principal de buscada es el de Cloudflat R2 y si no
> lo encuentra ahí lo busca en la carpeta de Statics.

Por eso la exactitud se mide con voz real: `docs/validation-voice/frases-voz.txt`
(diez frases rioplatenses con terminologia tecnica en ingles) y
`scripts/collect-voice-validation.py`, que empareja la traza de latencia
con el historial y con el WAV y calcula WER y CER.

---

## 6. Defectos encontrados y corregidos

| Defecto | Estado | Donde |
|---|---|---|
| El banco media **el modelo equivocado**, en silencio, y lo anotaba en la columna equivocada | **Corregido** | `scripts/measure-latency.ps1` (el bucle de enlaces duros usaba `$model` y pisaba el parametro `-Model`) |
| Las etapas no medidas se imprimian como `0 ms`, igual que las que costaron nada | **Corregido** | `src-tauri/src/latency.rs` (ahora `-`) |
| Los WAV de las dictaciones canceladas se quedaban en disco para siempre | **Corregido** | `src-tauri/src/actions.rs` (cuatro caminos de cancelacion ahora borran el WAV) |
| El texto transcrito no llegaba al historial si el WAV no se escribia | **Corregido** | `src-tauri/src/actions.rs` (la entrada se guarda siempre; el WAV es un extra) |
| Dos dictaciones en el mismo milisegundo escribian el mismo WAV | **Corregido** | `src-tauri/src/actions.rs` (contador + comprobacion de unicidad, como el repo ya hizo para los prompts) |
| Pegar vaciaba el portapapeles cuando su contenido no se podia restaurar (una lista de archivos) | **Corregido** | `src-tauri/src/clipboard.rs` (ya no se vacia; el historial de Windows puede recuperar los archivos) |

Defectos revisados **y descartados** con evidencia:

- **Varios procesos de Dixi a la vez**: ya resuelto. El plugin de instancia
  unica mata el segundo proceso antes de que cree bandeja o cargue modelo
  (`src-tauri/src/lib.rs:1018`). En modo headless es a proposito: cada
  `--transcribe-file` es un proceso independiente.
- **Notepad y Notepad++**: el pegado del dictado no tiene lista de ventanas
  (es portapapeles + Ctrl+V), asi que funciona en ambos. La lista de clases
  solo aplica a la ruta de *seleccion* (`managers/selection.rs:367`), que
  rechaza `Scintilla` (Notepad++) y `Windows.UI.Core.CoreWindow` (Notepad de
  Windows 11). Es una limitacion de la seleccion, no del pegado.

Defectos **sin corregir a proposito**:

- **Cierre con trabajo en vuelo**: no hay manejador de `ExitRequested`, asi
  que una transcripcion en curso se pierde al salir, y el item "Salir" sigue
  habilitado con trabajo en curso (`tray.rs:525`). Es un cambio de ciclo de
  vida de la app, no un arreglo: queda para otra etapa.
- **mRemoteNG**: la integracion con el escritorio remoto sigue
  **desactivada para el pegado automatico** (`remote_desktop_enabled=false`,
  `remote_desktop_auto_mode=false`), y el modo manual sigue activo. Las
  pruebas de control manual estan en `docs/validation-mremote.md`.

---

## 7. Inventario de los temporales y limpieza preparada

Las siete carpetas del banco de la sesion anterior ocupan **440,3 MB** reales:

| Carpeta | MB reales | Archivos | Junction |
|---|---|---|---|
| `dixi-latency-antes-canary-180m-flash-125837` | 63,1 | 33 | 1 |
| `dixi-latency-antes-canary-180m-flash-131443` | 63,1 | 33 | 1 |
| `dixi-latency-antes-whisper-base-123629` | 61,4 | 23 | 1 |
| `dixi-latency-antes-whisper-base-123847` | 63,1 | 33 | 1 |
| `dixi-latency-antes-whisper-base-125632` | 63,2 | 33 | 1 |
| `dixi-latency-antes-whisper-base-131313` | 63,2 | 33 | 1 |
| `dixi-latency-antes-whisper-small-125909` | 63,2 | 33 | 1 |

**Los siete junctions apuntan a la carpeta real de modelos del usuario**
(`%APPDATA%\com.dixi.app\models`). Ese es el peligro: `Remove-Item -Recurse`
de PowerShell 5.1 sigue los junctions y **vaciaria la carpeta de modelos**.

`scripts/cleanup-bench-instances.ps1` esta preparado y **no se ejecuto**:

- sin `-Execute` solo informa (lo que se ve arriba es su salida);
- corta el enlace ANTES de borrar la carpeta, con la API de .NET, que no lo
  sigue;
- comprueba que ningun destino de junction caiga dentro de la carpeta que
  se va a borrar.

Ningun proceso usa esas carpetas.

---

## 8. Como reproducir

```powershell
# Banco de la ruta en vivo, sobre las grabaciones reales
.\scripts\measure-live-latency.ps1 `
  -Exe src-tauri\target\release\dixi.exe `
  -Model handy-computer/whisper-base-gguf/whisper-base-Q8_0.gguf `
  -FixturesDir <carpeta con WAV> -Repeat 5 -Label medicion -Language es -NoText

# Barrido del corte en fragmentos
$env:DIXI_CHUNK_LONG_FORM = '1'
$env:DIXI_CHUNK_MAX_SECONDS = '8'
$env:DIXI_CHUNK_MIN_SECONDS = '3'

# Validacion con voz real
python scripts\collect-voice-validation.py --out <carpeta>
```

Todo corre sin ventana: `scripts/run-hidden.ps1` y `CreateNoWindow`.

---

## 9. Lo que queda pendiente

1. **La descarga de Nemotron (716 MB)**: pendiente de autorizacion. Es lo
   que bloquea la comparacion en igualdad de condiciones.
2. **La sesion guiada de dictacion** (`docs/validation-voice/frases-voz.txt`):
   sin ella no hay WER ni CER de voz real.
3. **La limpieza de los 440 MB**: pendiente de autorizacion.
4. **El cierre con trabajo en vuelo** y el **portapapeles con formatos
   propios**: defectos reales, con correccion de mayor alcance que esta
   etapa.
