# 12 — UX/UI: arquitectura de pantallas, navegación y flujos

> Fuente: árbol de accesibilidad (UI Automation) de la ventana principal, DOM capturado vía CDP con REA sobre los renderers `status` y `hub`, inventario de ventanas Win32 y flags de servidor.
> **[V]** verificado · **[I]** inferido · **[NV]** no verificado.

---

## 12.1 Modelo de navegación

La ventana principal («Wispr Flow», 1366×860 lógicos / 1708×1075 físicos a 125 %) es un **hub** con navegación lateral fija de 8 secciones de primer nivel.

### 12.1.1 Navegación principal (verificada por UIA)

| # | Sección | AutomationId | Icono | Contenido verificado |
|---|---|---|---|---|
| 1 | **Dictado** | `focus-overlay-target-sidebar-home` | 🎤 micrófono | Página de inicio del flujo de dictado |
| 2 | **Tomador de notas** | `focus-overlay-target-sidebar-notetaker` | ◉ (círculo relleno) | Checklist de 4 pasos + reuniones |
| 3 | **Estadísticas** | `focus-overlay-target-sidebar-insights` | 📊 barras | Métricas de uso |
| 4 | **Diccionario** | `focus-overlay-target-sidebar-dictionary` | 📖 libro | Términos y sustituciones |
| 5 | **Fragmentos** | `focus-overlay-target-sidebar-snippets` | ✂ tijeras | Snippets |
| 6 | **Estilo** | `focus-overlay-target-sidebar-style` | Tt | 5 estilos + desplegable |
| 7 | **Transforms** | `focus-overlay-target-sidebar-transforms` | ✎ lápiz | Catálogo de transforms |
| 8 | **Bloc de notas** | `focus-overlay-target-sidebar-notes` | 🗒 nota | Notas del usuario |

> **Patrón de nomenclatura [V]:** todos los IDs de navegación usan el prefijo `focus-overlay-target-sidebar-*`. El prefijo `focus-overlay-target-` es la **misma familia** que usan los targets del overlay de foco (el `Context Menu` / overlay de foco). ⇒ Wispr comparte un **único sistema de targets de overlay** entre el hub y los overlays flotantes. Es un patrón elegante de **un solo modelo de input** para hub y overlay. [I]

> **⚠️ Hallazgo de deriva (drift) [V]:** el sidebar **público** de Wispr Flow documentado en su base de conocimiento **no incluye una entrada «Transforms»** — pero la instalación auditada (1.6.1174) **sí** la tiene, con icono propio ✎. Lo mismo ocurre con la sección «Bloc de notas»/Scratchpad, que existe en la app pero no aparece como sección principal pública.
>
> **Interpretación [I]:** la app 1.6.1174 **va por delante de su propia documentación pública** en al menos dos funciones (Transforms y Bloc de notas), y varias funciones están en **rollout progresivo por flag de servidor** (§2.7). Esto significa que **auditar sólo la documentación pública habría producido una matriz de paridad incorrecta** — de ahí el valor de la auditoría sobre la app real.

### 12.1.2 Navegación secundaria

| Elemento | Estado | Nota |
|---|---|---|
| **Invita a tu equipo** | visible | CTA de crecimiento (equipo/diccionario compartido) |
| **Consigue un mes gratis** | visible | CTA de referidos (`referralCode` presente en `config.json`) |
| **Configuración** | visible con badge **«Requiere atención»** | ⚠️ badge de onboarding: hay algo pendiente |
| **Ayuda** | visible | |

> El badge **«Requiere atención»** sobre Configuración es la señalización de un estado pendiente en la configuración (probablemente el checklist de MCP/calendario pendiente, coherente con `notetakerChecklistMcpDone = false`). [I]

### 12.1.3 Cabecera

| Elemento | Detalle |
|---|---|
| Logo Wispr Flow | izquierda |
| **Theme** | botón de tema (claro/oscuro) — `theme = "light"`, `darkModeCoachmarkSeen = true` |
| **Notificaciones** | botón con badge; abre el panel de notificaciones |
| **Cuenta** | menú de cuenta (menú desplegable abierto en la captura) |

---

## 12.2 Las 5 superficies de la aplicación

