import React from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, Pencil, Trash2 } from "lucide-react";
import { ToggleSwitch } from "@/components/ui";
import Badge from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import type { DictionaryEntry } from "./types";

interface DictionaryEntryRowProps {
  entry: DictionaryEntry;
  onToggle: (entry: DictionaryEntry, enabled: boolean) => void;
  onEdit: (entry: DictionaryEntry) => void;
  onDelete: (entry: DictionaryEntry) => void;
  busy?: boolean;
}

const DictionaryEntryRowComponent: React.FC<DictionaryEntryRowProps> = ({
  entry,
  onToggle,
  onEdit,
  onDelete,
  busy = false,
}) => {
  const { t } = useTranslation();

  return (
    <div className="px-4 py-3 flex items-start justify-between gap-3">
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm font-medium text-text break-words">
            {entry.term}
          </span>
          {entry.kind === "substitution" ? (
            <>
              <ArrowRight className="w-3.5 h-3.5 text-mid-gray shrink-0" />
              <span className="text-sm text-mid-gray break-words">
                {entry.replacement}
              </span>
            </>
          ) : (
            <Badge variant="secondary" className="px-2 py-0.5">
              {t("settings.dictionary.entry.kind.term")}
            </Badge>
          )}
          {entry.language && (
            <Badge variant="secondary" className="px-2 py-0.5">
              {entry.language}
            </Badge>
          )}
        </div>
        <div className="flex items-center gap-3 flex-wrap text-xs text-mid-gray">
          <span>
            {t("settings.dictionary.entry.hits", { count: entry.hit_count })}
          </span>
          {entry.whole_word && (
            <span>{t("settings.dictionary.entry.wholeWord")}</span>
          )}
          {entry.case_sensitive && (
            <span>{t("settings.dictionary.entry.caseSensitive")}</span>
          )}
        </div>
      </div>

      <div className="flex items-center gap-2 shrink-0">
        <ToggleSwitch
          checked={entry.enabled}
          onChange={(checked) => onToggle(entry, checked)}
          label={t("settings.dictionary.entry.enabled")}
          description={t("settings.dictionary.entry.enabledDescription")}
          grouped={true}
          disabled={busy}
        />
        <Button
          variant="secondary"
          size="sm"
          onClick={() => onEdit(entry)}
          disabled={busy}
          aria-label={t("settings.dictionary.entry.edit")}
          title={t("settings.dictionary.entry.edit")}
          className="flex items-center"
        >
          <Pencil className="w-3.5 h-3.5" />
        </Button>
        <Button
          variant="danger-ghost"
          size="sm"
          onClick={() => onDelete(entry)}
          disabled={busy}
          aria-label={t("settings.dictionary.entry.delete")}
          title={t("settings.dictionary.entry.delete")}
          className="flex items-center"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </Button>
      </div>
    </div>
  );
};

export const DictionaryEntryRow = React.memo(DictionaryEntryRowComponent);
DictionaryEntryRow.displayName = "DictionaryEntryRow";
