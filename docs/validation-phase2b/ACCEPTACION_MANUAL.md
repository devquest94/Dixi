# Aceptación manual — Fase 2B (dictado, contexto y diccionario)

Esta es la prueba que **necesita tu voz**. El resto de las comprobaciones de
la Fase 2B son automáticas; esta verifica el camino completo: micrófono →
ASR local → contexto → sustituciones → portapapeles → inserción de texto.

No hay que instalar nada ni cambiar la configuración del sistema.

---

## 0. Preparación

1. Lanzá Dixi desde el binario de esta fase:

   ```
   D:\Codigo\Dixi\src-tauri\target\release\dixi.exe
   ```

   O desde una **copia aislada**, para no tocar tus datos reales. La forma
   más limpia es la carpeta portátil:

   ```
   D:\Codigo\Dixi\src-tauri\target\release\portable      (archivo de texto)
   ```

   El contenido del archivo `portable` tiene que ser exactamente:

   ```
   Dixi Portable Mode
   ```

   Con ese archivo al lado del `.exe`, Dixi guarda ajustes, modelos,
   grabaciones y el diccionario en `Data\` junto al ejecutable, y no toca
   `%APPDATA%`. **Borrá la carpeta `Data\` cuando termines** si no la querés
   conservar.

2. Abrí **Ajustes → Diccionario** y agregá estas reglas. Son las que
   prueban los términos técnicos y las prioridades:

   | Término | Reemplazo | Tipo | Prioridad | Idioma |
   |---|---|---|---|---|
   | `asp dot net` | `ASP.NET` | sustitución | 0 | *(vacío)* |
   | `post gre sql` | `PostgreSQL` | sustitución | 0 | *(vacío)* |
   | `entity framework` | `Entity Framework` | sustitución | 0 | *(vacío)* |
   | `git hub` | `GitHub` | sustitución | 0 | *(vacío)* |
   | `equipo` | `equipo interno` | sustitución | 0 | *(vacío)* |
   | `equipo` | `equipo de la casa` | sustitución | 10 | *(vacío)* |
   | `server` | `servidor` | sustitución | 0 | `en` |
   | `PostgreSQL` | *(vacío)* | término | 0 | *(vacío)* |

   Las dos reglas de «equipo» comparten término a propósito: gana la de
   **prioridad 10**. La de «server» lleva etiqueta de idioma `en`, así que
   **no debe tocar** una transcripción en español.

3. En **Ajustes → General** (o donde esté el campo), poné el **contexto de
   transcripción**:

   ```
   El usuario habla español rioplatense y trabaja con .NET y PostgreSQL.
   ```

4. Dejá el **posprocesado con IA apagado** para esta prueba: así se aísla el
   diccionario del efecto del proveedor.

---

## 1. Qué decir y qué tiene que aparecer

Usá el atajo de dictado (por defecto, mantener pulsada la tecla de push-to-talk)
y dictá cada frase. Anotá el texto que queda en el portapapeles.

### 1.1 Sustituciones técnicas

| Dictá | Tiene que quedar |
|---|---|
| «hoy deployamos asp dot net contra post gre sql» | «hoy deployamos ASP.NET contra PostgreSQL» |
| «con entity framework y subimos a git hub» | «con Entity Framework y subimos a GitHub» |

### 1.2 Prioridad

| Dictá | Tiene que quedar |
|---|---|
| «el equipo está listo» | «el equipo de la casa está listo» |

> Si queda «equipo interno», la prioridad no se está aplicando: avisame.

### 1.3 Regla con idioma

| Dictá | Tiene que quedar |
|---|---|
| «el server no responde» (en español) | «el server no responde» — **sin cambios** |

> La regla está etiquetada como `en`, así que una transcripción en español
> no la toca. Si aparece «servidor», la regla se está aplicando sin mirar
> el idioma.

### 1.4 Contexto de transcripción

Con el contexto puesto, el modelo debería escribir mejor los términos. Para
comprobarlo sinmetricas:

| Dictá | Resultado esperado |
|---|---|
| «mañana revisamos el plan de despliegue con el equipo» | «mañana revisamos el plan de despliegue con el equipo de la casa» |

### 1.5 El resto del camino

| Qué probar | Cómo | Qué tiene que pasar |
|---|---|---|
| **Micrófono** | Dictá cualquier cosa | Se oye, aparece el indicador de grabación |
| **ASR local** | «prueba de micrófono» | El texto es correcto, sin esperas largas |
| **Portapapeles** | Copiá algo, dictá otra cosa | El portapapeles queda con el texto dictated |
| **Inserción de texto** | Abrí el Bloc de notas, dictá | El texto aparece **en el Bloc de notas**, no en el portapapeles |
| **Cancelación** | Empezá a dictá y soltá antes de terminar | No se pega nada |
| **Cancelación con Escape** | Empezá a dictá y pulsá Escape | Se cancela, no se pega nada |
| **Recuperación ante error** | Con el micrófono tapado o desconectado, dictá | La app **no se cuelga** y avisa del error |

### 1.6 Importar y exportar

1. En **Diccionario → Exportar**, elegí un archivo nuevo (por ejemplo
   `diccionario-prueba.json`).tiene que aparecer un aviso de que el archivo
   ya existe si repetís la operación sobre el mismo nombre: el backend
   **no** reemplaza un archivo sin que confirmes.
2. **Importá** ese mismo archivo. Tiene que avisar que se importaron 8
   entradas (o las que hayas creado).
3. Importá el mismo archivo otra vez: tiene que decir que se importaron **0**
   entradas, porque ya estaban.
4. Borrá una entrada, importá el archivo otra vez, y comprobá que la entrada
   borrada **vuelve**: el archivo es un respaldo real, no una foto del
   estado.

### 1.7 Reinicio

1. Cerrá Dixi por completo.
2. Volvé a lanzarlo.
3. Abrí **Diccionario**: las entradas tienen que seguir ahí, con su
   activación, su prioridad y su idioma.

---

## 2. Qué anotar

| Qué | Dónde |
|---|---|
| Texto final de cada frase | Tabla de este documento |
| Si algo falló | Nota: qué se esperaba y qué pasó |
| Si la app se cuelga o avisa | Nota: momento exacto |
| Latencia percibida | La de la 1.1, en segundos |

## 3. Qué NO hay que tocar

- El diccionario ni las preferencias de **Wispr Flow**: esta prueba es
  únicamente sobre Dixi.
- Los ajustes de `%APPDATA%` de la instalación normal: usá la copia portátil
  (paso 0.1) si no querés que estos cambios se mezclen con los tuyos.
