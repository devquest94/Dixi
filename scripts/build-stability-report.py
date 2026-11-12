#!/usr/bin/env python3
"""build-stability-report.py - arma las tablas del informe de estabilidad.

Las tablas se CALCULAN a partir de los JSON de las mediciones: nada se
transcribe a mano. El informe (`docs/dixi-streaming-stability-report.md`) tiene
marcadores `<!--TABLA:NOMBRE-->`; este script los sustituye por las tablas
reales y deja los marcadores en su sitio, para que se pueda re-ejecutar.

Uso:
    python scripts/build-stability-report.py \
        --antes <dir> --despues <dir> [--cancelacion <dir>] \
        --informe docs/dixi-streaming-stability-report.md
"""

import argparse
import json
import os
import re
import statistics
import unicodedata


def load_rows(directory):
    path = os.path.join(directory, "ejecuciones-estabilidad.json")
    if not os.path.isfile(path):
        return []
    with open(path, encoding="utf-8-sig") as f:
        return json.load(f)


def percentile(values, pct):
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return float(ordered[0])
    rank = max(0, min(len(ordered) - 1, int(-(-pct * len(ordered) // 100)) - 1))
    return float(ordered[rank])


def stats(values):
    values = [float(v) for v in values if v is not None]
    if not values:
        return None
    return {
        "n": len(values),
        "media": statistics.mean(values),
        "min": min(values),
        "p50": percentile(values, 50),
        "p95": percentile(values, 95),
        "max": max(values),
    }


def fmt(value, digits=0):
    if value is None:
        return "-"
    if digits == 0:
        return f"{value:,.0f}".replace(",", " ")
    return f"{value:,.{digits}f}".replace(",", " ").replace(".", ",")


def stat_row(label, st, unit="", digits=0):
    if st is None:
        return f"| {label} | 0 | - | - | - | - | - |"
    return (
        f"| {label} | {st['n']} | {fmt(st['media'], digits)} | {fmt(st['min'], digits)} "
        f"| {fmt(st['p50'], digits)} | {fmt(st['p95'], digits)} | {fmt(st['max'], digits)} |{unit}"
    )


def words(text):
    """Words of a transcription, normalized for comparison."""
    if not text:
        return []
    normalized = unicodedata.normalize("NFKD", text.lower())
    normalized = "".join(c for c in normalized if not unicodedata.combining(c))
    return [w for w in re.split(r"[^0-9a-z]+", normalized) if w]


def duplicated_words(text):
    """Consecutive repetitions of the same word: the sign of a doubled fragment."""
    seq = words(text)
    return [
        (seq[i - 1], seq[i])
        for i in range(1, len(seq))
        if seq[i - 1] == seq[i] and len(seq[i]) > 2
    ]


def word_diff(a, b):
    """Words of `a` that are not in `b` (omitted) and the other way round."""
    wa, wb = words(a), words(b)
    from collections import Counter

    ca, cb = Counter(wa), Counter(wb)
    omitted = list((ca - cb).elements())
    added = list((cb - ca).elements())
    return omitted, added


def table_scenario(label, rows):
    """The aggregate table of one scenario."""
    wait = stats([r["EsperaMs"] for r in rows])
    cpu = stats([r["CpuPct"] for r in rows])
    ram = stats([r["RamPrivMB"] for r in rows])
    backlog = stats([r.get("RetrasoMs") for r in rows])
    lines = [
        "| Metrica | N | Media | Min | p50 | p95 | Maximo |",
        "|---|---|---|---|---|---|---|",
        stat_row("Espera tras soltar (ms)", wait),
        stat_row("CPU del arbol (%)", cpu, digits=1),
        stat_row("RAM privada del arbol (MB)", ram, digits=1),
        stat_row("Retraso pendiente al soltar (ms)", backlog),
    ]
    return "\n".join(lines)


def table_per_audio(before_rows, after_rows):
    """One row per clip: the wait before and after the change."""
    def by_clip(rows):
        grouped = {}
        for r in rows:
            grouped.setdefault(r["Audio"], []).append(r)
        return grouped

    gb, ga = by_clip(before_rows), by_clip(after_rows)
    lines = [
        "| Audio | s | N | p50 espera antes (ms) | p95 antes (ms) | max antes (ms) "
        "| p50 espera despues (ms) | p95 despues (ms) | max despues (ms) "
        "| p50 retraso despues (ms) | rutas despues |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for clip in sorted(set(gb) | set(ga)):
        rb = gb.get(clip, [])
        ra = ga.get(clip, [])
        sb = stats([r["EsperaMs"] for r in rb])
        sa = stats([r["EsperaMs"] for r in ra])
        bl = stats([r.get("RetrasoMs") for r in ra])
        rutas = ",".join(sorted({r["Ruta"] for r in ra})) or "-"
        lines.append(
            f"| {clip} | {fmt(ra[0]['AudioSegs'] if ra else rb[0]['AudioSegs'], 2)} "
            f"| {sb['n'] if sb else 0} | {fmt(sb['p50']) if sb else '-'} "
            f"| {fmt(sb['p95']) if sb else '-'} | {fmt(sb['max']) if sb else '-'} "
            f"| {fmt(sa['p50']) if sa else '-'} "
            f"| {fmt(sa['p95']) if sa else '-'} | {fmt(sa['max']) if sa else '-'} "
            f"| {fmt(bl['p50']) if bl else '-'} | {rutas} |"
        )
    return "\n".join(lines)


def table_paths(rows):
    """How many runs took each route."""
    live = [r for r in rows if r["Ruta"] == "live"]
    batch = [r for r in rows if r["Ruta"] == "batch"]
    lines = [
        "| Ruta | Corridas | Espera p50 (ms) | Espera p95 (ms) | Espera max (ms) |",
        "|---|---|---|---|---|",
    ]
    for name, group in (("live (el motor finaliza el stream)", live), ("batch (pasada completa)", batch)):
        st = stats([r["EsperaMs"] for r in group])
        if st is None:
            lines.append(f"| {name} | 0 | - | - | - |")
        else:
            lines.append(
                f"| {name} | {st['n']} | {fmt(st['p50'])} | {fmt(st['p95'])} | {fmt(st['max'])} |"
            )
    return "\n".join(lines)


def table_integrity(before_rows, after_rows):
    """The audio integrity check: the same text, the same words, nothing doubled."""
    def first_text(rows):
        for r in rows:
            if r.get("Texto"):
                return r["Texto"]
        return ""

    lines = [
        "| Audio | palabras antes | palabras despues | omitidas | anadidas | repeticiones | texto identico |",
        "|---|---|---|---|---|---|---|",
    ]
    for clip in sorted({r["Audio"] for r in before_rows} | {r["Audio"] for r in after_rows}):
        tb = first_text([r for r in before_rows if r["Audio"] == clip])
        ta = first_text([r for r in after_rows if r["Audio"] == clip])
        omitted, added = word_diff(tb, ta)
        dups = duplicated_words(ta)
        same = "si" if words(tb) == words(ta) else "no"
        lines.append(
            f"| {clip} | {len(words(tb))} | {len(words(ta))} | {len(omitted)} "
            f"| {len(added)} | {len(dups)} | {same} |"
        )
    return "\n".join(lines)


def table_cancel(rows):
    """The cancellation scenario: how long the engine took to stop."""
    if not rows:
        return "_(sin datos)_"
    cancelled = [r for r in rows if r.get("Cancelado")]
    if not cancelled:
        return "_(ninguna corrida se cancelo)_"
    wait = stats([r["EsperaMs"] for r in cancelled])
    backlog = stats([r.get("RetrasoMs") for r in cancelled])
    lines = [
        "| Metrica | N | Media | Min | p50 | p95 | Maximo |",
        "|---|---|---|---|---|---|---|",
        stat_row("Espera hasta que el motor se detuvo (ms)", wait),
        stat_row("Retraso pendiente al cancelar (ms)", backlog),
    ]
    return "\n".join(lines)


def table_summary(scenarios):
    """One row per scenario: the wait before and after, in one table."""
    lines = [
        "| Escenario | N | p50 antes (ms) | p95 antes (ms) | max antes (ms) "
        "| p50 despues (ms) | p95 despues (ms) | max despues (ms) |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for label, before, after in scenarios:
        sb = stats([r["EsperaMs"] for r in before]) if before else None
        sa = stats([r["EsperaMs"] for r in after]) if after else None
        lines.append(
            "| %s | %s | %s | %s | %s | %s | %s | %s |" % (
                label,
                sb["n"] if sb else 0,
                fmt(sb["p50"]) if sb else "-",
                fmt(sb["p95"]) if sb else "-",
                fmt(sb["max"]) if sb else "-",
                fmt(sa["p50"]) if sa else "-",
                fmt(sa["p95"]) if sa else "-",
                fmt(sa["max"]) if sa else "-",
            )
        )
    return "\n".join(lines)


def table_clip_comparison(control_rows, catchup_rows):
    """Per clip: the wait with the catch-up and without it."""
    def by_clip(rows):
        grouped = {}
        for r in rows:
            grouped.setdefault(r["Audio"], []).append(r)
        return grouped

    gc, gb = by_clip(control_rows), by_clip(catchup_rows)
    lines = [
        "| Audio | Sin la pasada (ms) | Con la pasada (ms) |",
        "|---|---|---|",
    ]
    for clip in sorted(set(gc) | set(gb)):
        c = [r["EsperaMs"] for r in gc.get(clip, [])]
        b = [r["EsperaMs"] for r in gb.get(clip, [])]
        if not c or not b:
            continue
        lines.append(
            "| %s | %s | %s - %s |" % (
                clip,
                fmt(min(c)),
                fmt(min(b)),
                fmt(max(b)),
            )
        )
    return "\n".join(lines)


def table_user_state(path):
    """The verification of the user's own state, as it was read from disk."""
    if not path or not os.path.isfile(path):
        return "_(sin verificacion)_"
    with open(path, encoding="utf-8") as f:
        lines = [line.rstrip() for line in f if line.strip()]
    return "```\n" + "\n".join(lines) + "\n```"


def table_commits(repo):
    """The commits of this stage, taken from git."""
    import subprocess

    out = subprocess.run(
        ["git", "-C", repo, "log", "--oneline", "--no-decorate", "-n", "12"],
        capture_output=True,
        text=True,
    ).stdout.strip()
    if not out:
        return "_(sin commits)_"
    lines = ["| Commit | Descripcion |", "|---|---|"]
    for line in out.splitlines():
        sha, _, message = line.partition(" ")
        lines.append(f"| `{sha}` | {message} |")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--antes", required=True)
    parser.add_argument("--despues", required=True)
    parser.add_argument("--cancelacion", default="")
    parser.add_argument("--moderada-antes", default="")
    parser.add_argument("--moderada-despues", default="")
    parser.add_argument("--alta-antes", default="")
    parser.add_argument("--alta-despues", default="")
    parser.add_argument("--alta-sin-pasada", default="")
    parser.add_argument("--umbral-solo", default="")
    parser.add_argument("--repo", default=r"D:\Codigo\Dixi")
    parser.add_argument("--estado-usuario", default="")
    parser.add_argument("--informe", required=True)
    args = parser.parse_args()

    antes = load_rows(args.antes)
    despues = load_rows(args.despues)
    cancel = load_rows(args.cancelacion) if args.cancelacion and os.path.isdir(args.cancelacion) else []

    def optional(directory):
        return load_rows(directory) if directory and os.path.isdir(directory) else []

    tables = {
        "RESUMEN": table_summary([
            ("Sin carga", antes, despues),
            ("Carga moderada (2 nucleos)",
             optional(args.moderada_antes), optional(args.moderada_despues)),
            ("Carga alta (4 nucleos)",
             optional(args.alta_antes), optional(args.alta_despues)),
        ]),
        "ESCENARIO-SIN-CARGA-ANTES": table_scenario("antes", antes),
        "ESCENARIO-SIN-CARGA-DESPUES": table_scenario("despues", despues),
        "ESCENARIO-CARGA-MODERADA-ANTES": table_scenario("antes", optional(args.moderada_antes)),
        "ESCENARIO-CARGA-MODERADA-DESPUES": table_scenario("despues", optional(args.moderada_despues)),
        "ESCENARIO-CARGA-ALTA-ANTES": table_scenario("antes", optional(args.alta_antes)),
        "ESCENARIO-CARGA-ALTA-DESPUES": table_scenario("despues", optional(args.alta_despues)),
        "ESCENARIO-CARGA-ALTA-SIN-PASADA": table_scenario(
            "sin pasada", optional(args.alta_sin_pasada)
        ),
        "COMPARACION-CLIPS": table_clip_comparison(
            optional(args.alta_sin_pasada), optional(args.alta_despues)
        ),
        "SOLO-UMBRAL": table_scenario("solo umbral", optional(args.umbral_solo)),
        "POR-GRABACION": table_per_audio(antes, despues),
        "RUTAS-DESPUES": table_paths(despues),
        "INTEGRIDAD": table_integrity(antes, despues),
        "CANCELACION": table_cancel(cancel),
        "COMMITS": table_commits(args.repo),
        "ESTADO-USUARIO": table_user_state(args.estado_usuario),
    }

    with open(args.informe, encoding="utf-8") as f:
        report = f.read()

    for name, body in tables.items():
        marker = f"<!--TABLA:{name}-->"
        if marker not in report:
            continue
        # Una tabla sin datos no se escribe: se deja el marcador para que se
        # vea que falta la medicion, en vez de una tabla con guiones.
        if "| - |" in body or not body.strip():
            print(f"  sin datos: {name} (marcador intacto)")
            continue
        report = report.replace(marker, body)

    with open(args.informe, "w", encoding="utf-8", newline="") as f:
        f.write(report)

    print("tablas escritas en", args.informe)
    for name in tables:
        print(" -", name)


if __name__ == "__main__":
    raise SystemExit(main())
