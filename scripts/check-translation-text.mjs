/**
 * check-translation-text.mjs — control de calidad de las traducciones.
 *
 * Comprueba dos cosas sobre los 26 ficheros de `src/i18n/locales`:
 *
 * 1. Que todas las claves que usan los componentes de la fase 2C
 *    (perfiles, fragmentos, transforms e historial de texto) existan en
 *    todos los idiomas, con texto no vacio. Una clave que falta en un
 *    idioma se ve en pantalla como la propia clave.
 *
 * 2. Que ninguna cadena haya sufrido dano de codificacion (caracteres de
 *    reemplazo, mojibake) ni se haya quedado con palabras latinas pegadas
 *    dentro de un texto escrito en otro alfabeto. Eso es lo que pasa si
 *    una seccion se copia de otro idioma y se queda a medio traducir.
 *
 * Uso: node scripts/check-translation-text.mjs
 * Devuelve 1 si encuentra algun problema.
 */

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const repo = process.cwd();
const localesDir = join(repo, "src", "i18n", "locales");

/** Los codigos de fallo que el backend puede devolver, en camelCase. */
const SELECTION_ERROR_KEYS = [
  "nothingSelected",
  "passwordField",
  "unsupportedControl",
  "clipboardNotPreservable",
  "clipboard",
  "selectionChanged",
  "destinationUnavailable",
  "destinationGone",
  "write",
];

/** Componentes de la fase 2C y las claves dinamicas que usan. */
const COMPONENTS = [
  {
    file: "src/components/settings/profiles/ProfilesSettings.tsx",
    templates: {
      "settings.profiles.cleanup.${level}": ["none", "light", "medium"],
      "settings.profiles.origin.${origin}": [
        "manualOverride",
        "appOverride",
        "detectedProfile",
        "global",
      ],
      "settings.profiles.rule.${rule}": [
        "capitalizeSentences",
        "ensureFinalPeriod",
        "collapseSpaces",
        "spaceBeforePunctuation",
      ],
    },
  },
  {
    file: "src/components/settings/snippets/SnippetsSettings.tsx",
    templates: {},
  },
  {
    file: "src/components/settings/transforms/TransformsSettings.tsx",
    templates: {},
  },
  {
    file: "src/components/settings/textHistory/TextHistorySettings.tsx",
    templates: {
      "settings.textHistory.kind.${kind}": ["transform", "snippet", "cleanup"],
      "settings.selection.errors.${selectionErrorKey(failure.code)}":
        SELECTION_ERROR_KEYS,
    },
  },
];

/** Secuencias tipicas de UTF-8 leido como Latin-1. */
const MOJIBAKE = [
  "Ã¡", "Ã©", "Ã­", "Ã³", "Ãº", "Ã±", "Ã¼", "Ã¤", "Ã¶", "Ã¿",
  "Â¡", "Â«", "Â»", "â€œ", "â€", "â€™", "ï»¿",
];

/** Alfabetos que no usan el latin. */
const NON_LATIN = {
  ar: /[\u0600-\u06FF]/,
  he: /[\u0590-\u05FF]/,
  hi: /[\u0900-\u097F]/,
  ne: /[\u0900-\u097F]/,
  ja: /[\u3040-\u30FF\u4E00-\u9FFF]/,
  ko: /[\uAC00-\uD7AF]/,
  zh: /[\u4E00-\u9FFF]/,
  "zh-TW": /[\u4E00-\u9FFF]/,
  bg: /[\u0400-\u04FF]/,
};

/** Terminos que pueden quedar en latin sin que sea un error. */
const ALLOWED_LATIN = new Set([
  // Producto y organizaciones
  "dixi", "github", "releases", "hugging", "face", "ggml", "georgi",
  "gerganov", "openai", "anthropic", "apple", "silicon", "tahoe",
  "whisper", "silero", "earshot", "bluetooth", "beta", "data",
  // Sistemas y tecnologias
  "linux", "macos", "windows", "json", "url", "api", "gpu", "cpu", "vad",
  "metal", "cuda", "webview", "ai", "chat", "completions", "intelligence",
  "mac", "endpoint", "clamshell", "transcribe", "cpp", "models",
  // Teclas y atajos
  "ctrl", "shift", "enter", "cmd", "super", "insert", "option", "space",
  "pid", "d", "e", "i",
  // Codigos de idioma (los ejemplos de los campos de idioma)
  "ar", "bg", "ca", "cs", "da", "de", "en", "es", "fr", "he", "hi", "id",
  "it", "ja", "ko", "ne", "nl", "pl", "pt", "ru", "sv", "tr", "uk", "vi",
  "zh", "tw", "sh",
  // Nombres de los menus del sistema que se citan en los mensajes
  "settings", "privacy", "security", "microphone", "system", "dix",
  // Marcadores y palabras de la interfaz que si van en latin
  "code", "output", "name", "count", "total", "size", "speed", "percentage",
  "error", "term", "word", "model", "modelname", "script", "modifier",
  "auto", "secure", "input", "disable", "updater", "progress", "version",
  "fecha", "hora",
]);

