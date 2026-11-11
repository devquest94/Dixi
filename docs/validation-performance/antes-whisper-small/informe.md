# Latencia del motor ASR - antes-whisper-small

Banco reproducible sobre el corpus de referencia
(`docs/validation-phase2a/audio-referencia`), con `--repeat 5` por
audio y quedandose con la mejor pasada.

- Binario: `D:\Codigo\Dixi\src-tauri\target\debug\dixi.exe`
- Modelo: `handy-computer/whisper-small-gguf/whisper-small-Q8_0.gguf`
- Instancia aislada: `C:\Users\DANIEL~1\AppData\Local\Temp\dixi-latency-antes-whisper-small-125909` (modelos por junction)
- CPU / RAM de referencia: 4 nucleos, 16 GB

| Metrica | Media | p50 | p95 | Maximo |
|---|---|---|---|---|
| Carga del modelo (ms) | 619.6 | 479 | 1303 | 1303 |
| Inferencia ASR (ms) | 8035.3 | 3984 | 25594 | 25594 |
| Mejor pasada (ms) | 8051.5 | 3998 | 25610 | 25610 |
| Reglas de texto (ms) | 0 | 0 | 0 | 0 |
| Total trazado (ms) | 8655.4 | 4463 | 26694 | 26694 |

Factor de tiempo real global (audio total / inferencia total): **0.672 x**

## Detalle por audio

| Audio | Audio (s) | Carga (ms) | ASR (ms) | Reglas (ms) | Total (ms) | RTF |
|---|---|---|---|---|---|---|
| F1.wav | 4.0829375 | 1303 | 11153 | 0 | 12457 | 0.365265476829486 |
| F10.wav | 5.402125 | 1099 | 25594 | 0 | 26694 | 0.210938110113237 |
| F2.wav | 5.3865625 | 479 | 3984 | 0 | 4463 | 1.34731428214107 |
| F3.wav | 5.30375 | 597 | 5119 | 0 | 5716 | 1.03246058010512 |
| F4.wav | 4.680125 | 445 | 5664 | 0 | 6110 | 0.824110758936432 |
| F5.wav | 8.2625 | 438 | 14372 | 0 | 14810 | 0.574343111358265 |
| F6.wav | 5.0916875 | 521 | 3718 | 0 | 4239 | 1.36433212754555 |
| F7.wav | 5.0218125 | 423 | 3703 | 0 | 4126 | 1.35067576654115 |
| F8.wav | 5.3588125 | 409 | 3571 | 0 | 3981 | 1.49353748606466 |
| F9.wav | 5.37275 | 482 | 3475 | 0 | 3958 | 1.53991114932645 |
