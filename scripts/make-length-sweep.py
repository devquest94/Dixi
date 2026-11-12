"""Corta la grabacion mas larga en prefixed de duraciones crecientes.

El contenido de voz es el mismo (es un prefijo): lo unico que cambia es la
duracion. Asi el barrido aísla el efecto de la longitud sobre la latencia de
la ruta de streaming, que es la pregunta del umbral entre 14 y 18 s.

Uso:
    python scripts/make-length-sweep.py <wav-origen> <carpeta-destino> [segundos...]
"""
import io
import os
import sys
import wave

DEFAULT = [10, 12, 13, 14, 15, 16, 17, 18, 20, 22]


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    origen = sys.argv[1]
    destino = sys.argv[2]
    segundos = [float(s) for s in sys.argv[3:]] or DEFAULT
    os.makedirs(destino, exist_ok=True)

    with wave.open(origen, "rb") as w:
        rate = w.getframerate()
        canales = w.getnchannels()
        ancho = w.getsampwidth()
        total = w.getnframes()
        datos = w.readframes(total)

    if canales != 1 or ancho != 2:
        print("el origen debe ser mono de 16 bits")
        return 2

    for s in segundos:
        n = int(s * rate)
        if n > total:
            continue
        nombre = "L%02d.wav" % int(round(s))
        ruta = os.path.join(destino, nombre)
        with wave.open(ruta, "wb") as out:
            out.setnchannels(1)
            out.setsampwidth(2)
            out.setframerate(rate)
            out.writeframes(datos[: n * 2])
        print("  %-10s %5.1f s" % (nombre, s))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
