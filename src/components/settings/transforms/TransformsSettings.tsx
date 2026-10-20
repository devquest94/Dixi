import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import { Copy, Pencil, Plus, Trash2 } from "lucide-react";
import {
  commands,
  type SelectionContext,
  type SelectionFailure,
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

// La cuenta atras le da al usuario tiempo para volver a la aplicacion de
// origen y seleccionar el texto antes de que Dixi lo capture.
const CAPTURE_COUNTDOWN_SECONDS = 4;

// El backend manda el motivo de un fallo con un codigo estable en
// snake_case ("nothing_selected") y la interfaz lo traduce con una clave en
// camelCase ("nothingSelected"). Un codigo que no esta en la lista cae en el
// error generico de escritura.
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
] as const;

const selectionErrorKey = (code: string): string => {
  const camel = code.replace(/_([a-z])/g, (_match, letter: string) =>
    letter.toUpperCase(),
  );
  return (SELECTION_ERROR_KEYS as readonly string[]).includes(camel)
    ? camel
    : "write";
};

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
  const { settings, getSetting } = useSettings();

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
  const [captured, setCaptured] = useState<SelectionContext | null>(null);
  const [countdown, setCountdown] = useState<number | null>(null);
  const [selectionChanged, setSelectionChanged] = useState(false);

  // El atajo global de captura: es la via principal, porque no le quita el
  // foco a la aplicacion de origen.
  const captureShortcut = getSetting("bindings")?.capture_selection?.current_binding;

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

  /**
   * A capture arrives from outside: the global shortcut does not take the
   * focus away from the source application, so the text is read there and
   * then told to the interface. Here it is only reflected.
   */
  const applyCapture = useCallback((context: SelectionContext) => {
    setCaptured(context);
    setSelectionChanged(false);
    setRunnerText(context.text);
    setRunnerResult(null);
    setDiff(null);
  }, []);

  /**
   * The alternative to the global shortcut: capture after a countdown, so
   * the user has time to go back to the source application and select the
   * text before Dixi reads it.
   */
  const performCapture = useCallback(async () => {
    try {
      const result = await commands.captureSelection();
      if (result.status === "ok") {
        if (!result.data.text) {
          toast.error(t("settings.selection.errors.nothingSelected"));
          return;
        }
        applyCapture(result.data);
      } else {
        toast.error(
          t(`settings.selection.errors.${selectionErrorKey(result.error.code)}`),
        );
      }
    } catch (error) {
      console.error("Failed to capture the selection:", error);
      toast.error(t("settings.transforms.errors.action"));
    }
  }, [t, applyCapture]);

  // What was captured the last time, so the panel is not empty when it is
  // opened after a capture done from the global shortcut.
  useEffect(() => {
    let mounted = true;

    void commands.getCapturedSelection().then((result) => {
      if (!mounted) return;
      if (result.status === "ok") {
        if (result.data) applyCapture(result.data);
        return;
      }
      console.error(
        "Failed to read the captured selection:",
        result.error.message,
      );
      toast.error(
        t(`settings.selection.errors.${selectionErrorKey(result.error.code)}`),
      );
    });

    return () => {
      mounted = false;
    };
  }, [applyCapture]);

  // The shortcut captures and tells the interface; a failure is a message
  // the user has to see, because nothing else will happen.
  useEffect(() => {
    const unlistenCaptured = listen<SelectionContext>(
      "selection-captured",
      (event) => {
        applyCapture(event.payload);
      },
    );

    const unlistenFailed = listen<SelectionFailure>(
      "selection-capture-failed",
      (event) => {
        toast.error(
          t(`settings.selection.errors.${selectionErrorKey(event.payload.code)}`),
        );
      },
    );

    return () => {
      void unlistenCaptured.then((unlisten) => unlisten());
      void unlistenFailed.then((unlisten) => unlisten());
    };
  }, [t, applyCapture]);

  // The countdown: every second it goes down one, and at zero the capture
  // happens.
  useEffect(() => {
    if (countdown === null) return;

    if (countdown <= 0) {
      setCountdown(null);
      void performCapture();
      return;
    }

    const timer = window.setTimeout(() => {
      setCountdown((current) => (current === null ? null : current - 1));
    }, 1000);

    return () => window.clearTimeout(timer);
  }, [countdown, performCapture]);

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

    if (!captured) {
      toast.error(t("settings.transforms.runner.acceptNeedsCapture"));
      return;
    }

    // El backend escribe sobre la seleccion capturada y deja el cambio en el
    // historial, con el sitio donde se hizo: por eso la interfaz no
    // registra nada por su cuenta.
    // El identificador del transform aplicado es el del ultimo de la cadena,
    // que es el que produjo el texto que se escribe.
    const sourceId =
      selectedTransformIds.length > 0
        ? selectedTransformIds[selectedTransformIds.length - 1]
        : null;

    try {
      const result = await commands.applyToCapturedSelection(
        runnerResult,
        sourceId,
      );
      if (result.status === "ok") {
        setSelectionChanged(false);
        toast.success(t("settings.transforms.runner.accepted"));
        return;
      }

      const failure = result.error;
      if (failure.code === "selection_changed") {
        // No se escribio nada en ninguna parte: el texto se ofrece para
        // copiar, no se escribe en otro sitio.
        setSelectionChanged(true);
        toast.error(t("settings.selection.errors.selectionChanged"));
        return;
      }

      toast.error(
        t(`settings.selection.errors.${selectionErrorKey(failure.code)}`),
      );
    } catch (error) {
      console.error(
        "Failed to apply the result to the captured selection:",
        error,
      );
      toast.error(t("settings.transforms.errors.action"));
    }
  };

  const handleCopy = async () => {
    if (runnerResult === null) return;
    try {
      await navigator.clipboard.writeText(runnerResult);
      if (selectionChanged) {
        // El texto no llego a escribirse en la aplicacion: el usuario tiene
        // que pegarlo a mano, y el aviso lo dice.
        toast.warning(t("settings.selection.errors.selectionChanged"));
      }
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
        {captureShortcut && (
          <div className="px-4 py-3">
            <p className="text-sm text-mid-gray">
              {t("settings.transforms.runner.hotkeyHint", {
                shortcut: captureShortcut,
              })}
            </p>
          </div>
        )}

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
            <div className="flex items-center gap-3">
              <Button
                variant="secondary"
                size="md"
                onClick={() => setCountdown(CAPTURE_COUNTDOWN_SECONDS)}
                disabled={isRunning || countdown !== null}
              >
                {t("settings.transforms.runner.captureSelection")}
              </Button>
              {countdown !== null && (
                <span className="text-sm text-mid-gray">
                  {t("settings.transforms.runner.captureCountdown", {
                    seconds: countdown,
                  })}
                </span>
              )}
            </div>
          </div>
        </SettingContainer>

        <div className="px-4 py-3 space-y-1">
          {captured ? (
            <>
              <p className="text-xs text-mid-gray">
                {t("settings.transforms.runner.captured")}
              </p>
              <pre className="text-sm text-text whitespace-pre-wrap break-words bg-mid-gray/5 border border-mid-gray/20 rounded-lg p-3">
                {captured.text}
              </pre>
              {captured.app_name && (
                <p className="text-xs text-mid-gray">
                  {t("settings.transforms.runner.capturedFrom", {
                    app: captured.app_name,
                  })}
                </p>
              )}
            </>
          ) : (
            <p className="text-sm text-mid-gray">
              {t("settings.transforms.runner.nothingCaptured")}
            </p>
          )}
        </div>

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
                disabled={isRunning || !captured}
                title={
                  captured
                    ? undefined
                    : t("settings.transforms.runner.acceptNeedsCapture")
                }
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

        {runnerResult !== null && !captured && (
          <div className="px-4 py-3">
            <p className="text-sm text-mid-gray">
              {t("settings.transforms.runner.acceptNeedsCapture")}
            </p>
          </div>
        )}

        {selectionChanged && (
          <div className="px-4 py-3">
            <p className="text-sm text-error">
              {t("settings.selection.errors.selectionChanged")}
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
