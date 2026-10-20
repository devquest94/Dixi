import { commands } from "@/bindings";

/**
 * Typed façade over the personal-dictionary Tauri commands.
 *
 * The commands come from the generated bindings, which are regenerated from
 * the Rust command registry with `dixi.exe --export-bindings`, so this file
 * only gives them names and keeps the call sites short.
 */

/** Lists the entries of a scope. Omitting the scope uses the global scope. */
export const listDictionaryEntries = (scope?: string) =>
  commands.listDictionaryEntries(scope ?? null);

/** Adds an entry and returns the stored version. */
export const addDictionaryEntry = (
  entry: Parameters<typeof commands.addDictionaryEntry>[0],
) => commands.addDictionaryEntry(entry);

/** Updates the writable fields of an entry. */
export const updateDictionaryEntry = (
  id: number,
  entry: Parameters<typeof commands.updateDictionaryEntry>[1],
) => commands.updateDictionaryEntry(id, entry);

/** Deletes an entry. */
export const deleteDictionaryEntry = (id: number) =>
  commands.deleteDictionaryEntry(id);

/** Enables or disables an entry. */
export const setDictionaryEntryEnabled = (id: number, enabled: boolean) =>
  commands.setDictionaryEntryEnabled(id, enabled);

/** Adds several entries at once, skipping the ones that already exist. */
export const importDictionaryEntries = (
  entries: Parameters<typeof commands.importDictionaryEntries>[0],
) => commands.importDictionaryEntries(entries);

/**
 * Writes the dictionary to the JSON file the user chose. The backend owns the
 * file format and the filesystem access, so the webview never touches the
 * file itself. Returns how many entries were written.
 *
 * An existing file is only replaced when `overwrite` is set: the backend
 * refuses otherwise, so the caller has to ask the user first.
 */
export const exportDictionaryEntries = (path: string, overwrite: boolean) =>
  commands.exportDictionaryEntries(path, overwrite);

/**
 * Adds the entries contained in the dictionary file the user chose. The
 * backend skips the entries that already exist and returns how many entries
 * were actually added.
 */
export const importDictionaryEntriesFromFile = (path: string) =>
  commands.importDictionaryEntriesFromFile(path);

/** The vocabulary the ASR engine would receive for the given scope. */
export const getDictionaryVocabulary = (scope?: string) =>
  commands.getDictionaryVocabulary(scope ?? null);

/** Runs the substitution rules over a piece of text. */
export const applyDictionarySubstitutions = (text: string, scope?: string) =>
  commands.applyDictionarySubstitutions(text, scope ?? null);
