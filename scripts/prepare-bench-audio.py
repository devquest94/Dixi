#!/usr/bin/env python3
"""prepare-bench-audio.py - remuestrea el corpus de referencia al formato que
exige la ruta headless de Dixi.

El corpus de `docs/validation-phase2a/audio-referencia` es audio sintetico
de espeak-ng a 22050 Hz. La transcripcion por fichero solo acepta WAV PCM de
16 kHz, mono, 16 bits, porque ese es el formato que produce la captura del
microfono.

El remuestreo es lineal: para medir **latencia** es suficiente y es
identico antes y despues de cada cambio, que es lo que hace comparable la
medicion. No sirve para medir calidad de reconocimiento.

Uso:
    python scripts/prepare-bench-audio.py <directorio-origen> <directorio-destino>
"""

import array
import os
import sys
import wave

import numpy as np

TARGET_RATE = 16000
TAPS = 101


def _lowpass(samples: np.ndarray, orig_rate: int, target_rate: int) -> np.ndarray:
    """Filtro pasa-bajos de sinc con ventana de Hamming.

    Sin este filtro, el remuestreo lineal deja pasar las frecuencias por
    encima del Nyquist del destino y las pliega sobre la banda audible: el
    audio suena distinto y el modelo reconoce otra cosa. Con el filtro, el
    corpus remuestreado suena igual que el original.
    """
    factor = target_rate / float(orig_rate)
    if factor >= 1.0:
        return samples
    cutoff = 0.5 * factor  # ciclos por muestra, normalizado
    n = np.arange(TAPS) - (TAPS - 1) / 2.0
    h = 2.0 * cutoff * np.sinc(2.0 * cutoff * n)
    h *= np.hamming(TAPS)
    total = h.sum()
    if total:
        h /= total
    return np.convolve(samples, h, mode="same")


def resample(src: str, dst: str) -> None:
    with wave.open(src, "rb") as w:
        channels = w.getnchannels()
        width = w.getsampwidth()
        rate = w.getframerate()
        raw = w.readframes(w.getnframes())

    if width != 2:
        raise SystemExit(f"solo se admite PCM de 16 bits, {src} tiene {width * 8}")

    samples = array.array("h")
    samples.frombytes(raw)
    if sys.byteorder == "big":
        samples.byteswap()

    if channels > 1:
        mono = array.array("h")
        for i in range(0, len(samples) - channels + 1, channels):
            mono.append(sum(samples[i : i + channels]) // channels)
        samples = mono

    data = np.asarray(samples, dtype=np.float64)
    data = _lowpass(data, rate, TARGET_RATE)

    out_len = int(len(data) * TARGET_RATE / float(rate))
    idx = np.linspace(0, len(data) - 1, out_len, endpoint=False)
    out = np.interp(idx, np.arange(len(data)), data)
    pcm = np.clip(np.round(out), -32768, 32767).astype("<i2")

    with wave.open(dst, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(TARGET_RATE)
        w.writeframes(pcm.tobytes())


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2

    src_dir, dst_dir = sys.argv[1], sys.argv[2]
    os.makedirs(dst_dir, exist_ok=True)

    count = 0
    for name in sorted(os.listdir(src_dir)):
        if not name.lower().endswith(".wav"):
            continue
        src = os.path.join(src_dir, name)
        dst = os.path.join(dst_dir, name)
        resample(src, dst)
        with wave.open(dst, "rb") as w:
            secs = w.getnframes() / float(w.getframerate())
        print(f"  {name} -> {TARGET_RATE} Hz, {secs:.2f}s")
        count += 1

    print(f"{count} audios preparados en {dst_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
