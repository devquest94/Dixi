#!/bin/bash
# Genera audio de referencia (es-419, español latinoamericano) para las frases
# de prueba de la validación de Dixi.
#
# El audio es 100 % sintético y local: no se usa ninguna grabación real ni
# material de terceros. Sirve como referencia de contenido (qué debería
# transcribirse), no como medida de la calidad con voz humana.
#
# Uso (desde WSL):
#   wsl -d Ubuntu -u root -- bash /mnt/d/Codigo/Dixi/scripts/make-validation-audio.sh

set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SRC="$ROOT/docs/validation-phase2a/frases.txt"
DST="$ROOT/docs/validation-phase2a/audio-referencia"

mkdir -p "$DST"

command -v espeak-ng >/dev/null 2>&1 || {
    echo "espeak-ng no esta instalado en WSL." >&2
    echo "  wsl -d Ubuntu -u root -- apt-get install -y espeak-ng" >&2
    exit 1
}

# es-419 = Spanish (Latin America), la variante mas cercana al rioplatense
# en la taxonomia de espeak-ng.
VOICE="${VOICE:-es-419}"
SPEED="${SPEED:-145}"

n=0
while IFS='|' read -r id texto; do
    [ -z "$id" ] && continue
    case "$id" in \#*) continue ;; esac
    espeak-ng -v "$VOICE" -s "$SPEED" -w "$DST/$id.wav" "$texto"
    n=$((n + 1))
    echo "  $id -> $DST/$id.wav"
done < "$SRC"

echo "$n frases generadas en $DST (voz=$VOICE velocidad=$SPEED)"
