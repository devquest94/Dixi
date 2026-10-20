/**
 * patch-settings.mjs — modifica el store de ajustes de Dixi de forma segura.
 *
 * Por que en Node y no en PowerShell:
 *  - `Out-File -Encoding utf8` en Windows PowerShell 5.1 anade un BOM, y
 *    serde_json NO lo acepta: la app no lee el store, cae a los valores por
 *    defecto y los vuelve a guardar (los cambios se pierden en silencio).
 *  - `Get-Content` sin -Encoding UTF8 decodifica UTF-8 como ANSI y
 *    `ConvertTo-Json` vuelve a codificar: las cadenas no ASCII (el prompt de
 *    post-procesado lleva flechas U+2192) se deforman.
 *  Node lee y escribe UTF-8 sin BOM, y hace el round-trip sin perdida.
 *
 * Uso:  node scripts/patch-settings.mjs <store.json> [--restore] [clave=valor ...]
 */
import { readFileSync, writeFileSync, existsSync, copyFileSync } from "node:fs";
import { resolve } from "node:path";

const [, , storeArg, ...rest] = process.argv;
if (!storeArg) {
  console.error("uso: node patch-settings.mjs <store.json> [--restore] [clave=valor ...]");
  process.exit(2);
}
const store = resolve(storeArg);
const backup = `${store}.pre-validation.bak`;

if (rest.includes("--restore")) {
  if (!existsSync(backup)) {
    console.error(`no hay copia de seguridad: ${backup}`);
    process.exit(1);
  }
  copyFileSync(backup, store);
  console.log(`[patch] ajustes originales restaurados desde ${backup}`);
  process.exit(0);
}

if (!existsSync(store)) {
  console.error(`no existe el store: ${store}`);
  process.exit(1);
}
if (!existsSync(backup)) {
  copyFileSync(store, backup);
  console.log(`[patch] copia de seguridad creada: ${backup}`);
}

const doc = JSON.parse(readFileSync(store, "utf8"));
const s = doc.settings ?? (doc.settings = {});

/**
 * Repara texto deformado por una decodificacion cp1252 (mojibake).
 *
 * El prompt de post-procesado lleva flechas U+2192 y rayas U+2014. Al pasar
 * por una decodificacion cp1252 y volver a codificar como UTF-8 quedan como
 * "Ã¢â€â€™". Se deshace decodificando cp1252 -> bytes -> UTF-8, hasta dos
 * veces (el fichero puede estar deformado una o dos veces), con guarda: si el
 * resultado trae U+FFFD, no se aplica.
 *
 * Node solo trae latin1, y en latin1 NO caben U+20AC (euro) ni U+2122 (TM),
 * que son justo los caracteres que aparecen en la deformacion. Por eso se
 * implementa la tabla cp1252 explicita.
 */
const CP1252_HIGH = {
  0x20ac: 0x80, 0x81: 0x81, 0x201a: 0x82, 0x0192: 0x83, 0x201e: 0x84,
  0x2026: 0x85, 0x2020: 0x86, 0x2021: 0x87, 0x02c6: 0x88, 0x2030: 0x89,
  0x0160: 0x8a, 0x2039: 0x8b, 0x0152: 0x8c, 0x008d: 0x8d, 0x017d: 0x8e,
  0x008f: 0x8f, 0x0090: 0x90, 0x2018: 0x91, 0x2019: 0x92, 0x201c: 0x93,
  0x201d: 0x94, 0x2022: 0x95, 0x2013: 0x96, 0x2014: 0x97, 0x02dc: 0x98,
  0x2122: 0x99, 0x0161: 0x9a, 0x203a: 0x9b, 0x0153: 0x9c, 0x009d: 0x9d,
  0x017e: 0x9e, 0x0178: 0x9f,
};

function cp1252ToBytes(text) {
  const bytes = [];
  for (const ch of text) {
    const cp = ch.codePointAt(0);
    if (cp <= 0xff) bytes.push(cp);
    else if (CP1252_HIGH[cp] !== undefined) bytes.push(CP1252_HIGH[cp]);
    else return null; // no representable en cp1252: no seguimos
  }
  return Buffer.from(bytes);
}

function repairMojibake(text) {
  if (typeof text !== "string") return text;
  if (!/[ÃÂ]/.test(text)) return text;
  let out = text;
  for (let i = 0; i < 2; i++) {
    const bytes = cp1252ToBytes(out);
    if (!bytes) break;
    const candidate = bytes.toString("utf8");
    if (candidate.includes("\uFFFD") || candidate === out) break;
    out = candidate;
  }
  return out;
}

for (const prompt of s.post_process_prompts ?? []) {
  if (typeof prompt?.prompt === "string") {
    const fixed = repairMojibake(prompt.prompt);
    if (fixed !== prompt.prompt) {
      console.log(`  post_process_prompts[${prompt.id}]: prompt reparado (mojibake)`);
      prompt.prompt = fixed;
    }
  }
}

const changes = [
  ["selected_model", "handy-computer/whisper-base-gguf/whisper-base-Q8_0.gguf",
    "modelo local medido y verificado (sha256)"],
  ["selected_language", "es",
    "evita la deteccion automatica (fallo medido en canary-180m-flash)"],
  ["onboarding_completed", true,
    "el modelo ya esta descargado; no hace falta el asistente"],
  ["reliable_paste", true,
    "activa la restauracion del portapapeles"],
  ["clipboard_handling", "dont_modify",
    "restaurar la instantanea previa del portapapeles"],
  ["model_unload_timeout", "never",
    "latencia estable: el modelo no se descarga entre pruebas"],
  ["shortcut_activation", "push_to_talk",
    "push-to-talk puro: graba mientras se mantiene pulsado"],
  ["debug_mode", false,
    "log legible durante la validacion"],
  ["log_level", "debug",
    "log legible durante la validacion"],
];

// Ajustes pasados por linea de comandos (clave=valor, JSON opcional).
for (const arg of rest) {
  if (!arg.includes("=")) continue;
  const i = arg.indexOf("=");
  const key = arg.slice(0, i);
  const raw = arg.slice(i + 1);
  let value = raw;
  try { value = JSON.parse(raw); } catch { /* cadena literal */ }
  changes.push([key, value, "indicado en la linea de comandos"]);
}

console.log("[patch] ajustes aplicados:");
for (const [key, value, why] of changes) {
  const before = s[key];
  s[key] = value;
  console.log(
    `  ${key.padEnd(22)} ${JSON.stringify(before)} -> ${JSON.stringify(value)}  # ${why}`,
  );
}

writeFileSync(store, JSON.stringify(doc, null, 2), "utf8");
console.log(`[patch] store escrito en UTF-8 sin BOM: ${store}`);
