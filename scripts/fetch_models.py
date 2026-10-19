"""Descarga modelos ASR de Dixi con verificacion de procedencia.

Procedencia: repositorio Hugging Face + revision fijada + sha256, todo taken
del catalogo de Handy (catalog.json). La verificacion sha256 se hace SIEMPRE
sobre el fichero descargado: un hash que no coincide descarta el fichero.

Uso:  python fetch_models.py <catalog.json> <destino> [model_id ...]
"""
import hashlib
import json
import os
import sys
import urllib.request

CATALOG = sys.argv[1]
DEST = sys.argv[2]
WANTED = sys.argv[3:]

with open(CATALOG, "r", encoding="utf-8") as fh:
    catalog = json.load(fh)

os.makedirs(DEST, exist_ok=True)


def sha256_of(path, chunk=1 << 20):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while True:
            block = fh.read(chunk)
            if not block:
                break
            h.update(block)
    return h.hexdigest()


def download(url, target):
    tmp = target + ".part"
    req = urllib.request.Request(url, headers={"User-Agent": "Dixi/0.1"})
    with urllib.request.urlopen(req, timeout=120) as resp, open(tmp, "wb") as out:
        total = int(resp.headers.get("Content-Length") or 0)
        done = 0
        while True:
            block = resp.read(1 << 20)
            if not block:
                break
            out.write(block)
            done += len(block)
            pct = (done * 100 // total) if total else 0
            sys.stdout.write(f"\r  {done / 1e6:8.1f} MB  {pct:3d}%")
            sys.stdout.flush()
    print()
    os.replace(tmp, target)


selected = catalog["models"]
if WANTED:
    selected = [m for m in catalog["models"] if m["id"] in WANTED]

for model in selected:
    mid = model["id"]
    slug = model["slug"]
    quant = model.get("default_quant", "Q8_0")
    entry = next(
        (f for f in model["files"] if f["quant"] == quant), model["files"][0]
    )
    name = entry["filename"]
    out_path = os.path.join(DEST, name)

    if os.path.exists(out_path) and sha256_of(out_path) == entry["sha256"]:
        print(f"[skip] {name} ya presente y verificado")
        continue

    revision = model["revision"]
    url = f"https://huggingface.co/{mid}/resolve/{revision}/{name}"
    print(f"[get ] {mid}  ({name}, {entry['size_bytes'] / 1e6:.1f} MB, "
          f"licencia={model['license']}, base={model['base_model']})")
    download(url, out_path)

    actual = sha256_of(out_path)
    if actual != entry["sha256"]:
        os.remove(out_path)
        print(f"[FAIL] sha256Mismatch {name}\n"
              f"       esperado {entry['sha256']}\n"
              f"       obtenido {actual}")
        sys.exit(2)
    print(f"[ok  ] {name}  sha256 verificado")

print("modelos listos en", DEST)
