/**
 * split-diff.mjs — divide a file's working-tree diff into hunks and apply a
 * chosen subset of them.
 *
 * Git's interactive `add -p` is not available in an automated run, so this
 * script does the same job non-interactively:
 *
 *   node scripts/split-diff.mjs <file> <patch-out> <keep|drop> <predicate>
 *
 * `predicate` is a regular expression tested against each hunk's text.
 *   keep: hunks matching the predicate are written to <patch-out>
 *   drop: hunks matching the predicate are removed from <patch-out>
 *
 * The script never touches the working tree; it only rewrites a patch file.
 * Apply the result with `git apply` after restoring the file.
 */

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const [, , file, patchOut, mode, predicateSource] = process.argv;

if (!file || !patchOut || !mode || !predicateSource) {
  console.error(
    "usage: node scripts/split-diff.mjs <file> <patch-out> <keep|drop> <regex>",
  );
  process.exit(2);
}

const predicate = new RegExp(predicateSource, "m");

const diff = execFileSync("git", ["diff", "-U3", "--", file], {
  encoding: "utf8",
});

if (!diff.trim()) {
  console.error(`no diff for ${file}`);
  process.exit(1);
}

const lines = diff.split("\n");
const header = [];
let index = 0;

// The header is everything before the first hunk.
while (index < lines.length && !lines[index].startsWith("@@")) {
  header.push(lines[index]);
  index += 1;
}

const hunks = [];
let current = null;
for (; index < lines.length; index += 1) {
  const line = lines[index];
  if (line.startsWith("@@")) {
    if (current) hunks.push(current);
    current = [line];
  } else if (current) {
    current.push(line);
  }
}
if (current) hunks.push(current);

const kept = hunks.filter((hunk) =>
  mode === "keep" ? predicate.test(hunk.join("\n")) : !predicate.test(hunk.join("\n")),
);

if (kept.length === 0) {
  console.error(`no hunk matched (${mode} /${predicateSource}/)`);
  process.exit(1);
}

const out = [...header];
for (const hunk of kept) out.push(...hunk);
// A patch must end with a newline or `git apply` rejects it as corrupt.
writeFileSync(patchOut, out.join("\n").replace(/\n*$/, "\n"), "utf8");

console.log(
  `${file}: ${hunks.length} hunk(s) total, ${kept.length} kept -> ${patchOut}`,
);