| Superficie | Ventana | Tamaño | Tipo | Cuándo aparece |
|---|---|---|---|---|
| **Hub** | «Wispr Flow» | 1366×860 | Redimensionable, con sidebar | Al hacer clic en el icono de la bandeja |
| **Flow Bar** | «Status» | 512×614 (lógico) | Frameless, siempre encima, forma configurable | **Siempre visible** (es el widget de estado) |
| **Context Menu** | «Context Menu» | 2048×1112 (pantalla completa) | Overlay translúcido a pantalla completa | Al abrir el overlay de contexto |
| **Meeting Recorder** | «Meeting Recorder» | 634×1112 | Panel de grabación | Al grabar una reunión |
| **Scratchpad** | «Scratchpad» | 506×423 | Ventana compacta | Acceso rápido a notas |

**Arquitectura de overlays [V + I]:**

```
                    ┌──────────────────────────┐
  fondo  ─────────▶│  Context Menu (fullscreen)│  ← overlay modal, captura todo
                    └───────────┬──────────────┘
                                │
                    ┌───────────▼──────────────┐
                    │      Hub (1366×860)       │  ← ventana normal
                    └───────────┬──────────────┘
                                │
                    ┌───────────▼──────────────┐
                    │  Flow Bar (512×614)       │  ← siempre encima (top-most)
                    │  data-bar-position        │
                    │  data-status-window-shape │
                    │  data-indicator-state     │
                    └──────────────────────────┘
```

> **Dixi puede simplificar esto [I]:** con Tauri, la Flow Bar puede ser **una sola ventana always-on-top con `decorations: false` y `set_ignore_cursor_events`**, y el Context Menu un overlay propio. La ventana del Hub puede ser la ventana principal normal. **Dixi necesita 3 ventanas, Wispr necesita 5** (y además 2 renderers menos: el Context Menu y el Scratchpad pueden ser páginas del Hub).

---

## 12.3 Flow Bar (la pieza de UX más característica)

### 12.3.1 DOM observado (renderer `status`, 228 nodos)

| Atributo | Valores observados | Significado |
|---|---|---|
| `data-bar-position` | presente | Posición de anclaje (bottom) |
| `data-status-window-shape` | presente | Forma de la ventana |
| `data-status-window-shape-inflate` | presente | Estado «inflado» (al pulsar) |
| `data-status-window-native-shape` | presente | Forma nativa (vs. la del SO) |
| `data-indicator-state` | presente | Estado del indicador |
| `data-tour-anchor` | presente | Ancla del tour |
| `data-testid` | 7 elementos | Testing automatizado instrumentado |
| `aria-label` | 3 | Accesibilidad |
| `aria-haspopup` / `aria-expanded` | 2 / 2 | Botón con menú desplegable |

**Composición [V]:** 74 nodos `#text`, 58 `STYLE`, 49 `DIV`, 15 `path` (SVG), 7 `svg`, 5 `BUTTON`, 4 `SPAN`, 2 `circle`, 2 `rect`, 1 `clipPath`, 1 `g`, 1 `defs`.

⇒ El indicador de estado es **un gráfico vectorial** (15 paths + 2 circles + 2 rects = hasta **15 estados visuales distintos**), no un simple icono. Los 2 `circle` + 2 `rect` sugieren **una waveform / barra de nivel de audio**. [I]

### 12.3.2 Requisitos de diseño para Dixi

| Requisito | Implementación en Dixi |
|---|---|
| Always-on-top, frameless | `Tauri` + `alwaysOnTop: true`, `decorations: false` |
| Forma redondeada personalizable | `setShape()` con `set_ignore_cursor_events` fuera de la forma |
| Posición anclable (bottom) | Persistir posición en `config.json` |
| Indicador de nivel de audio | Analizador sobre el stream de captura (Rust) → IPC al front |
| Estados: inactivo / escuchando / transcribiendo / pegando | Máquina de estados explícita |
| Arrastrable | Flag `can-drag-flow-bar` existe en Wispr ⇒ **Dixi debe poderlo** |
| Click-through en zonas vacías | `set_ignore_cursor_events(true)` |

---

## 12.4 Onboarding y descubrimiento: el problema observado

La instalación auditada revela un **problema de diseño de producto** que es la lección más importante de esta fase:

