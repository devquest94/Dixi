# Sesion guiada de validacion de precision — Dixi

Estado: **preparada, sin ejecutar**. No se ha grabado audio nuevo: esta
sesion la hace la persona, cuando lo decida.

## Para que sirve

Es lo que falta para que la comparacion entre Whisper Base y Nemotron 3.5
sea una **recomendacion con numeros** y no una impresion. Las siete
grabaciones que hay no tienen transcripcion de referencia humana: sin
referencia no hay WER ni CER defendibles.

**No se usa la salida de Whisper como referencia verdadera**: eso compararia
un modelo contra si mismo con distinto reconocedor de idioma.

## Que hay que hacer

1. Abrir Dixi y dejar el motor en el modo que se vaya a medir.
2. Leer las diez frases de `docs/validation-voice/frases-voz.txt`, **en el
   orden indicado**, con una pausa de dos segundos entre una y otra.
3. Dictar cada frase con el atajo de dictado (ctrl+space en push-to-talk:
   apretar, hablar, soltar).
4. Repetir el ciclo entero en el otro modo, para tener las mismas frases
   medidas con los dos motores.
5. No mirar la transcripcion mientras se dicta: leer el texto de referencia
   antes condiciona el dictado.

## Que se recoge

- La app ya escribe la traza de latencia en su log: `dismiss` la etapa
  `finalize` (la espera tras soltar) y el total.
- La entrada del historial guarda el texto transcrito de cada dictado.
- La carpeta de grabaciones guarda el WAV.

El colector une las tres cosas por la hora y las compara con la referencia:

```powershell
python scripts\collect-voice-validation.py --out <carpeta>
```

Sin `--with-text` el informe **no incluye el texto transcrito**: es
contenido privado y no debe quedar en un informe.

## Que sale

- WER y CER por dictado y por media, **solo si hay una sesion guiada**; si
  no, `NOT_MEASURED`.
- La espera tras soltar por dictado, en cada modo.
- Terminos tecnicos en ingles: cuales sobreviven y cuales no.
- Una comparacion lado a lado de los dos motores sobre las mismas frases.

## Lo que hay que mirar en el texto

La revision cualitativa que ya se hizo sobre las siete grabaciones mostró
que:

- Whisper Base alucina en audio corto (una cola en japones al final de un
  dictado de 22 s, un "Thank you" que nadie dijo en uno de 37 s).
- Nemotron no hizo ninguna de esas dos alucinaciones en las mismas
  grabaciones.
- Los dos fallan con los nombres propios tecnicos: "Cloudflare R2" sale
  como "Clawflow R2" o "clavufla R2".
- El voseo se conserva en los dos ("pushea" -> "puse a" en Whisper,
  "pushea" en Nemotron).

La sesion guiada dirá si esas diferencias se sostienen con texto de
referencia conocido, y cuanto se parece cada motor al guion.

## Lo que NO se hace en esta sesion

- No se cambia el modelo activo de forma permanente: se mide con el modo
  que corresponda y se vuelve al anterior.
- No se graba audio para los servicios externos.
- No se borra nada del historial ni de las grabaciones.
