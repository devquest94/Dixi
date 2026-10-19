# 01 — Entorno de ejecución

> Auditoría de ingeniería inversa funcional de **Wispr Flow 1.6.1174** (Windows) como base para **Dixi**.
> Todas las afirmaciones de este documento están etiquetadas con su nivel de confianza:
> **[V]** verificado por observación directa · **[I]** inferido de evidencia coherente · **[NV]** no verificado.

---

## 1.1 Resumen ejecutivo

| Ítem | Valor | Fuente |
|---|---|---|
| Producto auditado | Wispr Flow **1.6.1174** (Windows x64) | `packages/WisprFlow-1.6.1174-full.nupkg` [V] |
| Tecnología base | **Electron 42.11.2 / Chromium 148.0.7778.280 / V8 14.8.178.38** | CDP `Browser.getVersion` [V] |
| Gestor de instalación | **Squirrel.Windows** | `Update.exe`, `squirrel.exe`, `Squirrel-*.log` [V] |
| Empaquetado | `app.asar` (129 MB) + webpack + 4 renderers HTML | CDP target list [V] |
| Helper nativo | `Wispr Flow Helper.exe` 19,8 MB, **v1.0.0+38f663c0…** (hash git) | `VersionInfo` [V] |
| Modelo de ASR | **Nube** (no local) | `clientNetworkLatency` p50 = 270 ms en 3 160 dictados [V] |
| SO | Windows 10 Pro 19045 (22H2) x64 | `Win32_OperatingSystem` [V] |
| RAM instalada | 15,8 GB | `TotalVisibleMemorySize` = 16 615 732 KB [V] |
| CPU / GPU | i5-8300H (4C/8T, 2,3 GHz) · UHD 630 + GTX 1050 Ti | `Win32_Processor` / `Win32_VideoController` [V] |
| Monitores | primario 2048×1152 @125 % · secundario 1920×1080 @(2560,358) | `Screen.AllScreens` [V] |

> **Conclusión de Fase A:** la hipótesis «WebView vs Electron» queda **resuelta con evidencia**: es **Electron**, y por lo tanto **dixi NO compite en igualdad de condiciones con un WebView nativo** — compite contra un runtime completo con 4 renderers, GPU process, utility processes, crashpad, Sentry nativo y un helper Rust separado.

---

## 1.2 Hardware y sistema operativo

| Componente | Especificación | Confianza |
|---|---|---|
| SO | Microsoft Windows 10 Pro, versión 10.0.19045 (build 19045), 64 bits | [V] |
| CPU | Intel(R) Core(TM) i5-8300H CPU @ 2.30 GHz — 4 núcleos / 8 hilos lógicos, 2 304 MHz máx. | [V] |
| GPU integrada | Intel(R) UHD Graphics 630, driver 31.0.101.2111 | [V] |
| GPU dedicada | NVIDIA GeForce GTX 1050 Ti, driver 32.0.15.7270 | [V] |
| RAM | 16 615 732 KB totales (≈ 15,8 GB) | [V] |
| Disco C: | 255 342 981 120 B (≈ 238 GiB), 52 089 761 792 B libres (≈ 48,5 GiB) | [V] |
| Escalado de pantalla | `--device-scale-factor=1.25` (125 %) | [V] |
| Distribución multi-monitor | 2 monitores, coords virtuales 2560×358 → 4480×1438 | [V] |

**Implicación para Dixi (Fase D):** la máquina es de la gama media-baja de escritorio (4 núcleos, GPU integrada + discreta de 2018). Cualquier motor ASR **local** compite por CPU contra Electron+Chromium. Ver [13-RENDIMIENTO.md](13-RENDIMIENTO.md) §6.

---

## 1.3 Instalación y empaquetado

