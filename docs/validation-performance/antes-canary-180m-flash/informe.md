# Latencia del motor ASR - antes-canary-180m-flash

Banco reproducible sobre el corpus de referencia
(`docs/validation-phase2a/audio-referencia`), con `--repeat 5` por
audio y quedandose con la mejor pasada.

- Binario: `D:\Codigo\Dixi\src-tauri\target\debug\dixi.exe`
- Modelo: `handy-computer/canary-180m-flash-gguf/canary-180m-flash-Q8_0.gguf`
- Instancia aislada: `C:\Users\DANIEL~1\AppData\Local\Temp\dixi-latency-antes-canary-180m-flash-131443` (modelos por junction)
- CPU / RAM de referencia: 4 nucleos, 16 GB

| Metrica | Media | p50 | p95 | Maximo |
|---|---|---|---|---|
| Carga del modelo (ms) | 348.9 | 342 | 402 | 402 |
| Inferencia ASR (ms) | 306.4 | 262 | 460 | 460 |
| Mejor pasada (ms) | 325.1 | 282 | 479 | 479 |
| Reglas de texto (ms) | 0 | 0 | 0 | 0 |
| Total trazado (ms) | 655.6 | 627 | 862 | 862 |

Factor de tiempo real global (audio total / inferencia total): **17.612 x**

## Detalle por audio

| Audio | Audio (s) | Carga (ms) | ASR (ms) | Reglas (ms) | Total (ms) | RTF |
|---|---|---|---|---|---|---|
| F1.wav | 4.0829375 | 342 | 293 | 0 | 635 | 13.0445287539936 |
| F10.wav | 5.402125 | 402 | 460 | 0 | 862 | 11.2779227557411 |
| F2.wav | 5.3865625 | 334 | 261 | 0 | 595 | 19.2377232142857 |
| F3.wav | 5.30375 | 322 | 262 | 0 | 585 | 18.8076241134752 |
| F4.wav | 4.680125 | 350 | 225 | 0 | 575 | 19.2597736625514 |
| F5.wav | 8.2625 | 329 | 408 | 0 | 737 | 19.3501170960187 |
| F6.wav | 5.0916875 | 336 | 247 | 0 | 584 | 19.2866950757576 |
| F7.wav | 5.0218125 | 382 | 244 | 0 | 627 | 19.1672232824427 |
| F8.wav | 5.3588125 | 346 | 295 | 0 | 641 | 17.0121031746032 |
| F9.wav | 5.37275 | 346 | 369 | 0 | 715 | 13.9190414507772 |
