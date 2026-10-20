# Validación de la Fase 2A — procedimiento

Este documento explica cómo ejecutar Dixi, cómo activar el dictado, cómo
configurar el micrófono y qué comprobar. Todo lo que hay que hacer está
**automatizado** salvo el paso de hablar: el resto lo miden scripts.

---

## 0. Requisitos (ya cumplidos en esta máquina)

| Requisito | Estado |
|---|---|
| Binario release | `src-tauri/target/release/dixi.exe` |
| Modelo ASR | `whisper-base-Q8_0.gguf` en `%APPDATA%\com.dixi.app\models` (sha256 verificado) |
| Permiso de micrófono (Windows) | `Allow` en `CapabilityAccessManager\ConsentStore\microphone` y `\NonPackaged` |
| Micrófono | Redragon Gaming Headset (dispositivo 3, WASAPI, 48 kHz → 16 kHz) |
| Wispr Flow | **no se toca**: no se modifica su configuración ni se detienen sus procesos |

---

## 1. Ejecutar Dixi

```powershell
# La GUI se muestra normal; NO aparece ninguna consola.
& "D:\Codigo\Dixi\src-tauri\target\release\dixi.exe"
```

Notas:

- La app vive en la **bandeja del sistema** (icono propio de Dixi). Cerrar la
  ventana la oculta, no la termina. Para salir de verdad: icono de la bandeja
  → *Salir*.
- Si la ventana no aparece, el icono de la bandeja está en la zona de
  notificaciónOverflow.
- El log está en `%LOCALAPPDATA%\com.dixi.app\logs\dixi.log`.

### 1.1 Importante: cómo se compila

El binario de producción **debe** compilarse con la característica
`custom-protocol`:

```powershell
cd src-tauri
cargo build --release --features custom-protocol
```

Sin esa característica, el binario arranca pero **no puede cargar su propio
frontend**: la WebView2 intenta conectarse al servidor de desarrollo de Vite
(`http://localhost:1420`), la interfaz queda en blanco, el frontend nunca
ejecuta sus efectos y **los atajos globales no se registran** — la app queda
inutilizable. Es un defecto de la Fase 2A que ya está corregido; ver el
informe, §Defectos.

Equivalente, recomendado porque además genera el instalador NSIS:

```powershell
npm.cmd run tauri build
```

---

## 2. Activar el dictado

### 2.1 El atajo

| Atajo | Acción | Valor por defecto |
|---|---|---|
| **Ctrl + Espacio** | Grabar y transcribir | `ctrl+space` |
| **Escape** | Cancelar la grabación en curso | `escape` |
| Ctrl + Shift + Espacio | Dictar con post-procesado IA (desactivado) | `ctrl+shift+space` |

**Comportamiento**: en *Ajustes → General → Comportamiento del atajo* hay tres
modos. El recomendado para probar es **Mantener** (*push-to-talk*): se graba
mientras se mantiene pulsado y se transcribe al soltar. Está ya configurado
así por `scripts/prepare-validation.ps1`.

Si eliges *Alternar* (toggle), una pulsación empieza y otra para.

### 2.2 Cambiar el atajo

*Ajustes → General → Atajos de Dixi → Atajo de transcripción* → pulsar
*Presiona teclas…* → pulsar la combinación → *Guardar*.

### 2.3 Nota sobre el conflicto con Windows

`Ctrl + Espacio` es, en muchos sistemas, el **interruptor de método de
entrada** (IME). Si al mantener pulsado el atajo se abre el selector de
idiomas del sistema en vez de grabar, hay que cambiar el atajo (por ejemplo a
`Ctrl + Alt + Espacio`) o desactivar el uso de `Ctrl+Espacio` en
*Configuración → Hora e idioma → Writing/IME*.

Dixi registra el atajo con un hook de bajo nivel, así que el conflicto se
nota antes de pulsar: el log muestra
`Registered handy-keys shortcut: transcribe -> Hotkey {...}`.

---

## 3. Configurar el micrófono

### 3.1 Estado actual

Ya está configurado y **funciona**: el log muestra

```
opened Micrófono (3- Redragon Gaming Headset) (WASAPI) in 63 ms at 48000 Hz, 1 ch;
delivering 16000 Hz, 1 ch, 512-frame chunks
```

El dispositivo por defecto del sistema es el Redragon. `selected_microphone`
está vacío, lo que significa **"usar el micrófono por defecto del sistema"**.

### 3.2 Fijar un micrófono concreto

*Ajustes → Audio (o "Dispositivos")* → elegir en *Micrófono*. Se puede
comprobar el nivel en el propio panel.

### 3.3 Permiso de Windows

