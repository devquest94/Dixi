/**
 * add-i18n-key.mjs — add one translation key to every locale file.
 *
 * Usage:
 *   node scripts/add-i18n-key.mjs <key-path> <translations.json>
 *
 * `<key-path>` is a dotted path inside the `settings` object, for example
 * `settings.dictionary.transfer.overwriteTitle`.
 *
 * `<translations.json>` maps a locale code to the translated string. A
 * locale that is missing from the map is reported and left untouched, so a
 * missing translation can never pass unnoticed.
 *
 * The files are written back with the same line ending they had, so the
 * diff only shows the added key.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [, , keyPath, translationsFile] = process.argv;

if (!keyPath || !translationsFile) {
  console.error(
    "usage: node scripts/add-i18n-key.mjs <key-path> <translations.json>",
  );
  process.exit(2);
}

const localesDir = join("src", "i18n", "locales");
const translations = JSON.parse(readFileSync(translationsFile, "utf8"));

const path = keyPath.split(".");
if (path.length < 2 || path.some((step) => step.length === 0)) {
  console.error("the key path must have at least two segments, e.g. 'common.downloading'");
  process.exit(2);
}
const key = path.pop();

const missing = [];
const added = [];

for (const locale of Object.keys(translations)) {
  const file = join(localesDir, locale, "translation.json");
  const raw = readFileSync(file, "utf8");
  const newline = raw.includes("\r\n") ? "\r\n" : "\n";

  const document = JSON.parse(raw);

  let target = document;
  for (const step of path) {
    if (typeof target[step] !== "object" || target[step] === null) {
      target[step] = {};
    }
    target = target[step];
  }

  if (target[key] !== undefined) {
    console.log(`${locale}: already present, skipped`);
    continue;
  }

  target[key] = translations[locale];

  // The files are indented with two spaces and end with a newline.
  writeFileSync(file, JSON.stringify(document, null, 2) + newline, "utf8");
  added.push(locale);
}

if (missing.length > 0) {
  console.error(`missing translations for: ${missing.join(", ")}`);
  process.exit(1);
}

console.log(`added '${keyPath}' to ${added.length} locale(s): ${added.join(", ")}`);
