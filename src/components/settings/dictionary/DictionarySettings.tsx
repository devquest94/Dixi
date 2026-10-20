import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { Download, Search, Upload } from "lucide-react";
import {
  Dialog,
  Dropdown,
  SettingContainer,
  SettingsGroup,
} from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { DictionaryEntryForm } from "./DictionaryEntryForm";
import { DictionaryEntryRow } from "./DictionaryEntryRow";
import { DictionaryPreview } from "./DictionaryPreview";
import {
  addDictionaryEntry,
  deleteDictionaryEntry,
  exportDictionaryEntries,
  importDictionaryEntriesFromFile,
  listDictionaryEntries,
  setDictionaryEntryEnabled,
  updateDictionaryEntry,
} from "./api";
import {
  EMPTY_DICTIONARY_FORM,
  dictionaryEntryToFormState,
  dictionaryFormToInput,
  type DictionaryEntry,
  type DictionaryEntryKind,
  type DictionaryFormState,
} from "./types";

type KindFilter = "all" | DictionaryEntryKind;

export const DictionarySettings: React.FC = () => {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [kindFilter, setKindFilter] = useState<KindFilter>("all");
  const [form, setForm] = useState<DictionaryFormState>(EMPTY_DICTIONARY_FORM);
  const [formError, setFormError] = useState<string | null>(null);
  const [editingEntry, setEditingEntry] = useState<DictionaryEntry | null>(
    null,
  );
  const [editForm, setEditForm] = useState<DictionaryFormState>(
    EMPTY_DICTIONARY_FORM,
  );
  const [editError, setEditError] = useState<string | null>(null);

  const loadEntries = useCallback(async () => {
    setIsLoading(true);
    try {
      const response = await listDictionaryEntries();
      if (response.status === "ok") {
        setEntries(response.data);
      } else {
        toast.error(response.error);
      }
    } catch (error) {
      console.error("Failed to load dictionary entries:", error);
      toast.error(t("settings.dictionary.loadError"));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadEntries();
  }, [loadEntries]);

  const filteredEntries = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    return entries.filter((entry) => {
      if (kindFilter !== "all" && entry.kind !== kindFilter) return false;
      if (!query) return true;
      const haystack = `${entry.term} ${entry.replacement}`.toLowerCase();
      return haystack.includes(query);
    });
  }, [entries, kindFilter, searchQuery]);

  const kindFilterOptions = useMemo(
    () => [
      { value: "all", label: t("settings.dictionary.filter.all") },
      { value: "term", label: t("settings.dictionary.filter.terms") },
      {
        value: "substitution",
        label: t("settings.dictionary.filter.substitutions"),
      },
    ],
    [t],
  );

  const handleAdd = async () => {
    if (!form.term.trim()) {
      setFormError(t("settings.dictionary.form.termRequired"));
      return;
    }
    if (form.kind === "substitution" && !form.replacement.trim()) {
      toast.error(t("settings.dictionary.form.replacementRequired"));
      return;
    }

    setFormError(null);
    setIsMutating(true);
    try {
      const response = await addDictionaryEntry(dictionaryFormToInput(form));
      if (response.status === "ok") {
        setForm(EMPTY_DICTIONARY_FORM);
        setFormError(null);
        await loadEntries();
        toast.success(t("settings.dictionary.added"));
      } else {
        toast.error(response.error);
      }
    } catch (error) {
      console.error("Failed to add dictionary entry:", error);
      toast.error(t("settings.dictionary.saveError"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleStartEdit = (entry: DictionaryEntry) => {
    setEditingEntry(entry);
    setEditForm(dictionaryEntryToFormState(entry));
    setEditError(null);
  };

  const handleCloseEdit = () => {
    setEditingEntry(null);
    setEditError(null);
  };

  const handleSaveEdit = async () => {
    if (!editingEntry) return;

    if (!editForm.term.trim()) {
      setEditError(t("settings.dictionary.form.termRequired"));
      return;
    }
    if (editForm.kind === "substitution" && !editForm.replacement.trim()) {
      toast.error(t("settings.dictionary.form.replacementRequired"));
      return;
    }

    setEditError(null);
    setIsMutating(true);
    try {
      const response = await updateDictionaryEntry(
        editingEntry.id,
        dictionaryFormToInput(editForm, editingEntry),
      );
      if (response.status === "ok") {
        handleCloseEdit();
        await loadEntries();
        toast.success(t("settings.dictionary.updated"));
      } else {
        toast.error(response.error);
      }
    } catch (error) {
      console.error("Failed to update dictionary entry:", error);
      toast.error(t("settings.dictionary.saveError"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleDelete = async (entry: DictionaryEntry) => {
    const confirmed = await ask(
      t("settings.dictionary.entry.deleteConfirm", { term: entry.term }),
      {
        title: t("settings.dictionary.entry.deleteTitle"),
        kind: "warning",
      },
    );
    if (!confirmed) return;

    setIsMutating(true);
    try {
      const response = await deleteDictionaryEntry(entry.id);
      if (response.status === "ok") {
        await loadEntries();
        toast.success(t("settings.dictionary.deleted"));
      } else {
        toast.error(response.error);
      }
    } catch (error) {
      console.error("Failed to delete dictionary entry:", error);
      toast.error(t("settings.dictionary.deleteError"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleToggle = async (entry: DictionaryEntry, enabled: boolean) => {
    setIsMutating(true);
    try {
      const response = await setDictionaryEntryEnabled(entry.id, enabled);
      if (response.status !== "ok") {
        toast.error(response.error);
      }
      await loadEntries();
    } catch (error) {
      console.error("Failed to toggle dictionary entry:", error);
      toast.error(t("settings.dictionary.toggleError"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleExport = async () => {
    if (entries.length === 0) {
      toast.error(t("settings.dictionary.transfer.exportEmpty"));
      return;
    }

    try {
      const path = await save({
        title: t("settings.dictionary.transfer.export"),
        defaultPath: "dixi-dictionary.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;

      // The backend owns the file format and the filesystem access: the
      // webview only chooses the destination.
      //
      // The backend refuses to replace an existing file unless it is told
      // to, so an accidental double click cannot destroy a file the user had
      // edited. The confirmation is asked here, in the app's own language.
      const response = await exportDictionaryEntries(path, false);
      if (response.status === "ok") {
        toast.success(
          t("settings.dictionary.transfer.exportSuccess", {
            count: response.data,
          }),
        );
        return;
      }

      if (!response.error.includes("already exists")) {
        toast.error(t("settings.dictionary.transfer.exportError"), {
          description: response.error,
        });
        return;
      }

      const confirmed = await ask(
        t("settings.dictionary.transfer.overwriteTitle"),
        {
          title: t("settings.dictionary.transfer.overwriteTitle"),
          kind: "warning",
        },
      );
      if (!confirmed) return;

      const replaced = await exportDictionaryEntries(path, true);
      if (replaced.status === "ok") {
        toast.success(
          t("settings.dictionary.transfer.exportSuccess", {
            count: replaced.data,
          }),
        );
      } else {
        toast.error(t("settings.dictionary.transfer.exportError"), {
          description: replaced.error,
        });
      }
    } catch (error) {
      console.error("Failed to export dictionary entries:", error);
      toast.error(t("settings.dictionary.transfer.exportError"));
    }
  };

  const handleImport = async () => {
    try {
      const selected = await open({
        title: t("settings.dictionary.transfer.import"),
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (typeof selected !== "string" || !selected) return;

      // The backend reads and validates the file, skips the entries that
      // already exist and returns how many entries were actually added.
      const response = await importDictionaryEntriesFromFile(selected);
      if (response.status === "ok") {
        await loadEntries();
        toast.success(
          t("settings.dictionary.transfer.importSuccess", {
            count: response.data,
          }),
        );
      } else {
        toast.error(t("settings.dictionary.transfer.importInvalid"), {
          description: response.error,
        });
      }
    } catch (error) {
      console.error("Failed to import dictionary entries:", error);
      toast.error(t("settings.dictionary.transfer.importError"));
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.dictionary.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.dictionary.description")}
        </p>
      </div>

      <SettingsGroup
        title={t("settings.dictionary.entries.title")}
        description={t("settings.dictionary.entries.description")}
      >
        <SettingContainer
          title={t("settings.dictionary.search.label")}
          description={t("settings.dictionary.search.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-2">
              <Search className="w-4 h-4 text-text/40 shrink-0" />
              <Input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={t("settings.dictionary.search.placeholder")}
                className="w-[220px]"
              />
            </div>
            <Dropdown
              options={kindFilterOptions}
              selectedValue={kindFilter}
              onSelect={(value) => setKindFilter(value as KindFilter)}
              className="min-w-[160px]"
            />
          </div>
        </SettingContainer>

        {isLoading ? (
          <div className="flex items-center justify-center py-10">
            <div className="w-6 h-6 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
          </div>
        ) : filteredEntries.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {entries.length === 0
              ? t("settings.dictionary.empty")
              : t("settings.dictionary.noMatches")}
          </div>
        ) : (
          filteredEntries.map((entry) => (
            <DictionaryEntryRow
              key={entry.id}
              entry={entry}
              onToggle={handleToggle}
              onEdit={handleStartEdit}
              onDelete={handleDelete}
              busy={isMutating}
            />
          ))
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.dictionary.form.addTitle")}
        description={t("settings.dictionary.form.description")}
      >
        <div className="px-4 py-3">
          <DictionaryEntryForm
            form={form}
            onChange={setForm}
            onSubmit={handleAdd}
            submitLabel={t("settings.dictionary.form.submitAdd")}
            disabled={isMutating}
            termError={formError}
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.dictionary.preview.title")}
        description={t("settings.dictionary.preview.description")}
      >
        <div className="px-4 py-3">
          <DictionaryPreview />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.dictionary.transfer.title")}
        description={t("settings.dictionary.transfer.description")}
      >
        <SettingContainer
          title={t("settings.dictionary.transfer.filesTitle")}
          description={t("settings.dictionary.transfer.filesDescription")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <div className="flex items-center gap-2">
            <Button
              onClick={handleExport}
              disabled={isMutating}
              className="flex items-center gap-2"
            >
              <Download className="w-4 h-4" />
              {t("settings.dictionary.transfer.export")}
            </Button>
            <Button
              variant="secondary"
              onClick={handleImport}
              disabled={isMutating}
              className="flex items-center gap-2"
            >
              <Upload className="w-4 h-4" />
              {t("settings.dictionary.transfer.import")}
            </Button>
          </div>
        </SettingContainer>
      </SettingsGroup>

      <Dialog
        open={editingEntry !== null}
        title={t("settings.dictionary.form.editTitle")}
        description={t("settings.dictionary.form.editDescription")}
        onOpenChange={(open) => {
          if (!open) handleCloseEdit();
        }}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button variant="secondary" onClick={handleCloseEdit}>
              {t("settings.dictionary.form.cancel")}
            </Button>
            <Button onClick={handleSaveEdit} disabled={isMutating}>
              {t("settings.dictionary.form.submitSave")}
            </Button>
          </>
        }
      >
        <DictionaryEntryForm
          form={editForm}
          onChange={setEditForm}
          onSubmit={handleSaveEdit}
          submitLabel={t("settings.dictionary.form.submitSave")}
          showActions={false}
          disabled={isMutating}
          termError={editError}
          idPrefix="dictionary-edit"
        />
      </Dialog>
    </div>
  );
};
