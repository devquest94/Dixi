"""Prepara las muestras de audio sintetico para las pruebas de Dixi.

Convierte el WAV de espeak-ng (22 050 Hz mono 16-bit) al formato que exige la
ruta de transaccion por lotes de Handy/Dixi: 16 000 Hz, mono, 16-bit PCM.

Remuestreo por interpolacion con kernel sinc ventana Kaiser (paso bajo real:
22050 -> 16000 tiene folding si se usara interpolacion lineal simple).

No usa audio real de terceros: todo el audio es sintetico y generado localmente.
"""
import os
import sys
import wave

import numpy as np

SRC_RATE = 22050
DST_RATE = 16000


def resample_kaiser(x: np.ndarray, src_rate: int, dst_rate: int) -> np.ndarray:
    """Remuestreo con sinc ventana Kaiser (factor 32, beta 8.6).

    Se procesa en bloques para no reservar matrices grandes: la maquina puede
    estar bajo presion de memoria durante la compilacion en paralelo.
    """
    if src_rate == dst_rate:
        return x
    ratio = dst_rate / src_rate
    n_out = int(round(len(x) * ratio))
    n_in = len(x)

    half = 32
    offsets = np.arange(-half + 1, half + 1)
    w = np.kaiser(2 * half, 8.6)

    out = np.empty(n_out, dtype=np.float64)
    block = 4096
    for start in range(0, n_out, block):
        stop = min(start + block, n_out)
        t = np.arange(start, stop) / ratio
        left = np.floor(t).astype(np.int64)
        idx = left[:, None] + offsets[None, :]
        frac = (t - left)[:, None] - offsets[None, :] + (half - 1)
        with np.errstate(invalid="ignore", divide="ignore"):
            u = frac
            sinc = np.where(u == 0, 1.0, np.sin(np.pi * u) / (np.pi * u))
            kernel = sinc * w[None, :]
            kernel /= kernel.sum(axis=1, keepdims=True)
            valid = (idx >= 0) & (idx < n_in)
            vals = np.where(valid, x[np.clip(idx, 0, n_in - 1)], 0.0)
            out[start:stop] = (vals * kernel).sum(axis=1)
    return out.astype(np.float64)


def read_wav(path):
    with wave.open(path, "rb") as w:
        assert w.getsampwidth() == 2, "se esperaba PCM de 16 bits"
        n_ch = w.getnchannels()
        rate = w.getframerate()
        data = w.readframes(w.getnframes())
    x = np.frombuffer(data, dtype="<i2").astype(np.float64) / 32768.0
    if n_ch > 1:
        x = x.reshape(-1, n_ch).mean(axis=1)
    return x, rate


def write_wav16(path, x, rate):
    x = np.clip(x, -1.0, 1.0)
    pcm = (x * 32767.0).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm.tobytes())


def main():
    src_dir, dst_dir = sys.argv[1], sys.argv[2]
    os.makedirs(dst_dir, exist_ok=True)

    for name in sorted(os.listdir(src_dir)):
        if not name.endswith(".wav"):
            continue
        src = os.path.join(src_dir, name)
        x, rate = read_wav(src)
        y = resample_kaiser(x, rate, DST_RATE)

        # 200 ms de silencio al inicio y al final: imita una grabacion real y
        # comprueba que el pipeline tolera margenes de silencio.
        pad = int(0.2 * DST_RATE)
        y = np.concatenate([np.zeros(pad), y, np.zeros(pad)])

        # Normalizacion de pico a 0.95: espeak-ng puede saturar y el recorte
        # introduce armónicos que falsean la medida de precision.
        peak = float(np.max(np.abs(y))) or 1.0
        if peak > 0.95:
            y = y * (0.95 / peak)

        dst = os.path.join(dst_dir, name.replace("_prueba", "_16k"))
        write_wav16(dst, y, DST_RATE)
        print(f"{name}  {rate} Hz -> {dst}  {DST_RATE} Hz  "
              f"{len(y) / DST_RATE:.2f} s  pico={np.max(np.abs(y)):.3f}")


if __name__ == "__main__":
    main()
