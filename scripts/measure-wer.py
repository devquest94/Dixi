#!/usr/bin/env python3
"""measure-wer.py - calcula WER, CER y acierto de terminologia sobre el
resultado del banco de latencia.

El corpus de referencia (`docs/validation-phase2a/frases.txt`) declara, para
cada audio, lo que hay que decir. El banco (`scripts/measure-latency.ps1`)
guarda lo que el modelo transcribio. De la comparacion sale:

  * WER (word error rate): ediciones de palabra necesarias para convertir la
    transcripcion en la frase de referencia, sobre el total de palabras de la
    referencia.
  * CER (character error rate): lo mismo a nivel de caracter.
  * Acierto de terminologia: que proporcion de los terminos tecnicos de la
    frase sobreviven en la transcripcion.

Advertencia importante: el audio es **sintetico** (espeak-ng, es-419). No es
voz humana, asi que estas cifras miden como se comporta el modelo con una
voz sintetica y no sustituyen a una evaluacion con voz real. Sirven para
comparar modelos entre si, con el mismo corpus y en la misma sesion.

Uso:
    python scripts/measure-wer.py <resultados.json> [<resultados2.json> ...]
"""

import json
import re
import sys
import unicodedata

# Terminos tecnicos que aparecen en el corpus, mas los que el encargo pide
# vigilar. Se comparan en minusculas y sin acentos.
TERMINOS = [
    "server",
    "servidor",
    "logs",
    "log",
    "deploy",
    "build",
    "error",
    "commit",
    "branch",
    "docker",
    "backup",
    "timeout",
    "endpoint",
    "api gateway",
    "merge",
    "stack trace",
    "codigo",
    "memoria",
    "asp.net core",
    "entity framework",
    "postgresql",
    "github",
    "pull request",
    "dependency injection",
]


def normalizar(texto: str) -> str:
    """Minusculas, sin acentos ni signos, con espacios colapsados."""
    texto = unicodedata.normalize("NFKD", texto.lower())
    texto = "".join(c for c in texto if not unicodedata.combining(c))
    texto = re.sub(r"[^a-z0-9]+", " ", texto)
    return re.sub(r"\s+", " ", texto).strip()


def distancia(a: list, b: list) -> int:
    """Distancia de Levenshtein, con dos filas."""
    if len(a) < len(b):
        a, b = b, a
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def leer_frases(ruta: str) -> dict:
    frases = {}
    with open(ruta, encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#") or "|" not in linea:
                continue
            ident, texto = linea.split("|", 1)
            frases[ident.strip()] = texto.strip()
    return frases


def evaluar(resultados: list, frases: dict) -> dict:
    palabras_ref = 0
    palabras_err = 0
    chars_ref = 0
    chars_err = 0
    terminos_total = 0
    terminos_ok = 0
    detalle = []

    for r in resultados:
        ref = frases.get(r["Fixture"].replace(".wav", ""), "")
        if not ref:
            continue
        hyp = r.get("Text") or ""
        rw = normalizar(ref).split()
        hw = normalizar(hyp).split()
        rc = normalizar(ref).replace(" ", "")
        hc = normalizar(hyp).replace(" ", "")

        ed_w = distancia(rw, hw)
        ed_c = distancia(list(rc), list(hc))
        palabras_ref += len(rw)
        palabras_err += ed_w
        chars_ref += len(rc)
        chars_err += ed_c

        ref_n = normalizar(ref)
        hyp_n = normalizar(hyp)
        for t in TERMINOS:
            if t in ref_n:
                terminos_total += 1
                if t in hyp_n:
                    terminos_ok += 1

        detalle.append(
            {
                "fixture": r["Fixture"],
                "ref": ref,
                "hyp": hyp,
                "wer": round(ed_w / max(1, len(rw)), 3),
                "cer": round(ed_c / max(1, len(rc)), 3),
            }
        )

    return {
        "wer": round(palabras_err / max(1, palabras_ref), 4),
        "cer": round(chars_err / max(1, chars_ref), 4),
        "palabras_referencia": palabras_ref,
        "terminos_total": terminos_total,
        "terminos_acertados": terminos_ok,
        "acierto_terminologia": round(terminos_ok / max(1, terminos_total), 4),
        "detalle": detalle,
    }


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2

    frases = leer_frases("docs/validation-phase2a/frases.txt")
    for ruta in sys.argv[1:]:
        with open(ruta, encoding="utf-8-sig") as f:
            resultados = json.load(f)
        informe = evaluar(resultados, frases)
        etiqueta = ruta.replace("\\", "/").split("/")[-2]
        print(f"=== {etiqueta} ===")
        print(
            "  WER={wer}  CER={cer}  ({palabras_referencia} palabras de referencia)".format(
                **informe
            )
        )
        print(
            "  terminologia: {terminos_acertados}/{terminos_total} ({acierto_terminologia})".format(
                **informe
            )
        )
        for d in informe["detalle"]:
            print(
                "    {fixture:<12} wer={wer:<6} cer={cer:<6} | {hyp}".format(**d)
            )
        print()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