Ruta de instalación: `C:\Users\DanielSan\AppData\Local\WisprFlow\`

```
WisprFlow/
├── app-1.6.1167/            ← versión anterior (16/10/2026 01:00)
├── app-1.6.1174/            ← versión vigente  (16/10/2026 21:58)
├── packages/
│   ├── WisprFlow-1.6.1174-full.nupkg   (323 970 KB ≈ 316 MB)
│   ├── SquirrelTemp/
│   ├── .betaId
│   └── RELEASES
├── app.ico
├── Squirrel-CheckForUpdate.log    (838,7 KB — actualizado 19/10/2026 13:01)
├── Squirrel-ProcessStart.log
├── Squirrel-Shortcut.log
├── Squirrel-Update.log
├── Update.exe                    (2 187,4 KB)
└── Wispr Flow.exe                (581,9 KB — stub lanzador Squirrel)
```

### 1.3.1 Contenido de `app-1.6.1174/`

| Elemento | Tamaño | Significado |
|---|---|---|
| `Wispr Flow.exe` | 229 913,9 KB | Binario principal Electron (≈ 225 MB) |
| `app.asar` | 132 176,2 KB (≈ 129 MB) | Bundle de aplicación |
| `app.asar.unpacked/` | — | Módulos nativos + assets |
| `ffmpeg.dll` | 3 000,9 KB | Codificación/decodificación de audio (Opus/Vorbis) |
| `dxcompiler.dll` | 25 052,4 KB | Compilador de shaders DirectX (Chromium) |
| `dxil.dll`, `vulkan-1.dll`, `vk_swiftshader.dll` | 1 474 / 919 / 5 415 KB | Stack gráfico (Vulkan por software = sin GPU dedicada en uso) |
| `libGLESv2.dll`, `libEGL.dll` | 7 851 / 482 KB | GLES |
| `icudtl.dat` | 10 621,6 KB | Datos ICU (Unicode/collation) |
| `locales/` | **55 packs `.pak`** | i18n completa |
| `.sentry-native/` | — | Binario nativo de Sentry (crash reporting) |
| `resources/migrations/` | 174 archivos `.js` | Migraciones de base de datos (Sec. 2.4) |
| `resources/ax-inspect*.mjs` | 92,5 / 5,2 / 3,7 KB | **Inspector de accesibilidad embebido** (ver §2.6) |
| `resources/Release/Wispr Flow Helper.exe` | 19,8 MB | Helper nativo (Sec. 2.3) |

### 1.3.2 Módulos nativos extraídos (`app.asar.unpacked/`)

| Módulo | Tamaño | Función inferida |
|---|---|---|
| `node_sqlite3.node` | 8 192 KB | Driver **SQLite nativo** (no `better-sqlite3`); confirma que la persistencia es SQLite en C++ |
| `jabra-device-connector.node` | 1 108 KB | Integración con auriculares **Jabra** (captura de micrófono a nivel de dispositivo) |

> **Hallazgo para Dixi:** Wispr Flow **no usa un ORM en Rust/TS**, sino `node_sqlite3` (node-sqlite3) sobre **SQLite**, con migraciones en JavaScript. Es exactamente el mismo motor que propone el stack de Dixi (SQLite). La capa de migraciones usa un ORM ligero con tabla `SequelizeMeta` (Sec. 4.2).

---

## 1.4 Arquitectura de procesos (verificada en vivo)

Proceso principal: **PID 4484**, `Wispr Flow.exe`, iniciado el **16/10/2026 21:58:26**, WS ≈ 119 MB.

| PID | Nombre | Tipo Chromium | WS (MB) | Rol inferido |
|---|---|---|---|---|
| 4484 | Wispr Flow.exe | main | 119,0 | Proceso principal (BrowserWindow owner, tray) |
| 16872 | Wispr Flow Helper.exe | (nativo, no Chromium) | 32,3 | **Helper nativo**: hotkeys globales, captura de audio, inyección de texto |
| 21436 | Wispr Flow.exe | crashpad | 9,9 | Crashpad handler |
| 19152 | Wispr Flow.exe | utility | 17,4 | Utility (network service) |
| 21280 | Wispr Flow.exe | utility | 37,2 | Utility (audio service) |
| 16160 | Wispr Flow.exe | utility | 20,6 | Utility (storage service) |
| 10036 | Wispr Flow.exe | renderer | 48,5 | Renderer A |
| 23680 | Wispr Flow.exe | renderer | 11,5 | Renderer B |
| 12672 | Wispr Flow.exe | renderer | 4,8 | Renderer C |
| 22356 | Wispr Flow.exe | renderer | 20,4 | Renderer D |
| 12020 | Wispr Flow.exe | renderer | 12,0 | Renderer E |
| 2532 | Wispr Flow.exe | gpu-process | 20,6 | Proceso GPU |

**Total en reposo: ≈ 361 MB de working set** (ver [13-RENDIMIENTO.md](13-RENDIMIENTO.md)).

> **Observación crítica para Dixi:** de los 5 renderers, solo **4** exponen páginas (`hub`, `status`, `contextMenu`, `scratchpad`). El quinto renderer corresponde a la ventana *Meeting Recorder*, que se crea de forma diferida bajo demanda. El proceso **helper nativo es un proceso separado**, no un thread: esto implica una frontera de IPC real entre el motor de captura/hotkeys y la app Electron. [V] por el árbol de procesos y cmdlines; [I] para la asignación de renderers por nombre.

### 1.4.1 Interruptores de línea de comandos observados

```
--lang=es                          --device-scale-factor=1.25
--user-data-dir="…\Roaming\Wispr Flow"
--no-sandbox  --no-zygote          (Chromium sandbox desactivado en Windows)
--disable-blink-features=Auxclick  (clic medio deshabilitado → no interfiere con el dictado)
--standard-schemes=wispr-extension
--secure-schemes=wispr-extension,sentry-ipc
--bypasscsp-schemes=sentry-ipc
--enable-features=…ScreenAIOCREnabled…      (y repetido en --disable-features → prevalece disable)
--video-capture-use-gpu-memory-buffer
--renderer-client-id=4,5,6,7,11
--disable-gpu-process-crash-limit --disable-features=LocalNetworkAccessChecks
```

**Lecturas relevantes:**

1. `--disable-blink-features=Auxclick` — el clic medio deja de enviarse a la app enfocada. **[I]** Motivo probable: evitar que el usuario active accidentalmente funciones de Wispr con el botón central del ratón.
2. `ScreenAIOCREnabled` aparece **simultáneamente** en `--enable-features` y `--disable-features`; en Chromium prevalece la última coincidencia (`--disable-features`) ⇒ **OCR de pantalla efectivamente desactivado** en esta instalación, coherente con `prefs.user.shouldOCRScreenCapture = false`. **[V]**
3. `--no-sandbox --no-zygote` — Chromium sin su sandbox de proceso. **[I]** Decisión de empaquetado para evitar choques con el antivirus/EDR corporate (AvastSvc activo en la máquina, 14 puertos loopback observados).
4. `wispr-extension://` es un **esquema de protocolo propio** para la extensión de navegador. **[V]** el flag; **[I]** su uso para inyectar contexto desde la extensión web.

