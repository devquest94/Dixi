import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Copy, Pencil, Plus, Trash2 } from "lucide-react";
import {
  commands,
  type SelectionInfo,
  type TextDiffInfo,
  type Transform,
} from "@/bindings";
import {
  Dialog,
  Dropdown,
  SettingContainer,
  SettingsGroup,
  ToggleSwitch,
} from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Textarea } from "@/components/ui/Textarea";
import Badge from "@/components/ui/Badge";
import { useSettings } from "@/hooks/useSettings";

const NONE = "";

interface TransformFormState {
  name: string;
  prompt: string;
  provider_id: string;
  model: string;
  shortcut: string | null;
  category: string;
  enabled: boolean;
}

const EMPTY_TRANSFORM_FORM: TransformFormState = {
  name: "",
  prompt: "",
  provider_id: NONE,
  model: "",
  shortcut: null,
  category: "",
  enabled: true,
};

export const TransformsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const [transforms, setTransforms] = useState<Transform[]>([]);
  const [providerLabels, setProviderLabels] = useState<Record<string, string>>(
    {},
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);

  const [form, setForm] = useState<TransformFormState | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);

  const [runnerText, setRunnerText] = useState<string>("");
  const [selectedTransformIds, setSelectedTransformIds] = useState<string[]>([]);
  const [runnerResult, setRunnerResult] = useState<string | null>(null);
  const [diff, setDiff] = useState<TextDiffInfo | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [selection, setSelection] = useState<SelectionInfo | null>(null);
  const [selectionChanged, setSelectionChanged] = useState(false);

  const loadTransforms = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await commands.listTransforms();
      if (result.status !== "ok") {
        toast.error(result.error);
        setIsLoading(false);
        return;
      }

      setTransforms(result.data);

      // The provider a transform would really use is resolved by the backend,
      // so the list shows the effective provider and not the stored hint.
      const providers = await Promise.all(
        result.data.map(async (transform) => {
          const provider = await commands.getTransformProvider(transform.id);
          if (provider.status === "ok" && provider.data) {
            return [transform.id, provider.data.label] as const;
          }
          return [transform.id, null] as const;
        }),
      );

      const labels: Record<string, string> = {};
      for (const [id, label] of providers) {
        if (label) labels[id] = label;
      }
      setProviderLabels(labels);
    } catch (error) {
      console.error("Failed to load the transforms:", error);
      toast.error(t("settings.transforms.errors.load"));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadTransforms();
  }, [loadTransforms]);

  const providerOptions = useMemo(() => {
    const providers = settings?.post_process_providers ?? [];
    const options = [
      {
        value: NONE,
        label: t("settings.transforms.form.providerDefault"),
      },
      ...providers.map((provider) => ({
        value: provider.id,
        label: provider.label,
      })),
    ];

    // Un proveedor guardado que ya no esta en la configuracion se sigue
    // mostrando: si no, el desplegable diria "por defecto" mientras el
    // transform sigue guardando el identificador viejo.
    const stored = form?.provider_id;
    if (stored && stored !== NONE && !options.some((option) => option.value === stored)) {
      options.push({ value: stored, label: stored });
    }

    return options;
  }, [settings?.post_process_providers, form?.provider_id, t]);

  const handleStartCreate = () => {
    setEditingId(null);
    setForm({ ...EMPTY_TRANSFORM_FORM });
  };

  const handleStartEdit = (transform: Transform) => {
    setEditingId(transform.id);
    setForm({
      name: transform.name,
      prompt: transform.prompt,
      provider_id: transform.provider_id ?? NONE,
      model: transform.model ?? "",
      // El atajo no se edita aqui, pero se conserva: el backend escribe el
      // campo tal cual y perderlo borraria el atajo del transform.
      shortcut: transform.shortcut ?? null,
      category: transform.category ?? "",
      enabled: transform.enabled,
    });
  };

  const handleSave = async () => {
    if (!form) return;
    if (!form.name.trim() || !form.prompt.trim()) {
      toast.error(t("settings.transforms.form.required"));
      return;
    }

    setIsMutating(true);
    try {
      const payload = {
        name: form.name.trim(),
        prompt: form.prompt.trim(),
        provider_id: form.provider_id === NONE ? null : form.provider_id,
        model: form.model.trim() || null,
        shortcut: form.shortcut ?? null,
        category: form.category.trim() || null,
        enabled: form.enabled,
      };

      const result = editingId
        ? await commands.updateTransform(editingId, payload)
        : await commands.addTransform(payload);

      if (result.status === "ok") {
        setForm(null);
        setEditingId(null);
        await loadTransforms();
        toast.success(t("settings.transforms.actions.save"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to save the transform:", error);
      toast.error(t("settings.transforms.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleDelete = async (transform: Transform) => {
    if (transform.kind === "builtin") return;

    setIsMutating(true);
    try {
      const result = await commands.deleteTransform(transform.id);
      if (result.status === "ok") {
        await loadTransforms();
        toast.success(t("settings.transforms.actions.delete"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to delete the transform:", error);
      toast.error(t("settings.transforms.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleToggleEnabled = async (
    transform: Transform,
    enabled: boolean,
  ) => {
    setIsMutating(true);
    try {
      const result = await commands.setTransformEnabled(transform.id, enabled);
      if (result.status === "ok") {
        await loadTransforms();
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to enable the transform:", error);
      toast.error(t("settings.transforms.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleCaptureSelection = async () => {
    try {
      const result = await commands.captureSelection();
      if (result.status === "ok") {
        if (!result.data.text) {
          toast.error(t("settings.transforms.runner.selectionEmpty"));
          return;
        }
        setSelection(result.data);
        setSelectionChanged(false);
        setRunnerText(result.data.text);
        setRunnerResult(null);
        setDiff(null);
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to capture the selection:", error);
      toast.error(t("settings.transforms.errors.action"));
    }
  };

  const handleRun = async () => {
    if (!runnerText.trim()) {
      toast.error(t("settings.transforms.runner.inputRequired"));
      return;
    }
    if (selectedTransformIds.length === 0) {
      toast.error(t("settings.transforms.runner.selectRequired"));
      return;
    }

    setIsRunning(true);
    try {
      const original = runnerText;
      let current = original;

      for (const id of selectedTransformIds) {
        const result = await commands.runTransform(id, current);
        if (result.status !== "ok") {
          toast.error(result.error);
          // Un transform fallido no deja resultado: sin resultado no hay
          // diff y "Aceptar" queda deshabilitado, para no poder escribir
          // sobre la seleccion un texto que nunca se transformo.
          setRunnerResult(null);
          setDiff(null);
          return;
        }
        if (result.data.error) {
          toast.error(result.data.error);
          setRunnerResult(null);
          setDiff(null);
          return;
        }
        current = result.data.text;
      }

      setRunnerResult(current);

      const diffResult = await commands.diffText(original, current);
      if (diffResult.status === "ok") {
        setDiff(diffResult.data);
      } else {
        toast.error(diffResult.error);
      }
    } catch (error) {
      console.error("Failed to run the transforms:", error);
      toast.error(t("settings.transforms.errors.action"));
    } finally {
      setIsRunning(false);
    }
  };

  const handleAccept = async () => {
    if (runnerResult === null) return;

    if (!selection) {
      setRunnerText(runnerResult);
      toast.success(t("settings.transforms.runner.accepted"));
      return;
    }

    try {
      const result = await commands.replaceTextSelection(
        selection.text,
        selection.window,
        selection.control,
        runnerResult,
      );
      if (result.status !== "ok") {
        toast.error(result.error);
        return;
      }
      if (result.data) {
        setSelectionChanged(false);
        toast.success(t("settings.transforms.runner.accepted"));
      } else {
        // The selection changed while the transformation was running: nothing
        // is written, the result is offered to be copied instead.
        setSelectionChanged(true);
        toast.error(t("settings.transforms.runner.selectionChanged"));
      }
    } catch (error) {
      console.error("Failed to replace the selection:", error);
      toast.error(t("settings.transforms.errors.action"));
    }
  };

  const handleCopy = async () => {
    if (runnerResult === null) return;
    try {
      await navigator.clipboard.writeText(runnerResult);
      toast.success(t("settings.transforms.runner.copied"));
    } catch (error) {
      console.error("Failed to copy the result:", error);
      toast.error(t("settings.transforms.errors.action"));
    }
  };

  const toggleSelectedTransform = (id: string) => {
    setSelectedTransformIds((current) =>
      current.includes(id)
        ? current.filter((item) => item !== id)
        : [...current, id],
    );
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.transforms.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.transforms.description")}
        </p>
      </div>

      <SettingsGroup
        title={t("settings.transforms.list.title")}
        description={t("settings.transforms.list.description")}
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
            {t("settings.transforms.actions.add")}
          </Button>
        </div>

        {isLoading ? (
          <div className="flex items-center justify-center py-10">
            <div className="w-6 h-6 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
          </div>
        ) : transforms.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.transforms.list.empty")}
          </div>
        ) : (
          transforms.map((transform) => (
            <div
              key={transform.id}
              className="px-4 py-3 flex items-start justify-between gap-3"
            >
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-medium text-text break-words">
                    {transform.name}
                  </span>
                  {transform.kind === "builtin" && (
                    <Badge variant="secondary" className="px-2 py-0.5">
                      {t("settings.transforms.list.builtin")}
                    </Badge>
                  )}
                  {transform.category && (
                    <Badge variant="secondary" className="px-2 py-0.5">
                      {transform.category}
                    </Badge>
                  )}
                </div>
                <div className="flex items-center gap-3 flex-wrap text-xs text-mid-gray">
                  <span>
                    {t("settings.transforms.list.provider")}:{" "}
                    {providerLabels[transform.id] ??
                      transform.provider_id ??
                      t("settings.transforms.form.providerDefault")}
                  </span>
                  <span>
                    {t("settings.transforms.list.model")}:{" "}
                    {transform.model ?? "—"}
                  </span>
                  <span>
                    {t("settings.transforms.list.usageCount", {
                      count: transform.usage_count,
                    })}
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                <ToggleSwitch
                  checked={transform.enabled}
                  onChange={(checked) =>
                    void handleToggleEnabled(transform, checked)
                  }
                  label={t("settings.transforms.list.enabled")}
                  description={t("settings.transforms.list.enabledDescription")}
                  grouped={true}
                  disabled={isMutating}
                />
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleStartEdit(transform)}
                  disabled={isMutating || transform.kind === "builtin"}
                  aria-label={t("settings.transforms.actions.edit")}
                  title={
                    transform.kind === "builtin"
                      ? t("settings.transforms.actions.builtinCannotEdit")
                      : t("settings.transforms.actions.edit")
                  }
                  className="flex items-center"
                >
                  <Pencil className="w-3.5 h-3.5" />
                </Button>
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={() => void handleDelete(transform)}
                  disabled={isMutating || transform.kind === "builtin"}
                  aria-label={t("settings.transforms.actions.delete")}
                  title={
                    transform.kind === "builtin"
                      ? t("settings.transforms.actions.builtinCannotDelete")
                      : t("settings.transforms.actions.delete")
                  }
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
        title={t("settings.transforms.runner.title")}
        description={t("settings.transforms.runner.description")}
      >
        <SettingContainer
          title={t("settings.transforms.runner.input")}
          description={t("settings.transforms.runner.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex flex-col items-end gap-2">
            <Textarea
              value={runnerText}
              onChange={(e) => setRunnerText(e.target.value)}
              placeholder={t("settings.transforms.runner.inputPlaceholder")}
              variant="compact"
              className="min-w-[320px]"
            />
            <Button
              variant="secondary"
              size="md"
              onClick={() => void handleCaptureSelection()}
              disabled={isRunning}
            >
              {t("settings.transforms.runner.captureSelection")}
            </Button>
          </div>
        </SettingContainer>

        <SettingContainer
          title={t("settings.transforms.runner.selectTransform")}
          description={t("settings.transforms.runner.selectTransform")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex flex-wrap gap-3 justify-end">
            {transforms
              .filter((transform) => transform.enabled)
              .map((transform) => (
                <label
                  key={transform.id}
                  className="flex items-center gap-1 text-sm text-text cursor-pointer"
                >
                  <input
                    type="checkbox"
                    className="accent-logo-primary"
                    checked={selectedTransformIds.includes(transform.id)}
                    onChange={() => toggleSelectedTransform(transform.id)}
                  />
                  {transform.name}
                </label>
              ))}
          </div>
        </SettingContainer>

        <div className="px-4 py-3 flex items-center justify-end gap-2">
          <Button
            variant="primary"
            size="md"
            onClick={() => void handleRun()}
            disabled={isRunning}
          >
            {t("settings.transforms.runner.run")}
          </Button>
          {runnerResult !== null && (
            <>
              <Button
                variant="secondary"
                size="md"
                onClick={() => void handleAccept()}
                disabled={isRunning}
              >
                {t("settings.transforms.runner.accept")}
              </Button>
              <Button
                variant="secondary"
                size="md"
                onClick={() => void handleCopy()}
                disabled={isRunning}
                className="flex items-center gap-2"
              >
                <Copy className="w-4 h-4" />
                {t("settings.transforms.runner.copy")}
              </Button>
            </>
          )}
        </div>

        {selectionChanged && (
          <div className="px-4 py-3">
            <p className="text-sm text-error">
              {t("settings.transforms.runner.selectionChanged")}
            </p>
          </div>
        )}

        {runnerResult !== null && (
          <div className="px-4 py-3 space-y-3">
            <div>
              <p className="text-xs text-mid-gray mb-1">
                {t("settings.transforms.runner.result")}
              </p>
              <pre className="text-sm text-text whitespace-pre-wrap break-words bg-mid-gray/5 border border-mid-gray/20 rounded-lg p-3">
                {runnerResult}
              </pre>
            </div>

            {diff && (
              <div>
                <p className="text-xs text-mid-gray mb-1">
                  {t("settings.transforms.diff.title")}
                </p>
                <p className="text-xs text-mid-gray">
                  {t("settings.transforms.diff.inserted", {
                    count: diff.inserted_words,
                  })}{" "}
                  ·{" "}
                  {t("settings.transforms.diff.removed", {
                    count: diff.removed_words,
                  })}
                </p>
                <div className="text-sm text-text whitespace-pre-wrap break-words bg-mid-gray/5 border border-mid-gray/20 rounded-lg p-3">
                  {diff.identical ? (
                    t("settings.transforms.diff.identical")
                  ) : (
                    diff.segments.map((segment, index) => (
                      <span
                        key={index}
                        className={
                          segment.kind === "inserted"
                            ? "text-logo-primary"
                            : segment.kind === "removed"
                              ? "text-error line-through"
                              : ""
                        }
                      >
                        {segment.text}
                      </span>
                    ))
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </SettingsGroup>

      <Dialog
        open={form !== null}
        title={
          editingId
            ? t("settings.transforms.actions.edit")
            : t("settings.transforms.actions.add")
        }
        description={t("settings.transforms.description")}
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
              {t("settings.transforms.actions.cancel")}
            </Button>
            <Button onClick={() => void handleSave()} disabled={isMutating}>
              {t("settings.transforms.actions.save")}
            </Button>
          </>
        }
      >
        {form && (
          <div className="space-y-3">
            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.transforms.form.name")}
              </label>
              <Input
                type="text"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder={t("settings.transforms.form.namePlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.transforms.form.prompt")}
              </label>
              <Textarea
                value={form.prompt}
                onChange={(e) => setForm({ ...form, prompt: e.target.value })}
                placeholder={t("settings.transforms.form.promptPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.transforms.form.provider")}
              </label>
              <Dropdown
                options={providerOptions}
                selectedValue={form.provider_id}
                onSelect={(value) => setForm({ ...form, provider_id: value })}
                placeholder={t("settings.transforms.form.providerDefault")}
                className="min-w-[200px]"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.transforms.form.model")}
              </label>
              <Input
                type="text"
                value={form.model}
                onChange={(e) => setForm({ ...form, model: e.target.value })}
                placeholder={t("settings.transforms.form.modelPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.transforms.form.category")}
              </label>
              <Input
                type="text"
                value={form.category}
                onChange={(e) => setForm({ ...form, category: e.target.value })}
                placeholder={t("settings.transforms.form.categoryPlaceholder")}
                variant="compact"
              />
            </div>

            <ToggleSwitch
              checked={form.enabled}
              onChange={(checked) => setForm({ ...form, enabled: checked })}
              label={t("settings.transforms.list.enabled")}
              description={t("settings.transforms.list.enabledDescription")}
              grouped={true}
            />
          </div>
        )}
      </Dialog>
    </div>
  );
};
