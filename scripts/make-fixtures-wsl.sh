#!/bin/bash
set -e
mkdir -p /mnt/d/Codigo/_work/fixtures
cd /mnt/d/Codigo/_work/fixtures

espeak-ng -v es -s 150 -w es_basico.wav \
  "Hola, esta es una prueba de dictado en espanol para Dixi."

espeak-ng -v es -s 150 -w es_tecnico.wav \
  "Voy a hacer commit del codigo y revisar los logs del servidor de produccion."

espeak-ng -v en -s 150 -w en_basico.wav \
  "Hello, this is a dictation test for Dixi."

rm -f es_prueba.wav en_prueba.wav
ls -l