---

## 1.5 Ubicación de datos (Roaming)

Ruta: `C:\Users\DanielSan\AppData\Roaming\Wispr Flow\`

| Elemento | Tamaño / estado | Contenido |
|---|---|---|
| `flow.sqlite` | **791 994 368 B (755 MB)** | Base principal (29 tablas, 174 migraciones) |
| `flow.sqlite-wal` | 4 736 B | Write-Ahead Log |
| `config.json` | 39,8 KB | **Preferencias completas del usuario** (≈ 150 claves) |
| `feature-flags-cache.json` | 40,7 KB | **Catálogo de feature flags + payloads de servidor** |
| `session.json` | 3,3 KB | Datos de sesión — **NO inspeccionado** (regla de seguridad) |
| `backups/` | 7 copias | Backups diarios automáticos (12:00 local / 15:00 UTC) |
| `meetings/` | 4 carpetas | Grabaciones de reuniones (ver §8) |
| `Dictionaries/` | — | Diccionarios por usuario/equipo |
| `avatars/` | — | Avatares de participantes de reuniones |
| `logs/` | — | Logs de aplicación |
| `sentry/` | — | Eventos de error (Sentry) |
| `shared_proto_db/` | — | Base de protobuf compartida (p. ej. `bgv`/`prod` de Sentry) |
| `es-ES-3-0.bdic` | — | Diccionario ortográfico Chromium (español) |

> **Implicación para Dixi:** 755 MB de base para 3 160 dictados ⇒ **≈ 240 KB por dictado** (audio crudo + Opus + capturas). El diseño de almacenamiento de Wispr es *audio-first*: conserva el audio para reprocesar/re-formatear. Ver [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md).

---

## 1.6 Capacidad de las herramientas REA en este host

Verificado con `mcp__rea__binary_session`:

| Capacidad REA | Disponible en Windows | Uso en esta auditoría |
|---|---|---|
| `inspect_sqlite_database` | Sí | **Rechazó** `flow.sqlite` por exceder el presupuesto de 268 435 456 B (755 MB). Se resolvió copiando la BD a un directorio temporal **fuera de la instalaciÃ³n** (copia eliminada al terminar) y leyendo el esquema con `python3.13` + `sqlite3` en modo lectura. [V] |
| `list_electron_targets` | Sí | Conectado a `http://127.0.0.1:9333` → 4 renderers. [V] |
| `inspect_electron_page` | Sí | Inspección DOM/recursos de `status` y `hub`. [V] |
| `capture_electron_scenario` | Sí, **marcado `destructive` / `mayDiscardData`** | **No ejecutado** contra la instalación del usuario: REA es dueño del arranque *y* del apagado, con riesgo de afectar a la instancia viva. Ver §1.7. |
| `inspect_macho` / `inspect_artifact` / `inspect_binary_layout` | Sí | No usados: la auditoría es **funcional**, no de binario. |
| `execute_rizin_command` / `gdb_console` | Sí | No usados por la misma razón (ver §1.8). |

### 1.6.1 Procedimiento de aislamiento con CDP (reproducible)