```
userJourney.features:
  formatting         → discoveredAt ✓  firstUsedAt ✓  usageCount 40
  dictionary         → discoveredAt ✗  firstUsedAt ✗  usageCount 0
  snippets           → discoveredAt ✗  firstUsedAt ✗  usageCount 0
  polish             → discoveredAt ✗  firstUsedAt ✗  usageCount 0
  personalizedStyles → discoveredAt ✓  firstUsedAt ✗  usageCount 0
```

> **4 de las 5 funciones del producto nunca fueron descubiertas por el usuario.** El usuario completó el onboarding (`postOnboardingCompleted = true`, 13 páginas) y descubrió el formateo a los 60 s, pero **nunca vio** el diccionario, los fragmentos ni los Transforms.

**Causas probables [I]:**
1. Los accesos a estas funciones están **dentro de la app** (sidebar), no en el flujo natural de dictado.
2. El valor del formateo es **inmediato**; el del diccionario/Transforms requiere un **esfuerzo activo** (configurar, escribir una instrucción).
3. El badge «Requiere atención» sobre Configuración y el checklist del Notetaker **compiten** por la atención del usuario.

**Lecciones para Dixi (aplicables directamente):**

| Lección | Aplicación |
|---|---|
| **Descubrimiento ≠ disponibilidad** | Instrumentar `discoveredAt`/`firstUsedAt` desde el día 1; sin eso no se sabe qué función es invisible |
| **El primer resultado debe requerir cero configuración** | El formateo automático es el producto. El LLM/las funciones avanzadas son **segunda capa** |
| **Los fragmentos con variables se descubren solos** | «Dile a Dixi: *inserta mi firma*» ⇒ se usa sin ir a Configuración. Es el camino de descubrimiento natural de la función que Wispr no tiene |
| **Una sola pantalla, no 8** | El sidebar de 8 secciones reparte la atención. **Dixi v1 debería tener 3**: Dictado · Ajustes · Historial. Lo demás se descubre por voz |
| **El badge de «Requiere atención» es un costo** | En esta instalación había un badge que señalaba algo que el usuario no atendió ⇒ ruido. **Dixi no debe usar badges** |

---

## 12.5 Onboarding y tours

| Mecanismo | Evidencia |
|---|---|
| Tour de bienvenida (modal) | Visible en la captura: «¿Quieres saltarte esto?» con botones **«Mostrar todo»** y **«Saltar la introducción»**, y un botón «Cerrar» |
| Video de onboarding | **745 670 B de `video/webm`** cargado por el renderer `hub` [V] |
| Ilustraciones de onboarding | 4 PNG de gran tamaño: 594 930 B, 633 414 B, 252 284 B, 58 875 B [V] |
| Checklist del Notetaker | 4 pasos con estado individual |
| Coachmarks | 15+ flags `*CoachmarkSeen/Clicked` [V] |
| `pagesCompleted` | 13 páginas de onboarding |
| `tutorials` | `{sentMessages, sentEmails, sentMessagesAI, usedHandsfree, usedWhisperMode}` ⇒ **Wispr envía correos al usuario** paraKdiseñar la activación |

> **Lectura [I]:** Wispr gasta una cantidad **desproporcionada** de esfuerzo en el onboarding (video webm de 745 KB, 4 ilustraciones grandes, 13 páginas, 15+ coachmarks, correos). Para un reemplazo personal, **Dixi debería invertir 1/10 de ese esfuerzo**: un onboarding de 3 pasos, sin video, sin tour, y que la funcionalidad se descubra **usando la app**.

---

## 12.6 Panel de notificaciones

Estado observado: **abierto**, superpuesto a la derecha (x ≈ 1611–2107 px), con:

| Contenido | Detalle |
|---|---|
| Título | «Notificaciones» |
| Subtítulo | «Configura tus notificaciones» |
| Notificaciones de hitos | **10 000**, **12 345**, **20 000**, **50 000**, **99 000**, **100 000 palabras** |
| Notificaciones de racha | **1 mes**, **3 meses** |
| Botón de filtro | «Todas» / «No leídas» (2 botones con `aria-label`) |
| Iconos | Varios `img` con `data-src` (badges de logro SVG) |