Ya está en `Allow`. Si algún día cambia, se comprueba en
*Configuración → Privacidad → Micrófono*.

---

## 4. Las pruebas de dictado

### 4.1 Preparar el estado (una vez)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\prepare-validation.ps1 -Launch
```

Deja la app en un estado determinista: modelo `whisper-base` cargado,
idioma **forzado a `es`**, pegado fiable con restauración del portapapeles,
modelo que no se descarga entre pruebas y atajo en modo *Mantener*.

> **Por qué forzar `es`:** la detección automática de idioma es el punto
> débil demostrado en la Fase 2A: con `canary-180m-flash` una frase en
> español se transcribió **en inglés**. Con `es` forzado no hay margen de
> error.

### 4.2 Las frases

Están en [`frases.txt`](./frases.txt) (10 frases, español rioplatense con
voseo, lunfardo y términos técnicos en inglés). Ejemplos:

| # | Frase | Qué prueba |
|---|---|---|
| F1 | *Hola, esta es una prueba de dictado en español.* | línea base |
| F2 | *Fijate si podés revisar los logs del server antes de hacer el deploy.* | voseo + inglés |
| F3 | *El build me tiró un error, está medio quilombo, lo voy a chequear.* | lunfardo |
| F4 | *Hacé un commit, pusheá la branch y corré el deploy en Docker.* | inglés técnico denso |
| F5 | *…el backup del servidor de producción quedó bien, porque ayer el log showed un timeout.* | code-switching |
| F7 | *El servidor tenía cuarenta y cinco por ciento de la memoria usada.* | números |
| F9 | *Che, ¿me pasás el archivo del backup? Lo necesito para el merge.* | voseo + pregunta |

Hay audio **sintético** de referencia en `audio-referencia/` (generado con
`espeak-ng` en WSL, voz `es-419`). Sirve para comparar el mismo audio entre
modelos; **no** como expectativa de calidad con voz humana (la voz sintética
deforma muchas palabras: ver el informe, §Defectos).

### 4.3 Ejecutar el arnés de medición

En una terminal aparte, **antes** de dictar:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\validate-dictation.ps1 -TimeoutSeconds 120
```

El arnés:

- mide la RAM y la CPU del **árbol completo** de procesos (app + WebView2 +
  worker de ASR);
- marca el instante exacto en que aparece cada línea del log (el log solo
  tiene resolución de 1 segundo, así que se taila el fichero);
- detecta la soltura de la tecla de dictado;
- deja un **centinela** en el portapapeles y comprueba después si se restauró;
- registra la ventana en primer plano antes y después (foco);
- recoge los errores y avisos del log.

Mientras corre:

1. Abrir **Notepad** y dejar el cursor dentro.
2. Mantener `Ctrl+Espacio`, **hablar la frase**, soltar.
3. Comprobar que el texto aparece en Notepad.
4. En la terminal del arnés, pulsar **Enter** para cerrar la medición.

---

## 5. Qué se comprueba y con qué evidencia

| # | Comprobación | Evidencia |
|---|---|---|
| 1 | El atajo global dispara la grabación | log: `handy-keys event: binding=transcribe, state=Pressed` |
| 2 | El micrófono captura | log: `opened Micrófono (...) in N ms` |
| 3 | El VAD detecta voz | `Initialized Silero VAD backend` |
| 4 | Se indica que está grabando | log: `tray apply: icon=resources/tray_recording.png` + overlay `show=` |
| 5 | Se cancela con Escape | log: `Registered handy-keys shortcut: cancel -> ... Escape` |
| 6 | El ASR local transcribe | log: `Transcription completed in Xs for Ys of audio` |
| 7 | El texto llega a Notepad | el usuario lo ve; la app registra `[reliable-paste] paste chord sent` |
| 8 | El portapapeles se restaura | el arnés compara el centinela |
| 9 | El foco no se roba | el arnés compara la ventana en primer plano |
| 10 | Latencia voz → texto | el arnés: `captura_fin -> ctrl+V` y `captura_fin -> restaurado` |
| 11 | RAM y CPU del árbol completo | el arnés y `scripts\measure-dixi-tree.ps1` |
| 12 | Errores y cancelaciones | log: líneas `[ERROR]` / `[WARN]` |

---

## 6. Qué NO se puede automatizar

La **inserción real en Notepad** requiere el escritorio: ventana enfocada,
portapapeles compartido y la aplicación destino leyendo el texto. Se
comprueba a ojo. Todo lo demás (captura, ASR, pegado, portapapeles, foco,
cancelación) queda registrado con evidencia automática.

**El criterio de aceptación no se da por bueno hasta que el_dictado se
compruebe con la voz y el texto aparezca en Notepad.**
