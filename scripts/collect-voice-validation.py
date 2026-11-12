#!/usr/bin/env python3
"""collect-voice-validation.py - junta las trazas de latencia, el historial y
las grabaciones de las dictaciones reales, y calcula WER y CER.

QUE RECOGE
----------
De la app en marcha (no del banco sintetico):

  * la traza `[latency]` del log, que dice cuanto se espero tras soltar;
  * la entrada del historial, que dice que transcribio;
  * el WAV de la grabacion, que dice cuanto audio habia.

La traza y la entrada del historial se emparejan por la hora: la traza lleva
la marca del log y la entrada del historial lleva la marca de epoch.

De las frases del guion (`docs/validation-voice/frases-voz.txt`) sale la
referencia. Cada dictacion se asocia a la frase mas parecida, de modo que el
WER y el CER se calculan contra un texto conocido.

PRIVACIDAD
----------
El texto transcrito es contenido privado. Por eso el informe lo deja
FUERA por defecto: solo se escribe con `--with-text`, y el texto no se
imprime en la consola nunca.

Uso:
    python scripts/collect-voice-validation.py --out <dir> [--with-text]
"""

import argparse
import json
import os
import re
import sqlite3
import sys
import unicodedata
import wave
from datetime import datetime, timezone

# El log de la app vive en %LOCALAPPDATA% (el log dir de Tauri) y los datos
# (historial y grabaciones) en %APPDATA% (el data dir). Son dos carpetas
# distintas: el log va con la app, los datos van con el perfil del usuario.
APP_DIR = os.path.join(os.environ.get("APPDATA", ""), "com.dixi.app")
LOG_DIR = os.path.join(os.environ.get("LOCALAPPDATA", ""), "com.dixi.app")
LOG_PATH = os.path.join(LOG_DIR, "logs", "dixi.log")
HISTORY_DB = os.path.join(APP_DIR, "history.db")
RECORDINGS_DIR = os.path.join(APP_DIR, "recordings")
PHRASES = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "docs",
    "validation-voice",
    "frases-voz.txt",
)

LOG_LINE = re.compile(
    r"\[(?P<date>\d{4}-\d{2}-\d{2})\]\[(?P<time>\d{2}:\d{2}:\d{2})\].*?\[latency\] (?P<stages>.*)"
)
STAGE = re.compile(r"(?P<name>[a-z_]+)=(?P<value>\d+ms|-)")


def normalizar(texto: str) -> str:
    """Minusculas, sin acentos, solo letras y digitos."""
    texto = unicodedata.normalize("NFKD", texto.lower())
    texto = "".join(c for c in texto if not unicodedata.combining(c))
    return re.sub(r"[^a-z0-9]+", " ", texto).strip()


def palabras(texto: str):
    return normalizar(texto).split()


def ediciones(a, b):
    """Ediciones de palabra entre dos listas (distancia de Levenshtein)."""
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]


