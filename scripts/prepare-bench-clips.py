#!/usr/bin/env python3
"""prepare-bench-clips.py - arma clips de duracion controlada para el banco de
latencia y de calidad.

Los clips se construyen con las frases del corpus de referencia, separadas por
silencios reales (0,40 s). El silencio importa: es lo que hace que el cortador
de pausas del motor tenga pausas donde cortar, que es justamente lo que decide
si el dictado se decodifica mientras el usuario habla o despues de soltar la
tecla.

Cada clip lleva su texto esperado en un manifiesto, para que el WER y el CER
se calculen contra una referencia escrita y no contra una suposicion.

Los clips por encima de 45 s (el umbral actual del cortador) son los que
muestran si el chunking se activa; los de 3, 10 y 20 s son los que miden la
latencia que el usuario percibe en un dictado corriente.

Uso:
    python scripts/prepare-bench-clips.py <directorio-origen> <directorio-destino>
"""

import importlib.util
import json
import os
import sys
import wave

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
TARGET_RATE = 16000
GAP_SECONDS = 0.40

# El re-muestreo es el mismo que usa prepare-bench-audio.py: filtro pasa-bajos
# de sinc antes de interpolar. Sin el filtro el modelo reconoce otra cosa.
_spec = importlib.util.spec_from_file_location(
    "prepare_bench_audio", os.path.join(HERE, "prepare-bench-audio.py")
)
_prepare = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_prepare)


# El texto esperado de cada frase, tal como esta en frases.txt.
PHRASES = {
    "F1": "Hola, esta es una prueba de dictado en español.",
    "F2": "Fijate si podés revisar los logs del server antes de hacer el deploy.",
    "F3": "El build me tiró un error, está medio quilombo, lo voy a chequear.",
    "F4": "Hacé un commit, pusheá la branch y corré el deploy en Docker.",
    "F5": (
        "Quería preguntarte si el backup del servidor de producción quedó bien, "
        "porque ayer el log showed un timeout."
    ),
    "F6": "Atención: el deploy falla, no es un problema del código.",
    "F7": "El servidor tenía cuarenta y cinco por ciento de la memoria usada.",
    "F8": "El endpoint no responde, me parece que hay un timeout en el API gateway.",
    "F9": "Che, ¿me pasás el archivo del backup? Lo necesito para el merge.",
    "F10": "Revisá el stack trace del error, así vemos dónde se rompe el endpoint.",
}

# clip -> (frases, duracion maxima en segundos o None)
# None = la duracion la dan las frases; un numero recorta el clip.
CLIPS = {
    # 3 s: dictado muy corto. El recorte corta una palabra, asi que este clip
    # mide latencia, no precision.
    "C03": (["F1"], 3.0),
    # 4 s: una frase entera, la unidad minima de dictado.
    "C04": (["F1"], None),
    # 10 s: dos frases con una pausa entre ellas.
    "C10": (["F1", "F2"], None),
    # 20 s: cuatro frases, el largo tipico de un dictado.
    "C20": (["F1", "F2", "F3", "F4"], None),
    # 35 s: seis frases. Sigue por debajo del umbral de 45 s del cortador.
    "C35": (["F1", "F2", "F3", "F4", "F5", "F6"], None),
    # 49 s: las diez frases. Supera el umbral: aqui el cortador de pausas
    # actua y la decodificacion durante la grabacion se puede observar.
    "MIX49": (["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10"], None),
}


def read_16k(path: str) -> np.ndarray:
    """Remuestrea un WAV del corpus al formato que acepta la ruta headless."""
    import tempfile

    out = os.path.join(tempfile.gettempdir(), "_bench_clip_tmp.wav")
    _prepare.resample(path, out)
    with wave.open(out, "rb") as w:
        raw = w.readframes(w.getnframes())
    os.remove(out)
    return np.frombuffer(raw, dtype="<i2").astype(np.float64)


def write_wav(path: str, pcm: np.ndarray) -> None:
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(TARGET_RATE)
        w.writeframes(pcm.astype("<i2").tobytes())


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2

    src_dir, dst_dir = sys.argv[1], sys.argv[2]
    os.makedirs(dst_dir, exist_ok=True)

    gap = np.zeros(int(GAP_SECONDS * TARGET_RATE), dtype=np.float64)
    manifest = []

    for name in sorted(CLIPS):
        phrases, max_secs = CLIPS[name]
        parts = []
        for i, phrase in enumerate(phrases):
            if i:
                parts.append(gap)
            parts.append(read_16k(os.path.join(src_dir, phrase + ".wav")))
        pcm = np.concatenate(parts)

        truncated = False
        if max_secs is not None and len(pcm) > int(max_secs * TARGET_RATE):
            pcm = pcm[: int(max_secs * TARGET_RATE)]
            truncated = True

        out_path = os.path.join(dst_dir, name + ".wav")
        write_wav(out_path, pcm)

        secs = len(pcm) / float(TARGET_RATE)
        expected = " ".join(PHRASES[p] for p in phrases)
        manifest.append(
            {
                "clip": name,
                "archivo": name + ".wav",
                "segundos": round(secs, 2),
                "frases": phrases,
                "texto_esperado": expected,
                "recortado": truncated,
                "nota": "recortado a mitad de palabra: mide latencia, no precision"
                if truncated
                else "",
            }
        )
        print(f"  {name}.wav  {secs:6.2f}s  ({len(phrases)} frases)")

    with open(os.path.join(dst_dir, "manifiesto.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    print(f"\n{len(manifest)} clips preparados en {dst_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
