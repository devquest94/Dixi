/**
 * Domain types for the personal dictionary.
 *
 * They mirror the shapes the Rust backend exposes through the Tauri commands.
 * They are declared here so the UI can be built before `src/bindings.ts` is
 * regenerated with the dictionary commands; the shapes are identical, so the
 * generated types can replace them without touching the components.
 */

export type DictionaryEntryKind = "term" | "substitution";

export type DictionaryEntry = {
  id: number;
  term: string;
  replacement: string;
  kind: DictionaryEntryKind;
  enabled: boolean;
  scope: string;
  case_sensitive: boolean;
  whole_word: boolean;
  hit_count: number;
  created_at: number;
  updated_at: number;
  priority: number;
  language: string | null;
};

export type DictionaryEntryInput = {
  term: string;
  replacement: string;
  kind: DictionaryEntryKind;
  enabled?: boolean;
  scope?: string;
  case_sensitive?: boolean;
  whole_word?: boolean;
  priority?: number;
  language?: string | null;
};

/** Scope used when the backend has to fall back to a default. */
export const DICTIONARY_GLOBAL_SCOPE = "global";

/** Controlled state shared by the create form and the edit dialog. */
export interface DictionaryFormState {
  term: string;
  replacement: string;
  kind: DictionaryEntryKind;
  language: string;
  wholeWord: boolean;
  caseSensitive: boolean;
  priority: number;
}

export const EMPTY_DICTIONARY_FORM: DictionaryFormState = {
  term: "",
  replacement: "",
  kind: "substitution",
  language: "",
  wholeWord: false,
  caseSensitive: false,
  priority: 0,
};

export const dictionaryEntryToFormState = (
  entry: DictionaryEntry,
): DictionaryFormState => ({
  term: entry.term,
  replacement: entry.replacement,
  kind: entry.kind,
  language: entry.language ?? "",
  wholeWord: entry.whole_word,
  caseSensitive: entry.case_sensitive,
  priority: entry.priority,
});

/**
 * Builds the payload sent to the backend. `enabled` is preserved from the
 * stored entry so editing a disabled rule never silently re-enables it.
 */
export const dictionaryFormToInput = (
  form: DictionaryFormState,
  existing?: Pick<DictionaryEntry, "enabled"> | null,
): DictionaryEntryInput => {
  const language = form.language.trim();
  return {
    term: form.term.trim(),
    // A "term" entry is a vocabulary hint only: it never rewrites the
    // transcript, so it carries no replacement.
    replacement: form.kind === "substitution" ? form.replacement.trim() : "",
    kind: form.kind,
    enabled: existing?.enabled ?? true,
    scope: DICTIONARY_GLOBAL_SCOPE,
    case_sensitive: form.caseSensitive,
    whole_word: form.wholeWord,
    priority: Number.isFinite(form.priority) ? form.priority : 0,
    language: language ? language : null,
  };
};

/** Strips the server-owned fields so an entry can be exported and re-imported. */
export const dictionaryEntryToInput = (
  entry: DictionaryEntry,
): DictionaryEntryInput => ({
  term: entry.term,
  replacement: entry.replacement,
  kind: entry.kind,
  enabled: entry.enabled,
  scope: entry.scope,
  case_sensitive: entry.case_sensitive,
  whole_word: entry.whole_word,
  priority: entry.priority,
  language: entry.language,
});
