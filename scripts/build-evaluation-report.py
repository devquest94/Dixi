#!/usr/bin/env python3
"""build-evaluation-report.py - genera las tablas del informe de evaluacion
leyendo directamente los JSON del banco.

Los numeros NO se transcriben a mano: se leen de
`ejecuciones*.json` (una fila por ejecucion), que es la unica fuente de
verdad. Asi el informe no puede desincronizarse de la medicion.

Uso:
    python scripts/build-evaluation-report.py --bench <carpeta-del-banco> --out <fichero.md>
"""
import argparse
import io
import json
import os


def cargar(ruta):
    with io.open(ruta, "r", encoding="utf-8-sig") as f:
        return json.load(f)


def percentil(valores, p):
    if not valores:
        return 0
    ordenados = sorted(valores)
    if len(ordenados) == 1:
        return ordenados[0]
    import math

    rango = max(0, min(len(ordenados) - 1, int(math.ceil(p / 100.0 * len(ordenados))) - 1))
    return ordenados[rango]


def describe(valores):
    if not valores:
        return "n/d"
    return "min %d | p50 %d | p95 %d | max %d" % (
        min(valores),
        percentil(valores, 50),
        percentil(valores, 95),
        max(valores),
    )


def tabla_por_grabacion(ejecuciones, campo="PostMs"):
    """Una fila por grabacion, con la distribucion de SUS repeticiones."""
    por = {}
    for e in ejecuciones:
        por.setdefault(e["Fixture"], []).append(e)
    filas = []
    for nombre in sorted(por, key=lambda k: por[k][0]["AudioSecs"]):
        vals = [e[campo] for e in por[nombre]]
        filas.append(
            "| %s | %.2f | %d | %d | %d | %d | %d |"
            % (
                nombre,
                por[nombre][0]["AudioSecs"],
                len(vals),
                percentil(vals, 50),
                percentil(vals, 95),
                min(vals),
                max(vals),
            )
        )
    return "\n".join(filas)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bench", required=True, help="carpeta con los tres motores")
    ap.add_argument("--out", required=True, help="fichero markdown de salida")
    ap.add_argument(
        "--a",
        default="",
        help="fichero de ejecuciones del motor A (por lotes de la app)",
    )
    ap.add_argument(
        "--c",
        default="",
        help="fichero de ejecuciones del motor C (streaming)",
    )
    ap.add_argument(
        "--b",
        default="",
        help="fichero de ejecuciones del motor B (Nemotron por lotes)",
    )
    args = ap.parse_args()

    def ruta(por_defecto, explicito):
        if explicito:
            return explicito
        return os.path.join(args.bench, por_defecto)

    b = args.bench
    a_ej = cargar(ruta("A", args.a))
    c_ej = cargar(ruta("C", args.c))
    b_ej = cargar(ruta("B", args.b))

    a_post = [e["PostMs"] for e in a_ej]
    c_post = [e["PostMs"] for e in c_ej]
    b_asr = [e["AsrMs"] for e in b_ej]

    # Subconjunto por duracion: el corte esta medido, no supuesto.
    CORTE = 16.0
    a_cort = [e["PostMs"] for e in a_ej if e["AudioSecs"] <= CORTE]
    a_larg = [e["PostMs"] for e in a_ej if e["AudioSecs"] > CORTE]
    c_cort = [e["PostMs"] for e in c_ej if e["AudioSecs"] <= CORTE]
    c_larg = [e["PostMs"] for e in c_ej if e["AudioSecs"] > CORTE]

    out = []
    out.append("## Metodologia: tres estadisticas que no son lo mismo\n")
    out.append(
        "El banco por lotes y el de la ruta en vivo guardan ahora **una fila\n"
        "por ejecucion** (`ejecuciones*.json`). Sobre esa lista se calculan tres\n"
        "cosas distintas, y antes se confundian:\n"
    )
    out.append(
        "1. **POR GRABACION**: la distribucion de las N repeticiones de cada\n"
        "   grabacion. Es lo que describe una grabacion concreta.\n"
        "2. **AGRUPADA**: la distribucion de TODAS las ejecuciones del motor\n"
        "   (7 grabaciones x 5 repeticiones = 35). Es lo que describe el motor.\n"
        "3. **POR SUBCONJUNTO**: las ejecuciones de las grabaciones cortas\n"
        "   frente a las largas.\n"
    )
    out.append(
        "> El error corregido: los percentiles \"globales\" se calculaban sobre\n"
        "> una fila por grabacion que ya era el MINIMO de sus repeticiones. El\n"
        "> \"p50 global\" era la mediana de 7 minimos, y el \"p95 global\" era un\n"
        "> percentil sobre 7 valores. Ninguno de los dos era el p50 o el p95 de\n"
        "> las 35 ejecuciones.\n"
    )

    out.append("\n## Resultados agrupados (35 ejecuciones por motor)\n")
    out.append("| Motor | N | min | p50 | p95 | max |")
    out.append("|---|---|---|---|---|---|")
    out.append(
        "| Whisper Base, espera tras soltar (ms) | %d | %d | %d | %d | %d |"
        % (
            len(a_post),
            min(a_post),
            percentil(a_post, 50),
            percentil(a_post, 95),
            max(a_post),
        )
    )
    out.append(
        "| Nemotron streaming, espera tras soltar (ms) | %d | %d | %d | %d | %d |"
        % (
            len(c_post),
            min(c_post),
            percentil(c_post, 50),
            percentil(c_post, 95),
            max(c_post),
        )
    )
    out.append(
        "| Nemotron por lotes, inferencia ASR (ms) | %d | %d | %d | %d | %d |"
        % (
            len(b_asr),
            min(b_asr),
            percentil(b_asr, 50),
            percentil(b_asr, 95),
            max(b_asr),
        )
    )

    out.append("\n## Por subconjunto (corte en %.0f s de audio)\n" % CORTE)
    out.append("| Motor | Subconjunto | N | min | p50 | p95 | max |")
    out.append("|---|---|---|---|---|---|---|")
    for nombre, vals in (
        ("Whisper Base", a_cort),
        ("Whisper Base", a_larg),
        ("Nemotron streaming", c_cort),
        ("Nemotron streaming", c_larg),
    ):
        etiqueta = "<= %.0f s" % CORTE if nombre.startswith("Whisper") and vals is a_cort else ""
        etiqueta = ""
        if vals is a_cort:
            etiqueta = "<= %.0f s" % CORTE
        elif vals is a_larg:
            etiqueta = "> %.0f s" % CORTE
        elif vals is c_cort:
            etiqueta = "<= %.0f s" % CORTE
        elif vals is c_larg:
            etiqueta = "> %.0f s" % CORTE
        out.append(
            "| %s | %s | %d | %d | %d | %d | %d |"
            % (
                nombre,
                etiqueta,
                len(vals),
                min(vals),
                percentil(vals, 50),
                percentil(vals, 95),
                max(vals),
            )
        )

    out.append("\n## Whisper Base, por grabacion\n")
    out.append("| Grabacion | Audio (s) | N | p50 | p95 | min | max |")
    out.append("|---|---|---|---|---|---|---|")
    out.append(tabla_por_grabacion(a_ej))

    out.append("\n## Nemotron streaming, por grabacion\n")
    out.append("| Grabacion | Audio (s) | N | p50 | p95 | min | max |")
    out.append("|---|---|---|---|---|---|---|")
    out.append(tabla_por_grabacion(c_ej))

    out.append("\n## Nemotron por lotes, por grabacion (inferencia ASR)\n")
    out.append("| Grabacion | Audio (s) | N | p50 | p95 | min | max |")
    out.append("|---|---|---|---|---|---|---|")
    out.append(tabla_por_grabacion(b_ej, campo="AsrMs"))

    texto = "\n".join(out) + "\n"
    with io.open(args.out, "w", encoding="utf-8", newline="\r\n") as f:
        f.write(texto)
    print("escrito: " + args.out)
    print(texto)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
