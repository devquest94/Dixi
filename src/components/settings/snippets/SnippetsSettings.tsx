import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import {
  Download,
  Pencil,
  Plus,
  Star,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import {
  commands,
  type Snippet,
  type SnippetExpansionResult,
  type SnippetVariable,
} from "@/bindings";
import {
  Dialog,
  SettingContainer,
  SettingsGroup,
  ToggleSwitch,
} from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Textarea } from "@/components/ui/Textarea";
import Badge from "@/components/ui/Badge";

interface SnippetFormState {
  name: string;
  trigger_phrase: string;
  expansion: string;
  variables: SnippetVariable[];
  category: string;
  app_names: string;
  language: string;
  is_favorite: boolean;
  enabled: boolean;
}

const EMPTY_SNIPPET_FORM: SnippetFormState = {
  name: "",
  trigger_phrase: "",
  expansion: "",
  variables: [],
  category: "",
  app_names: "",
  language: "",
  is_favorite: false,
  enabled: true,
};

const parseAppNames = (value: string): string[] =>
  value
    .split(",")
    .map((name) => name.trim().toLowerCase())
    .filter((name) => name.length > 0);

export const SnippetsSettings: React.FC = () => {
  const { t } = useTranslation();

  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);

  const [form, setForm] = useState<SnippetFormState | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);

  const [previewText, setPreviewText] = useState<string>("");
  const [previewValues, setPreviewValues] = useState<Record<string, string>>({});
  const [previewResult, setPreviewResult] =
    useState<SnippetExpansionResult | null>(null);
  const [pendingVariable, setPendingVariable] = useState<string | null>(null);
  const [pendingValue, setPendingValue] = useState<string>("");
  const [isExpanding, setIsExpanding] = useState(false);

  const loadSnippets = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await commands.listSnippets();
      if (result.status === "ok") {
        setSnippets(result.data);
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to load the snippets:", error);
      toast.error(t("settings.snippets.errors.load"));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadSnippets();
  }, [loadSnippets]);

  const handleStartCreate = () => {
    setEditingId(null);
    setForm({ ...EMPTY_SNIPPET_FORM });
  };

  const handleStartEdit = (snippet: Snippet) => {
    setEditingId(snippet.id);
    setForm({
      name: snippet.name,
      trigger_phrase: snippet.trigger_phrase,
      expansion: snippet.expansion,
      variables: snippet.variables.map((variable) => ({ ...variable })),
      category: snippet.category ?? "",
      app_names: snippet.app_names.join(", "),
      language: snippet.language ?? "",
      // La estrella se conserva al editar: el backend escribe el campo
      // tal cual, y perderla cambiaria el orden de resolucion.
      is_favorite: snippet.is_favorite,
      enabled: snippet.enabled,
    });
  };

  const handleSave = async () => {
    if (!form) return;
    if (!form.name.trim() || !form.trigger_phrase.trim()) {
      toast.error(t("settings.snippets.form.required"));
      return;
    }

    setIsMutating(true);
    try {
      const payload = {
        name: form.name.trim(),
        trigger_phrase: form.trigger_phrase.trim(),
        expansion: form.expansion,
        variables: form.variables
          .filter((variable) => variable.name.trim().length > 0)
          .map((variable) => ({
            name: variable.name.trim(),
            required: variable.required ?? false,
            prompt: variable.prompt ?? "",
          })),
        category: form.category.trim() || null,
        app_names: parseAppNames(form.app_names),
        language: form.language.trim() || null,
        is_favorite: form.is_favorite,
        enabled: form.enabled,
      };

      const result = editingId
        ? await commands.updateSnippet(editingId, payload)
        : await commands.addSnippet(payload);

      if (result.status === "ok") {
        setForm(null);
        setEditingId(null);
        await loadSnippets();
        toast.success(t("settings.snippets.actions.save"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to save the snippet:", error);
      toast.error(t("settings.snippets.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleDelete = async (snippet: Snippet) => {
    setIsMutating(true);
    try {
      const result = await commands.deleteSnippet(snippet.id);
      if (result.status === "ok") {
        await loadSnippets();
        toast.success(t("settings.snippets.actions.delete"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to delete the snippet:", error);
      toast.error(t("settings.snippets.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleToggleFavorite = async (
    snippet: Snippet,
    favorite: boolean,
  ) => {
    setIsMutating(true);
    try {
      const result = await commands.setSnippetFavorite(snippet.id, favorite);
      if (result.status === "ok") {
        await loadSnippets();
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to star the snippet:", error);
      toast.error(t("settings.snippets.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleToggleEnabled = async (snippet: Snippet, enabled: boolean) => {
    setIsMutating(true);
    try {
      const result = await commands.setSnippetEnabled(snippet.id, enabled);
      if (result.status === "ok") {
        await loadSnippets();
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to enable the snippet:", error);
      toast.error(t("settings.snippets.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const runPreview = async (values: Record<string, string>) => {
    if (!previewText.trim()) {
      toast.error(t("settings.snippets.preview.inputRequired"));
      return;
    }

    setIsExpanding(true);
    try {
      const result = await commands.expandSnippet(
        previewText,
        values,
        null,
        null,
      );
      if (result.status === "ok") {
        setPreviewResult(result.data);
        setPreviewValues(values);
        if (result.data.needs_input) {
          setPendingVariable(result.data.needs_input.variable_name);
          setPendingValue("");
        } else {
          setPendingVariable(null);
        }
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to expand the snippet:", error);
      toast.error(t("settings.snippets.errors.action"));
    } finally {
      setIsExpanding(false);
    }
  };

  const handleRetryPreview = async () => {
    if (!pendingVariable) return;
    await runPreview({ ...previewValues, [pendingVariable]: pendingValue });
  };

  const handleExport = async () => {
    if (snippets.length === 0) {
      toast.error(t("settings.snippets.transfer.exportEmpty"));
      return;
    }

    try {
      const path = await save({
        title: t("settings.snippets.transfer.export"),
        defaultPath: "dixi-snippets.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;

      // The backend refuses to replace an existing file unless it is told to,
      // so an accidental double click cannot destroy a file the user had
      // edited. The confirmation is asked here, in the app's own language.
      const response = await commands.exportSnippets(path, false);
      if (response.status === "ok") {
        toast.success(
          t("settings.snippets.transfer.exportSuccess", {
            count: response.data,
          }),
        );
        return;
      }

      if (!response.error.includes("already exists")) {
        toast.error(t("settings.snippets.transfer.exportError"), {
          description: response.error,
        });
        return;
      }

      const confirmed = await ask(t("settings.snippets.transfer.overwriteTitle"), {
        title: t("settings.snippets.transfer.overwriteTitle"),
        kind: "warning",
      });
      if (!confirmed) return;

      const replaced = await commands.exportSnippets(path, true);
      if (replaced.status === "ok") {
        toast.success(
          t("settings.snippets.transfer.exportSuccess", {
            count: replaced.data,
          }),
        );
      } else {
        toast.error(t("settings.snippets.transfer.exportError"), {
          description: replaced.error,
        });
      }
    } catch (error) {
      console.error("Failed to export the snippets:", error);
      toast.error(t("settings.snippets.transfer.exportError"));
    }
  };

  const handleImport = async () => {
    try {
      const selected = await open({
        title: t("settings.snippets.transfer.import"),
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (typeof selected !== "string" || !selected) return;

      const response = await commands.importSnippetsFromFile(selected);
      if (response.status === "ok") {
        await loadSnippets();
        toast.success(
          t("settings.snippets.transfer.importSuccess", {
            count: response.data,
          }),
        );
      } else {
        toast.error(t("settings.snippets.transfer.importError"), {
          description: response.error,
        });
      }
    } catch (error) {
      console.error("Failed to import the snippets:", error);
      toast.error(t("settings.snippets.transfer.importError"));
    }
  };

  const updateVariable = (
    index: number,
    patch: Partial<SnippetVariable>,
  ) => {
    setForm((current) => {
      if (!current) return current;
      const variables = current.variables.map((variable, position) =>
        position === index ? { ...variable, ...patch } : variable,
      );
      return { ...current, variables };
    });
  };

  const addVariable = () => {
    setForm((current) => {
      if (!current) return current;
      return {
        ...current,
        variables: [
          ...current.variables,
          { name: "", required: false, prompt: "" },
        ],
      };
    });
  };

  const removeVariable = (index: number) => {
    setForm((current) => {
      if (!current) return current;
      return {
        ...current,
        variables: current.variables.filter((_, position) => position !== index),
      };
    });
  };

  const needsInput = previewResult?.needs_input ?? null;

  const pendingPrompt = useMemo(() => {
    if (!needsInput || !pendingVariable) return null;
    return needsInput.prompt;
  }, [needsInput, pendingVariable]);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.snippets.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.snippets.description")}
        </p>
      </div>

      <SettingsGroup
        title={t("settings.snippets.list.title")}
        description={t("settings.snippets.list.description")}
      >
        <div className="px-4 py-3 flex items-center justify-end">
          <Button
            variant="primary"
            size="md"
            onClick={handleStartCreate}
            disabled={isLoading}
            className="flex items-center gap-2"
          >
            <Plus className="w-4 h-4" />
            {t("settings.snippets.actions.add")}
          </Button>
        </div>

        {isLoading ? (
          <div className="flex items-center justify-center py-10">
            <div className="w-6 h-6 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
          </div>
        ) : snippets.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.snippets.list.empty")}
          </div>
        ) : (
          snippets.map((snippet) => (
            <div
              key={snippet.id}
              className="px-4 py-3 flex items-start justify-between gap-3"
            >
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-medium text-text break-words">
                    {snippet.name}
                  </span>
                  {snippet.category && (
                    <Badge variant="secondary" className="px-2 py-0.5">
                      {snippet.category}
                    </Badge>
                  )}
                </div>
                <div className="flex items-center gap-3 flex-wrap text-xs text-mid-gray">
                  <span>
                    {t("settings.snippets.form.trigger")}:{" "}
                    <code className="text-text">{snippet.trigger_phrase}</code>
                  </span>
                  <span>
                    {t("settings.snippets.list.usageCount", {
                      count: snippet.usage_count,
                    })}
                  </span>
                  <span>
                    {t("settings.snippets.form.applications")}:{" "}
                    {snippet.app_names.length === 0
                      ? t("settings.snippets.list.everywhere")
                      : snippet.app_names.join(", ")}
                  </span>
                  {snippet.language && (
                    <span>
                      {t("settings.snippets.form.language")}:{" "}
                      {snippet.language}
                    </span>
                  )}
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() =>
                    void handleToggleFavorite(snippet, !snippet.is_favorite)
                  }
                  disabled={isMutating}
                  aria-label={t("settings.snippets.list.favorite")}
                  title={t("settings.snippets.list.favorite")}
                  className="flex items-center"
                >
                  <Star
                    className={`w-4 h-4 ${snippet.is_favorite ? "fill-logo-primary text-logo-primary" : "text-mid-gray"}`}
                  />
                </Button>
                <ToggleSwitch
                  checked={snippet.enabled}
                  onChange={(checked) =>
                    void handleToggleEnabled(snippet, checked)
                  }
                  label={t("settings.snippets.list.enabled")}
                  description={t("settings.snippets.list.enabledDescription")}
                  grouped={true}
                  disabled={isMutating}
                />
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleStartEdit(snippet)}
                  disabled={isMutating}
                  aria-label={t("settings.snippets.actions.edit")}
                  title={t("settings.snippets.actions.edit")}
                  className="flex items-center"
                >
                  <Pencil className="w-3.5 h-3.5" />
                </Button>
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={() => void handleDelete(snippet)}
                  disabled={isMutating}
                  aria-label={t("settings.snippets.actions.delete")}
                  title={t("settings.snippets.actions.delete")}
                  className="flex items-center"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </Button>
              </div>
            </div>
          ))
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.snippets.preview.title")}
        description={t("settings.snippets.preview.description")}
      >
        <SettingContainer
          title={t("settings.snippets.preview.input")}
          description={t("settings.snippets.preview.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <Textarea
            value={previewText}
            onChange={(e) => setPreviewText(e.target.value)}
            placeholder={t("settings.snippets.preview.inputPlaceholder")}
            variant="compact"
            className="min-w-[320px]"
          />
        </SettingContainer>

        <div className="px-4 py-3 flex items-center justify-end">
          <Button
            variant="primary"
            size="md"
            onClick={() => void runPreview({})}
            disabled={isExpanding}
          >
            {t("settings.snippets.preview.run")}
          </Button>
        </div>

        {previewResult && (
          <div className="px-4 py-3 space-y-2">
            <p className="text-xs text-mid-gray">
              {previewResult.expanded
                ? t("settings.snippets.preview.expanded")
                : t("settings.snippets.preview.notExpanded")}
            </p>
            <pre className="text-sm text-text whitespace-pre-wrap break-words bg-mid-gray/5 border border-mid-gray/20 rounded-lg p-3">
              {previewResult.text}
            </pre>

            {needsInput && (
              <div className="space-y-2 rounded-lg border border-logo-primary/40 bg-logo-primary/5 p-3">
                <p className="text-sm text-text">
                  {pendingPrompt || needsInput.variable_name}
                </p>
                <div className="flex items-center gap-2 justify-end">
                  <Input
                    type="text"
                    value={pendingValue}
                    onChange={(e) => setPendingValue(e.target.value)}
                    placeholder={needsInput.variable_name}
                    variant="compact"
                    className="min-w-[220px]"
                  />
                  <Button
                    variant="primary"
                    size="md"
                    onClick={() => void handleRetryPreview()}
                    disabled={isExpanding}
                  >
                    {t("settings.snippets.preview.retry")}
                  </Button>
                </div>
              </div>
            )}
          </div>
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.snippets.transfer.title")}
        description={t("settings.snippets.transfer.description")}
      >
        <SettingContainer
          title={t("settings.snippets.transfer.filesTitle")}
          description={t("settings.snippets.transfer.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex items-center gap-2">
            <Button
              onClick={() => void handleExport()}
              disabled={isMutating}
              className="flex items-center gap-2"
            >
              <Download className="w-4 h-4" />
              {t("settings.snippets.transfer.export")}
            </Button>
            <Button
              variant="secondary"
              onClick={() => void handleImport()}
              disabled={isMutating}
              className="flex items-center gap-2"
            >
              <Upload className="w-4 h-4" />
              {t("settings.snippets.transfer.import")}
            </Button>
          </div>
        </SettingContainer>
      </SettingsGroup>

      <Dialog
        open={form !== null}
        title={
          editingId
            ? t("settings.snippets.actions.edit")
            : t("settings.snippets.actions.add")
        }
        description={t("settings.snippets.description")}
        onOpenChange={(open) => {
          if (!open) {
            setForm(null);
            setEditingId(null);
          }
        }}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button
              variant="secondary"
              onClick={() => {
                setForm(null);
                setEditingId(null);
              }}
            >
              {t("settings.snippets.actions.cancel")}
            </Button>
            <Button onClick={() => void handleSave()} disabled={isMutating}>
              {t("settings.snippets.actions.save")}
            </Button>
          </>
        }
      >
        {form && (
          <div className="space-y-3">
            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.snippets.form.name")}
              </label>
              <Input
                type="text"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder={t("settings.snippets.form.namePlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.snippets.form.trigger")}
              </label>
              <Input
                type="text"
                value={form.trigger_phrase}
                onChange={(e) =>
                  setForm({ ...form, trigger_phrase: e.target.value })
                }
                placeholder={t("settings.snippets.form.triggerPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.snippets.form.expansion")}
              </label>
              <Textarea
                value={form.expansion}
                onChange={(e) =>
                  setForm({ ...form, expansion: e.target.value })
                }
                placeholder={t("settings.snippets.form.expansionPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <div className="flex items-center justify-between gap-2">
                <label className="text-sm font-semibold text-text">
                  {t("settings.snippets.form.variables")}
                </label>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={addVariable}
                  className="flex items-center gap-1"
                >
                  <Plus className="w-3.5 h-3.5" />
                  {t("settings.snippets.form.addVariable")}
                </Button>
              </div>
              <p className="text-xs text-mid-gray">
                {t("settings.snippets.form.variablesDescription")}
              </p>
              <p className="text-xs text-mid-gray">
                {t("settings.snippets.form.builtinVariables")}
              </p>

              {form.variables.length === 0 ? (
                <p className="text-xs text-mid-gray">
                  {t("settings.snippets.form.noVariables")}
                </p>
              ) : (
                form.variables.map((variable, index) => (
                  <div
                    key={index}
                    className="flex flex-col gap-2 rounded-lg border border-mid-gray/20 p-2"
                  >
                    <div className="flex items-center gap-2">
                      <Input
                        type="text"
                        value={variable.name}
                        onChange={(e) =>
                          updateVariable(index, { name: e.target.value })
                        }
                        placeholder={t("settings.snippets.form.variableName")}
                        variant="compact"
                        className="min-w-[140px]"
                      />
                      <label className="flex items-center gap-1 text-xs text-text whitespace-nowrap cursor-pointer">
                        <input
                          type="checkbox"
                          className="accent-logo-primary"
                          checked={variable.required ?? false}
                          onChange={(e) =>
                            updateVariable(index, { required: e.target.checked })
                          }
                        />
                        {t("settings.snippets.form.variableRequired")}
                      </label>
                      <Button
                        variant="danger-ghost"
                        size="sm"
                        onClick={() => removeVariable(index)}
                        aria-label={t("settings.snippets.form.removeVariable")}
                        title={t("settings.snippets.form.removeVariable")}
                        className="flex items-center"
                      >
                        <X className="w-3.5 h-3.5" />
                      </Button>
                    </div>
                    <Input
                      type="text"
                      value={variable.prompt ?? ""}
                      onChange={(e) =>
                        updateVariable(index, { prompt: e.target.value })
                      }
                      placeholder={t("settings.snippets.form.variablePrompt")}
                      variant="compact"
                    />
                  </div>
                ))
              )}
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.snippets.form.category")}
              </label>
              <Input
                type="text"
                value={form.category}
                onChange={(e) =>
                  setForm({ ...form, category: e.target.value })
                }
                placeholder={t("settings.snippets.form.categoryPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.snippets.form.applications")}
              </label>
              <Input
                type="text"
                value={form.app_names}
                onChange={(e) =>
                  setForm({ ...form, app_names: e.target.value })
                }
                placeholder={t("settings.snippets.form.applicationsPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.snippets.form.language")}
              </label>
              <Input
                type="text"
                value={form.language}
                onChange={(e) =>
                  setForm({ ...form, language: e.target.value })
                }
                placeholder={t("settings.snippets.form.languagePlaceholder")}
                variant="compact"
              />
            </div>

            <ToggleSwitch
              checked={form.enabled}
              onChange={(checked) => setForm({ ...form, enabled: checked })}
              label={t("settings.snippets.list.enabled")}
              description={t("settings.snippets.list.enabledDescription")}
              grouped={true}
            />
          </div>
        )}
      </Dialog>
    </div>
  );
};