> **Dixi:** el panel de notificaciones con 16 logros y 3 anuncios es **innecesario** para un reemplazo personal. Recomendación: **sin panel de notificaciones**; el estado (último dictado, errores) se muestra en la Flow Bar.

---

## 12.7 Accesibilidad

**Lo que Wispr hace bien [V]:**

1. **Instrumentación de accesibilidad de fábrica**: `aria-label`, `role`, `aria-haspopup`, `aria-expanded`, `aria-hidden`, `aria-live`, `data-testid` en la Flow Bar; `aria-label` en el hub; `aria-labelledby` en el contenedor de la barra.
2. **El árbol AX se captura en producción**: `useAxContext = true` ⇒ el mismo sistema de accesibilidad que sirve para captura de contexto sirve para **accesibilidad real**. Es la misma tecnología en dos usos.
3. **Modo de depuración visual**: `showDebugOutlines`, `showOverlayDebugTint`.
4. **Aparecen las 55 traducciones de locale** ⇒ i18n completo, no sólo inglés.

**Lo que Wispr hace mal [V]:**

1. `aria-label` sólo en 3 elementos de la Flow Bar ⇒ la mayoría de los controles **no tienen nombre accesible**.
2. `shouldSaveAxText = false` ⇒ se usa la accesibilidad pero **no se guarda el contexto** ⇒ la historia de accesibilidad no es auditable.
3. `prefs.permissions` con `accessibility.granted = false` ⇒ Wispr **usa accesibilidad sin haber pedido permiso explícito** en la UI. Es un gris de privacidad.

> **Dixi debe:** pedir el permiso de accesibilidad **explícitamente** en el primer arranque (con explanation de para qué), labelling completo de todos los controles, y `aria-live` para anunciar el estado del dictado a lectores de pantalla. Con Tauri + WebView2, la accesibilidad funciona de forma nativa si el HTML es correcto.

---

## 12.8 Internacionalización

| Aspecto | Estado |
|---|---|
| Locale de UI | `es` (español) — la instalación auditada está en español |
| Packs de locale | **55** ⇒ cobertura internacional completa |
| Diccionario ortográfico | `es-ES-3-0.bdic` (Chromium) |
| Idiomas de dictado | `en`, `es` (detectados y seleccionados) |
| Formato de fecha/hora | `America/Buenos_Aires` + `timezoneOffsetMinutes` por dictado |

---

## 12.9 Resumen de la UX de Wispr y la UX objetivo de Dixi

| Dimensión | Wispr Flow | **Dixi (objetivo)** |
|---|---|---|
| Secciones del hub | 8 | **3** (Dictado · Ajustes · Historial) |
| Ventanas | 5 | **3** (Hub · Flow Bar · Grabador) |
| Overlay de contexto | Ventana fullscreen separada | **Integrated en la Flow Bar** o comando de voz |
| Onboarding | 13 páginas + video webm + 15 coachmarks | **3 pasos, sin video, sin tour** |
| Gamificación | Rachas + hitos + leaderboard + nudges con cohortes | **Racha local opcional, sin notificaciones** |
| Notificaciones | Panel con 19 Achievement/Announcement | **Ninguna** (estado en la Flow Bar) |
| Accesibilidad | Parcial (`aria-label` en 3 elementos) | **Completa**, con permiso explícito |
| Descubrimiento de funciones | 1 de 5 descubierta por el usuario | **Descubrimiento por voz** («Dixi, inserta mi firma») |
| Localización | 55 locales | 2 (es/en) al inicio |
| Tema | claro/oscuro | claro/oscuro + **sistema** |

---

### Referencias cruzadas

- [02-ARQUITECTURA.md](02-ARQUITECTURA.md) — ventanas y renderers
- [06-ESTILOS.md](06-ESTILOS.md) — estilos (sección «Estilo»)
- [09-STATS.md](09-STATS.md) — gamificación y embudo de descubrimiento
- [10-MCP.md](10-MCP.md) — «Ask anything» y MCP
- [11-CONFIGURACION.md](11-CONFIGURACION.md) — preferencias de UI
- [16-MATRIZ-PARIDAD.md](16-MATRIZ-PARIDAD.md) — brechas