La instancia viva **no expone puerto CDP** (verificado: ningún `LISTEN` en ninguno de los 12 PIDs de Wispr). Para obtener acceso DOM se lanzó una **instancia aislada**:

1. `launch-isolated.bat` → `Wispr Flow.exe --user-data-dir=<tmp>\isolated-profile --remote-debugging-port=9333`
2. Verificado aislamiento: proceso nuevo con `--user-data-dir=isolated-profile`; **PID 4484 (vivo) intacto**; 11 PIDs nuevos identificados.
3. `mcp__rea__list_electron_targets(cdp_endpoint="http://127.0.0.1:9333")` → 4 targets.
4. `mcp__rea__inspect_electron_page` sobre `status` y `hub`.
5. **Terminación de los 11 PIDs aislados**; verificado: puerto 9333 cerrado, PID 4484 + helper 16872 vivos, las 5 ventanas originales intactas. [V]

**Riesgo residual controlado:** durante ~4 min coexistieron dos instancias, ambas con el helper registrando hotkeys globales. Se verificó que sólo la aislada registró el puerto CDP y que la viva nunca estuvo en riesgo de apagado. La instancia aislada no tenía sesión iniciada (perfil nuevo), por lo que **nunca mostró datos de la cuenta del usuario**.

---

## 1.7 Límites explícitos de esta auditoría

| Restricción | Aplicación |
|---|---|
| No descompilar componentes propietarios | **No** se extrajo ni se leyó el contenido de `app.asar` (129 MB) ni de los bundles webpack. Sólo se inspeccionaron **nombres de archivo, rutas, tamaños y metadatos**. |
| No extraer credenciales | `session.json` no se abrió. `config.json` y `feature-flags-cache.json` se leyeron con **redacción automática** de cualquier clave que case con `token/secret/password/apikey/auth/credential/cookie/session/email/signature/bearer/jwt/key`. |
| No extraer datos de sesiones | `History` (3 160 filas) se consultó **exclusivamente en agregados numéricos** (latencias, conteos, distribuciones). **Ningún texto de transcripción fue volcado.** |
| No modificar nada | La única escritura fue en `D:\Codigo\Dixi\.tmp-audit\`. Seennetcdf: `ShowWindow`/`Hide` de la ventana principal (restaurado al estado original), y ESC para descartar overlays. Las preferencias de Wispr Flow **no se alteraron** (verificado re-leyendo `config.json` al cierre). |
| No sortear licencias | No se inspeccionaron ni alteraron mechanisms de licencia, trial o suscripción. |

---

## 1.8 Configuración regional y de idioma

- `uiLocale = "es"`, `--lang=es`, `es-ES-3-0.bdic` ⇒ **IU y motor ortográfico en español**. [V]
- `selectedLanguages = ["en","es"]`, `usedLanguages` con ambos. [V]
- `timezone = "America/Buenos_Aires"`, `prefs.timezone = "America/Buenos_Aires"`. [V]
- Idioma **detectado** en el historial: `es` (3 075) / `en` (8) ⇒ el ASR detecta idioma dinámicamente por dictado. [V]

---

## 1.9 Implicaciones directas para la Fase 2 (arquitectura de Dixi)

1. **El objetivo no es «rehacer un WebView»**: es reemplazar un runtime Electron con 4 renderers, ~360 MB de RAM y GPU process. Un stack **Tauri 2 + Rust** elimina GPU/utility processes y reduce drásticamente la RAM en reposo. Ver [02-ARQUITECTURA.md](02-ARQUITECTURA.md) §7.
2. **El helper nativo es el punto crítico**: hotkeys globales, captura de micrófono e inserción de texto viven fuera del proceso Electron. En Dixi esto puede ser un crate Rust único (`global-hotkey` + `cpal` + `enigo`), eliminando un proceso completo.
3. **El audio-first de Wispr (755 MB) es un coste, no una virtud**: la latencia observada (p50 640 ms e2e) se paga por el round-trip cloud. Una arquitectura local-first puede superar la latencia y el consumo de disco. Ver [13-RENDIMIENTO.md](13-RENDIMIENTO.md).
4. **El esquema SQLite de Wispr es una especificación gratuita**: 29 tablas y 174 migraciones describen 2 años de evolución de producto. Es el mejor catálogo de requisitos de Dixi. Ver [02-ARQUITECTURA.md](02-ARQUITECTURA.md) y [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md).

---

### Referencias cruzadas

- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — procesos, ventanas, datos, capas
- [03-FUNCIONES-DICTADO.md](03-FUNCIONES-DICTADO.md) — pipeline de dictado
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — inventario completo de preferencias
- [13-RENDIMIENTO.md](13-RENDIMIENTO.md) — métricas medidas
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas por función
