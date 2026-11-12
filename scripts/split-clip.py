"""Parte una grabacion en trozos para aislar que parte del contenido
provoca el coste.

Uso:
    python scripts/split-clip.py <wav> <carpeta> <segundos-a-conservar> [etiqueta]
"""
import io
import os
import sys
import wave


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return 2
    origen = sys.argv[1]
    destino = sys.argv[2]
    segundos = float(sys.argv[3])
    etiqueta = sys.argv[4] if len(sys.argv) > 4 else "P"
    os.makedirs(destino, exist_ok=True)

    with wave.open(origen, "rb") as w:
        rate = w.getframerate()
        total = w.getnframes()
        datos = w.readframes(total)

    n = int(segundos * rate)
    if n > total:
        n = total

    # Inicio y final: el mismo numero de muestras desde los dos extremos.
    inicio = datos[: n * 2]
    final = datos[(total - n) * 2 : total * 2]

    for nombre, trozo in (("%s_ini.wav" % etiqueta, inicio), ("%s_fin.wav" % etiqueta, final)):
        ruta = os.path.join(destino, nombre)
        with wave.open(ruta, "wb") as out:
            out.setnchannels(1)
            out.setsampwidth(2)
            out.setframerate(rate)
            out.writeframes(trozo)
        print("  %-12s %5.1f s" % (nombre, len(trozo) / 2.0 / rate))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