/** Resuelve una clave con puntos dentro del documento. */
function resolve(document, key) {
  let node = document;
  for (const step of key.split(".")) {
    if (typeof node !== "object" || node === null) return undefined;
    node = node[step];
  }
  return node;
}

/** Todas las claves que usan los componentes, con las plantillas expandidas. */
function collectKeys() {
  const keys = new Set();
  for (const component of COMPONENTS) {
    const source = readFileSync(join(repo, component.file), "utf8");
    for (const match of source.matchAll(/\bt\(\s*"([^"]+)"/g)) {
      keys.add(match[1]);
    }
    for (const match of source.matchAll(/\bt\(\s*`([^`]+)`/g)) {
      const template = match[1];
      // El codigo de los fallos de la seleccion se busca por prefijo: la
      // expresion que lo convierte a camelCase cambia segun el sitio.
      if (template.startsWith("settings.selection.errors.${")) {
        for (const value of SELECTION_ERROR_KEYS) {
          keys.add(template.replace(/\$\{[^}]*\}/, value));
        }
        continue;
      }
      const values = component.templates[template];
      if (!values) {
        throw new Error(
          `plantilla sin expansion conocida en ${component.file}: ${template}`,
        );
      }
      const name = template.slice(template.indexOf("${") + 2, template.indexOf("}"));
      for (const value of values) {
        keys.add(template.replace(`\${${name}}`, value));
      }
    }
  }
  return [...keys].sort();
}

const locales = readdirSync(localesDir);
const documents = new Map();
for (const locale of locales) {
  const file = join(localesDir, locale, "translation.json");
  documents.set(locale, {
    raw: readFileSync(file, "utf8"),
    document: JSON.parse(readFileSync(file, "utf8")),
  });
}

const problems = [];

// --- 1. Toda clave usada existe en todos los idiomas ------------------------

const keys = collectKeys();
for (const key of keys) {
  for (const locale of locales) {
    const value = resolve(documents.get(locale).document, key);
    if (typeof value !== "string" || value.trim().length === 0) {
      problems.push(`${locale}: falta o esta vacia la clave ${key}`);
    }
  }
}

// --- 2. Codificacion y mezclas de alfabetos ----------------------------------

let strings = 0;
for (const locale of locales) {
  const { raw, document } = documents.get(locale);

  if (raw.includes("\uFFFD")) {
    problems.push(`${locale}: contiene caracteres de reemplazo (U+FFFD)`);
  }
  for (const sequence of MOJIBAKE) {
    if (raw.includes(sequence)) {
      problems.push(`${locale}: mojibake ${JSON.stringify(sequence)}`);
    }
  }

  const nonLatin = NON_LATIN[locale];
  if (!nonLatin) continue;

  const walk = (node, path) => {
    if (typeof node === "string") {
      strings += 1;
      if (!nonLatin.test(node)) return;
      // Los marcadores de interpolacion no son palabras.
      const clean = node.replace(/\{\{[^}]*\}\}/g, " ");
      const latin = (clean.match(/[A-Za-zÀ-ÿ]{2,}/g) ?? []).filter(
        (word) => !ALLOWED_LATIN.has(word.toLowerCase()),
      );
      if (latin.length > 0) {
        problems.push(`${locale}: ${path} -> ${JSON.stringify(latin.join(", "))}`);
      }
      return;
    }
    if (typeof node === "object" && node !== null) {
      for (const key of Object.keys(node)) {
        walk(node[key], path ? `${path}.${key}` : key);
      }
    }
  };

  walk(document, "");
}

// --- Resultado ---------------------------------------------------------------

console.log(
  `locales: ${locales.length} | claves de la fase 2C: ${keys.length} | cadenas revisadas: ${strings}`,
);
if (problems.length === 0) {
  console.log("traducciones: sin problemas");
  process.exit(0);
}

console.log(`problemas: ${problems.length}`);
for (const problem of problems) {
  console.log(`  ${problem}`);
}
process.exit(1);
