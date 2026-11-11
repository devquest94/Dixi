# mRemoteNG — pruebas de control manuales

## Estado

La integracion con el escritorio remoto sigue **desactivada para el pegado
automatico**, tal y como pedia el encargo:

| Ajuste | Valor por defecto | Que significa |
|---|---|---|
| `remote_desktop_enabled` | `false` | La integracion con el cliente remoto esta apagada |
| `remote_desktop_auto_mode` | `false` | Dixi NO pega solo en la sesion remota |
| `remote_desktop_manual_mode` | `true` | Dixi deja el texto en el portapapeles y el usuario pega a mano |
| `remote_desktop_restore_clipboard` | `true` | Al terminar la transferencia se restaura el portapapeles anterior |

No se toco ninguna conexion de trabajo ni ninguna politica corporativa.

## Por que hace falta un control

ElPegado remoto falla por una de estas dos razones, y son cosas distintas:

1. **El canal de portapapeles del cliente remoto no transporta el texto.**
   Es un problema del cliente (mRemoteNG sobre RDP): no hay nada que arreglar
   en Dixi.
2. **El canal funciona y el fallo es de Dixi** (el enfoque, el destino, la
   ventana, el tiempo de espera).

Hasta saber cual de las dos es, no se puede decidir si hay algo que arreglar.
Por eso las pruebas de control van **de dentro hacia fuera** y **de fuera
hacia dentro**, una en cada sentido.

## Las pruebas

Cada prueba la hace una persona, en su sesion, cuando lo indique. Ninguna
toca la configuracion ni las conexiones: solo copian, pegan y anotan.

### Control 0 — el pegado local funciona

1. En el escritorio local, abrir el Bloc de notas.
2. Escribir a mano una frase de prueba.
3. Seleccionarla y copiarla (Ctrl+C).
4. Pegarla en otro Bloc de notas local (Ctrl+V).

**Si la frase pegada NO aparece**: el fallo es del pegado local de Dixi o
del portapapeles del sistema, y no tiene nada que ver con el cliente remoto.
Se para aqui.

**Si aparece**: seguir.

### Control 1 — del escritorio local a la sesion remota

1. Con la sesion RDP de mRemoteNG abierta y enfocada, copiar en el
   escritorio local una frase de prueba (Ctrl+C en un Bloc de notas local).
2. En la sesion remota, abrir un Bloc de notas y pegar (Ctrl+V).

**Resultado**: la frase pegada es la de arriba o no.

- **Si pega**: la redireccion local -> remoto funciona. Un fallo del
  pegado automatico de Dixi seria entonces de Dixi.
- **Si NO pega**: el canal de portapapeles de mRemoteNG no transporta el
  texto en este sentido. Anotar "MREMOTENG_CLIPBOARD=FALLO local->remoto".

### Control 2 — de la sesion remota al escritorio local

1. Dentro de la sesion remota, escribir una frase en un Bloc de notas
   remoto y copiarla (Ctrl+C dentro de la sesion).
2. En el escritorio local, pegar en un Bloc de notas local (Ctrl+V).

**Resultado**:

- **Si pega**: la redireccion remoto -> local funciona.
- **Si NO pega**: el canal no transporta el texto en este sentido. Anotar
  "MREMOTENG_CLIPBOARD=FALLO remoto->local".

### Control 3 — el modo manual de Dixi dentro de la sesion remota

1. Con la sesion RDP enfocada, dictar con Dixi (atajo de dictado).
2. El texto NO se pega solo: queda en el portapapeles del equipo local.
3. Volver a la sesion remota y pegar a mano (Ctrl+V).

**Resultado**:

- **Si el texto aparece en la sesion remota**: la redireccion
  local -> remoto funciona y el fallo del pegado automatico esta en la
  parte de Dixi (enfoque, destino, ventana o tiempo de espera).
- **Si NO aparece**: el canal de portapapeles no transporta el texto desde
  el equipo local, y la integracion automatica no puede funcionar con este
  cliente. La integracion sigue desactivada.

### Control 4 — el portapapeles local sobrevive al dictado

1. Copiar un archivo (o una imagen) en el escritorio local.
2. Dictar con Dixi.
3. Comprobar si el archivo o la imagen siguen en el portapapeles.

**Resultado**: si desaparecieron, el pegado de Dixi esta destruyendo el
contenido del portapapeles. Es el defecto que se corrigio en esta rama
(el portapapeles ya no se vacia cuando su contenido no se puede
restaurar), asi que este control verifica la correccion.

## Como se anotan

Una linea por control, en el informe de la sesion:

```
MREMOTENG_CONTROL 0 local          PASA
MREMOTENG_CONTROL 1 local->remoto  PASA|FALLA
MREMOTENG_CONTROL 2 remoto->local  PASA|FALLA
MREMOTENG_CONTROL 3 manual-remoto  PASA|FALLA
MREMOTENG_CONTROL 4 portapapeles   PASA|FALLA
```

## Que NO se hace

- No se modifica la configuracion de mRemoteNG ni del servidor.
- No se cambia ninguna politica corporativa.
- No se activa `remote_desktop_auto_mode` ni `remote_desktop_enabled`.
- No se abre ni se cierra ninguna sesion de trabajo.

## Que habilita la integracion automatica

Solo si **los cuatro controles de transporte pasan** (1, 2 y 3) y hay una
sesion remota unica y enfocada. Con algun control en FALLA, la
integracion automatica queda desactivada y el uso es el manual.
