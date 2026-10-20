import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { Eraser, RotateCcw } from "lucide-react";
import {
  commands,
  type HistoryKind,
  type TextHistoryEntry,
} from "@/bindings";
import { SettingContainer, SettingsGroup } from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import Badge from "@/components/ui/Badge";

const HISTORY_LIMIT = 200;
const PREVIEW_LENGTH = 120;

const truncate = (text: string, max: number = PREVIEW_LENGTH): string =>
  text.length > max ? `${text.slice(0, max)}…` : text;

export const TextHistorySettings: React.FC = () => {
  const { t, i18n } = useTranslation();

  const [entries, setEntries] = useState<TextHistoryEntry[]>([]);
  const [undoableId, setUndoneId] = useState<number | null>(null);
  const [retentionDays, setRetentionDays] = useState<string>("0");
  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);

  const loadHistory = useCallback(async () => {
    setIsLoading(true);
    try {
      const [listResult, undoableResult, retentionResult] = await Promise.all([
        commands.listTextHistory(HISTORY_LIMIT),
        commands.lastUndoableChange(),
        commands.getTextHistoryRetention(),
      ]);

      if (listResult.status === "ok") {
        setEntries(listResult.data);
      } else {
        toast.error(listResult.error);
      }

      if (undoableResult.status === "ok") {
        setUndoneId(undoableResult.data?.id ?? null);
      } else {
        toast.error(undoableResult.error);
      }

      if (retentionResult.status === "ok") {
        setRetentionDays(String(retentionResult.data));
      } else {
        toast.error(retentionResult.error);
      }
    } catch (error) {
      console.error("Failed to load the text history:", error);
      toast.error(t("settings.textHistory.errors.load"));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  const formatTimestamp = useCallback(
    (createdAt: number) => {
      try {
        return new Intl.DateTimeFormat(i18n.language, {
          year: "numeric",
          month: "short",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        }).format(new Date(createdAt));
      } catch (error) {
        console.error("Failed to format the date:", error);
        return new Date(createdAt).toISOString();
      }
    },
    [i18n.language],
  );

  const kindLabel = useCallback(
    (kind: HistoryKind) => t(`settings.textHistory.kind.${kind}`),
    [t],
  );

  /**
   * Reverting a change means putting the original text back where the change
   * was made. The backend has no "undo" command, so the selection flow does
   * it: the current selection is captured, checked against the text that was
   * written, and only then replaced by the original one.
   */
  const handleRevert = async (entry: TextHistoryEntry) => {
    setIsMutating(true);
    try {
      const selectionResult = await commands.captureSelection();
      if (selectionResult.status !== "ok") {
        toast.error(selectionResult.error);
        return;
      }

      if (!selectionResult.data.text) {
        toast.error(t("settings.textHistory.revert.selectionEmpty"));
        return;
      }

      if (selectionResult.data.text.trim() !== entry.result_text.trim()) {
        toast.error(t("settings.textHistory.revert.selectionMismatch"));
        return;
      }

      const replaceResult = await commands.replaceTextSelection(
        selectionResult.data.text,
        selectionResult.data.window,
        selectionResult.data.control,
        entry.original_text,
      );
      if (replaceResult.status !== "ok") {
        toast.error(replaceResult.error);
        return;
      }
      if (!replaceResult.data) {
        toast.error(t("settings.textHistory.revert.selectionChanged"));
        return;
      }

      const markResult = await commands.markTextChangeUndone(entry.id);
      if (markResult.status === "ok") {
        await loadHistory();
        toast.success(t("settings.textHistory.revert.done"));
      } else {
        toast.error(markResult.error);
      }
    } catch (error) {
      console.error("Failed to revert the change:", error);
      toast.error(t("settings.textHistory.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleMarkUndone = async (entry: TextHistoryEntry) => {
    setIsMutating(true);
    try {
      const result = await commands.markTextChangeUndone(entry.id);
      if (result.status === "ok") {
        await loadHistory();
        toast.success(t("settings.textHistory.revert.markedDone"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to mark the change as reverted:", error);
      toast.error(t("settings.textHistory.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleSaveRetention = async () => {
    // `Number` en vez de `parseInt`: "1e3" y "12abc" no son un numero
    // entero, y guardarlos como dias seria mentir sobre lo que se guarda.
    // Un campo vacio tampoco vale: cero significa "hasta que se limpie".
    const raw = retentionDays.trim();
    const days = Number(raw);
    if (raw.length === 0 || !Number.isInteger(days) || days < 0) {
      toast.error(t("settings.textHistory.retention.invalid"));
      return;
    }

    setIsMutating(true);
    try {
      const result = await commands.setTextHistoryRetention(days);
      if (result.status === "ok") {
        await loadHistory();
        toast.success(t("settings.textHistory.retention.saved"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to save the retention:", error);
      toast.error(t("settings.textHistory.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handlePurge = async () => {
    setIsMutating(true);
    try {
      const result = await commands.purgeTextHistory();
      if (result.status === "ok") {
        await loadHistory();
        toast.success(
          t("settings.textHistory.retention.purged", { count: result.data }),
        );
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to purge the text history:", error);
      toast.error(t("settings.textHistory.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleClear = async () => {
    const confirmed = await ask(t("settings.textHistory.actions.clearConfirm"), {
      title: t("settings.textHistory.actions.clearTitle"),
      kind: "warning",
    });
    if (!confirmed) return;

    setIsMutating(true);
    try {
      const result = await commands.clearTextHistory();
      if (result.status === "ok") {
        await loadHistory();
        toast.success(t("settings.textHistory.actions.cleared"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to clear the text history:", error);
      toast.error(t("settings.textHistory.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const groupedEntries = useMemo(
    () =>
      entries.map((entry) => ({
        entry,
        isUndoable: entry.id === undoableId && !entry.undone,
      })),
    [entries, undoableId],
  );

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.textHistory.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.textHistory.description")}
        </p>
      </div>

      <SettingsGroup
        title={t("settings.textHistory.list.title")}
        description={t("settings.textHistory.list.description")}
      >
        {isLoading ? (
          <div className="flex items-center justify-center py-10">
            <div className="w-6 h-6 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
          </div>
        ) : groupedEntries.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.textHistory.list.empty")}
          </div>
        ) : (
          groupedEntries.map(({ entry, isUndoable }) => (
            <div
              key={entry.id}
              className="px-4 py-3 flex items-start justify-between gap-3"
            >
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-medium text-text">
                    {formatTimestamp(entry.created_at)}
                  </span>
                  <Badge variant="secondary" className="px-2 py-0.5">
                    {kindLabel(entry.kind)}
                  </Badge>
                  {entry.undone && (
                    <Badge variant="secondary" className="px-2 py-0.5">
                      {t("settings.textHistory.list.undone")}
                    </Badge>
                  )}
                </div>
                <div className="text-xs text-mid-gray">
                  {t("settings.textHistory.list.application")}:{" "}
                  {entry.app_name ??
                    t("settings.textHistory.list.unknownApplication")}
                </div>
                <div className="text-xs text-mid-gray space-y-0.5">
                  <div className="break-words">
                    <span className="text-error line-through">
                      {truncate(entry.original_text)}
                    </span>
                  </div>
                  <div className="break-words">
                    <span className="text-logo-primary">
                      {truncate(entry.result_text)}
                    </span>
                  </div>
                </div>
              </div>

              {isUndoable && (
                <div className="flex items-center gap-2 shrink-0">
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => void handleMarkUndone(entry)}
                    disabled={isMutating}
                    className="flex items-center gap-1"
                  >
                    <Eraser className="w-3.5 h-3.5" />
                    {t("settings.textHistory.revert.markDone")}
                  </Button>
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => void handleRevert(entry)}
                    disabled={isMutating}
                    className="flex items-center gap-1"
                  >
                    <RotateCcw className="w-3.5 h-3.5" />
                    {t("settings.textHistory.revert.button")}
                  </Button>
                </div>
              )}
            </div>
          ))
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.textHistory.retention.title")}
        description={t("settings.textHistory.retention.description")}
      >
        <SettingContainer
          title={t("settings.textHistory.retention.title")}
          description={t("settings.textHistory.retention.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex items-center gap-2">
            <Input
              type="number"
              min={0}
              value={retentionDays}
              onChange={(e) => setRetentionDays(e.target.value)}
              placeholder={t("settings.textHistory.retention.days")}
              variant="compact"
              className="w-[110px]"
            />
            <Button
              variant="primary"
              size="md"
              onClick={() => void handleSaveRetention()}
              disabled={isMutating}
            >
              {t("settings.textHistory.retention.save")}
            </Button>
            <Button
              variant="secondary"
              size="md"
              onClick={() => void handlePurge()}
              disabled={isMutating}
            >
              {t("settings.textHistory.retention.purge")}
            </Button>
          </div>
        </SettingContainer>

        <div className="px-4 py-3">
          <p className="text-xs text-mid-gray">
            {t("settings.textHistory.retention.zero")}
          </p>
        </div>

        <SettingContainer
          title={t("settings.textHistory.actions.clear")}
          description={t("settings.textHistory.actions.clearDescription")}
          descriptionMode="inline"
          grouped={true}
        >
          <Button
            variant="danger"
            size="md"
            onClick={() => void handleClear()}
            disabled={isMutating || entries.length === 0}
          >
            {t("settings.textHistory.actions.clear")}
          </Button>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