def leer_fases():
    """Las frases del guion: id, texto a decir, texto esperado."""
    fases = []
    if not os.path.exists(PHRASES):
        return fases
    with open(PHRASES, encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            partes = linea.split("|")
            if len(partes) < 3:
                continue
            fases.append(
                {"id": partes[0].strip(), "decir": partes[1].strip(), "esperado": partes[2].strip()}
            )
    return fases


def leer_trazas():
    """Las trazas de latencia del log, con su momento."""
    trazas = []
    if not os.path.exists(LOG_PATH):
        return trazas
    with open(LOG_PATH, encoding="utf-8", errors="replace") as f:
        for linea in f:
            m = LOG_LINE.search(linea)
            if not m:
                continue
            # El log de la app marca UTC: el reloj del equipo puede estar en
            # otra zona, asi que las dos marcas se comparan en UTC.
            momento = datetime.strptime(
                f"{m.group('date')} {m.group('time')}", "%Y-%m-%d %H:%M:%S"
            ).replace(tzinfo=timezone.utc)
            etapas = {}
            for s in STAGE.finditer(m.group("stages")):
                valor = s.group("value")
                etapas[s.group("name")] = None if valor == "-" else int(valor[:-2])
            trazas.append({"momento": momento, "etapas": etapas, "linea": linea.strip()})
    return trazas


def leer_historial():
    """Las entradas del historial: archivo, texto y momento."""
    entradas = []
    if not os.path.exists(HISTORY_DB):
        return entradas
    con = sqlite3.connect(f"file:{HISTORY_DB}?mode=ro", uri=True)
    try:
        filas = con.execute(
            "SELECT file_name, timestamp, transcription_text FROM transcription_history"
        ).fetchall()
    except sqlite3.Error:
        return entradas
    finally:
        con.close()
    for file_name, timestamp, texto in filas:
        entradas.append(
            {
                "file_name": file_name,
                "timestamp": timestamp,
                "momento": datetime.fromtimestamp(timestamp, tz=timezone.utc),
                "texto": texto or "",
            }
        )
    return entradas


def duracion_wav(path):
    try:
        with wave.open(path, "rb") as w:
            return w.getnframes() / float(w.getframerate())
    except Exception:
        return None


# Cuanto puede desviarse una dictacion de su frase de referencia y seguir
# siendo la misma frase. Por encima, el emparejamiento no es creible y el
# WER no se calcula: se marca NOT_MEASURED.
MAX_DESVIACION = 0.45


def mejor_frase(texto, fases):
    """La frase del guion que mas se parece a lo transcrito.

    Devuelve (frase, desviacion) o (None, None) si no hay ninguna frase
    creíble: una dictación espontánea no se parece a ninguna frase del guion,
    y puntuarla contra la más parecida daría un WER sin sentido.
    """
    if not fases:
        return None, None
    hip = palabras(texto)
    if not hip:
        return None, None
    mejor = None
    for fase in fases:
        ref = palabras(fase["esperado"])
        if not ref:
            continue
        d = ediciones(hip, ref)
        if mejor is None or d < mejor[0]:
            mejor = (d, fase)
    if mejor is None:
        return None, None
    d, fase = mejor
    desviacion = d / float(len(palabras(fase["esperado"])))
    if desviacion > MAX_DESVIACION:
        return None, desviacion
    return fase, desviacion


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", required=True, help="carpeta del informe")
    ap.add_argument(
        "--with-text",
        action="store_true",
        help="incluye el texto transcrito en el informe (es contenido privado)",
    )
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)

    trazas = leer_trazas()
    entradas = leer_historial()
    fases = leer_fases()

    if not trazas:
        print("No hay trazas de latencia en el log; hacé una dictación primero.")
        return 2
    if not entradas:
        print("No hay entradas en el historial; hacé una dictación primero.")
        return 2

    # Cada traza se empareja con la entrada del historial mas cercana en el
    # tiempo, dentro de una ventana de un minuto.
    registros = []
    usados = set()
    for traza in trazas:
        mejor = None
        for i, entrada in enumerate(entradas):
            if i in usados:
                continue
            delta = abs((entrada["momento"] - traza["momento"]).total_seconds())
            if delta <= 60 and (mejor is None or delta < mejor[0]):
                mejor = (delta, i, entrada)
        if mejor is None:
            continue
        _, idx, entrada = mejor
        usados.add(idx)

        wav = os.path.join(RECORDINGS_DIR, entrada["file_name"])

        fase, desviacion = mejor_frase(entrada["texto"], fases)
        ref_palabras = palabras(fase["esperado"]) if fase else []
        hip_palabras = palabras(entrada["texto"])
        # Sin un emparejamiento creible no hay referencia: el WER y el CER
        # se quedan en NOT_MEASURED en vez de inventar un numero.
        wer = (ediciones(hip_palabras, ref_palabras) / len(ref_palabras)) if ref_palabras else None
        cer = None
        if fase:
            ref_c = normalizar(fase["esperado"])
            hip_c = normalizar(entrada["texto"])
            cer = ediciones(list(hip_c), list(ref_c)) / max(1, len(ref_c))

        etapas = traza["etapas"]
        registro = {
            "hora": traza["momento"].isoformat(timespec="seconds"),
            "archivo": entrada["file_name"],
            "audio_s": duracion_wav(wav),
            "fragmentos_espera_ms": etapas.get("finalize"),
            "tras_soltar_ms": etapas.get("text"),
            "total_ms": etapas.get("total"),
            "insert_ms": etapas.get("insert"),
            "frase": fase["id"] if fase else None,
            "wer": round(wer, 4) if wer is not None else None,
            "cer": round(cer, 4) if cer is not None else None,
        }
        if args.with_text:
            registro["transcripcion"] = entrada["texto"]
            if fase:
                registro["esperado"] = fase["esperado"]
        registros.append(registro)

    json_path = os.path.join(args.out, "validacion-voz.json")
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(registros, f, ensure_ascii=False, indent=2)

    # El informe: sin el texto, salvo que se pida explicitamente.
    lineas = [
        "# Validacion con voz real - Dixi",
        "",
        f"- Dictaciones emparejadas: **{len(registros)}**",
        f"- Trazas en el log: {len(trazas)}; entradas en el historial: {len(entradas)}",
    ]
    if not args.with_text:
        lineas += [
            "",
            "> El texto transcrito NO se incluye: es contenido privado.",
            "> Usa `--with-text` si lo necesitás, y borra el informe despues.",
        ]
    lineas += [
        "",
        "| Hora | Audio (s) | Espera tras soltar (ms) | Insert (ms) | Frase | WER | CER |",
        "|---|---|---|---|---|---|---|",
    ]
    for r in registros:
        lineas.append(
            f"| {r['hora']} | {r['audio_s'] if r['audio_s'] is not None else '-'} "
            f"| {r['fragmentos_espera_ms'] if r['fragmentos_espera_ms'] is not None else '-'} "
            f"| {r['insert_ms'] if r['insert_ms'] is not None else '-'} "
            f"| {r['frase'] or '-'} | {r['wer'] if r['wer'] is not None else '-'} "
            f"| {r['cer'] if r['cer'] is not None else '-'} |"
        )
    wers = [r["wer"] for r in registros if r["wer"] is not None]
    esperas = [r["fragmentos_espera_ms"] for r in registros if r["fragmentos_espera_ms"] is not None]
    if wers:
        lineas += ["", f"WER medio: **{sum(wers)/len(wers):.1%}** sobre {len(wers)} dictaciones"]
    if esperas:
        lineas += [
            "",
            f"Espera tras soltar: minimo **{min(esperas)} ms**, media **{sum(esperas)//len(esperas)} ms**",
        ]
    informe = os.path.join(args.out, "validacion-voz.md")
    with open(informe, "w", encoding="utf-8") as f:
        f.write("\n".join(lineas) + "\n")

    print(f"Informe escrito en {informe}")
    print(f"Datos en {json_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
