# Latencia del motor ASR - antes-whisper-base

Banco reproducible sobre el corpus de referencia
(`docs/validation-phase2a/audio-referencia`), con `--repeat 5` por
audio y quedandose con la mejor pasada.

- Binario: `D:\Codigo\Dixi\src-tauri\target\debug\dixi.exe`
- Modelo: `handy-computer/whisper-base-gguf/whisper-base-Q8_0.gguf`
- Instancia aislada: `C:\Users\DANIEL~1\AppData\Local\Temp\dixi-latency-antes-whisper-base-131313` (modelos por junction)
- CPU / RAM de referencia: 4 nucleos, 16 GB

| Metrica | Media | p50 | p95 | Maximo |
|---|---|---|---|---|
| Carga del modelo (ms) | 201 | 190 | 266 | 266 |
| Inferencia ASR (ms) | 1234.1 | 1085 | 1782 | 1782 |
| Mejor pasada (ms) | 1248.4 | 1101 | 1796 | 1796 |
| Reglas de texto (ms) | 0 | 0 | 0 | 0 |
| Total trazado (ms) | 1435.5 | 1285 | 1982 | 1982 |

Factor de tiempo real global (audio total / inferencia total): **4.373 x**

## Detalle por audio

| Audio | Audio (s) | Carga (ms) | ASR (ms) | Reglas (ms) | Total (ms) | RTF |
|---|---|---|---|---|---|---|
| F1.wav | 4.0829375 | 266 | 1395 | 0 | 1662 | 2.89775550035486 |
| F10.wav | 5.402125 | 195 | 1255 | 0 | 1451 | 4.25699369582348 |
| F2.wav | 5.3865625 | 200 | 1782 | 0 | 1982 | 2.99919961024499 |
| F3.wav | 5.30375 | 185 | 989 | 0 | 1174 | 5.28788634097707 |
| F4.wav | 4.680125 | 190 | 1697 | 0 | 1887 | 2.73531560490941 |
| F5.wav | 8.2625 | 187 | 1148 | 0 | 1336 | 7.1105851979346 |
| F6.wav | 5.0916875 | 190 | 958 | 0 | 1149 | 5.2383616255144 |
| F7.wav | 5.0218125 | 189 | 1066 | 0 | 1255 | 4.64552497687327 |
| F8.wav | 5.3588125 | 200 | 1085 | 0 | 1285 | 4.8672229791099 |
| F9.wav | 5.37275 | 208 | 966 | 0 | 1174 | 5.48239795918367 |
