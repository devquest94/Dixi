import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, ToggleSwitch } from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import type { DictionaryEntryKind, DictionaryFormState } from "./types";

interface DictionaryEntryFormProps {
  form: DictionaryFormState;
  onChange: (form: DictionaryFormState) => void;
  onSubmit: () => void;
  onCancel?: () => void;
  submitLabel: string;
  showActions?: boolean;
  disabled?: boolean;
  termError?: string | null;
  /**
   * Prefix for the generated input ids. The form is mounted twice at the same
   * time (page + edit dialog), so the ids must stay unique.
   */
  idPrefix?: string;
}

const DictionaryEntryFormComponent: React.FC<DictionaryEntryFormProps> = ({
  form,
  onChange,
  onSubmit,
  onCancel,
  submitLabel,
  showActions = true,
  disabled = false,
  termError = null,
  idPrefix = "dictionary",
}) => {
  const { t } = useTranslation();

  const handleKindChange = (value: string) => {
    const kind = value as DictionaryEntryKind;
    onChange({
      ...form,
      kind,
      // A "term" entry never rewrites the transcript, so the replacement is
      // cleared when switching to that kind.
      replacement: kind === "term" ? "" : form.replacement,
    });
  };

  const handlePriorityChange = (value: string) => {
    const parsed = Number.parseInt(value, 10);
    onChange({ ...form, priority: Number.isNaN(parsed) ? 0 : parsed });
  };

  return (
    <div className="space-y-3">
      <div className="space-y-2 flex flex-col">
        <label className="text-sm font-semibold">
          {t("settings.dictionary.form.kindLabel")}
        </label>
        <Dropdown
          options={[
            {
              value: "term",
              label: t("settings.dictionary.form.kind.term"),
            },
            {
              value: "substitution",
              label: t("settings.dictionary.form.kind.substitution"),
            },
          ]}
          selectedValue={form.kind}
          onSelect={handleKindChange}
          disabled={disabled}
          className="max-w-xs"
        />
        <p className="text-xs text-mid-gray">
          {form.kind === "term"
            ? t("settings.dictionary.form.kindHint.term")
            : t("settings.dictionary.form.kindHint.substitution")}
        </p>
      </div>

      <div className="space-y-2 flex flex-col">
        <label className="text-sm font-semibold" htmlFor={`${idPrefix}-term`}>
          {t("settings.dictionary.form.termLabel")}
        </label>
        <Input
          id={`${idPrefix}-term`}
          type="text"
          value={form.term}
          onChange={(e) => onChange({ ...form, term: e.target.value })}
          placeholder={t("settings.dictionary.form.termPlaceholder")}
          disabled={disabled}
        />
        {termError && <p className="text-xs text-error">{termError}</p>}
      </div>

      {form.kind === "substitution" && (
        <div className="space-y-2 flex flex-col">
          <label
            className="text-sm font-semibold"
            htmlFor={`${idPrefix}-replacement`}
          >
            {t("settings.dictionary.form.replacementLabel")}
          </label>
          <Input
            id={`${idPrefix}-replacement`}
            type="text"
            value={form.replacement}
            onChange={(e) => onChange({ ...form, replacement: e.target.value })}
            placeholder={t("settings.dictionary.form.replacementPlaceholder")}
            disabled={disabled}
          />
          <p className="text-xs text-warning">
            {t("settings.dictionary.form.replacementRequired")}
          </p>
        </div>
      )}

      <div className="flex flex-wrap gap-3">
        <div className="space-y-2 flex flex-col flex-1 min-w-[180px]">
          <label
            className="text-sm font-semibold"
            htmlFor={`${idPrefix}-language`}
          >
            {t("settings.dictionary.form.languageLabel")}
          </label>
          <Input
            id={`${idPrefix}-language`}
            type="text"
            value={form.language}
            onChange={(e) => onChange({ ...form, language: e.target.value })}
            placeholder={t("settings.dictionary.form.languagePlaceholder")}
            disabled={disabled}
          />
        </div>
        <div className="space-y-2 flex flex-col w-[120px]">
          <label
            className="text-sm font-semibold"
            htmlFor={`${idPrefix}-priority`}
          >
            {t("settings.dictionary.form.priorityLabel")}
          </label>
          <Input
            id={`${idPrefix}-priority`}
            type="number"
            min={0}
            value={form.priority}
            onChange={(e) => handlePriorityChange(e.target.value)}
            placeholder={t("settings.dictionary.form.priorityPlaceholder")}
            disabled={disabled}
          />
        </div>
      </div>

      <div className="-mx-4">
        <ToggleSwitch
          checked={form.wholeWord}
          onChange={(checked) => onChange({ ...form, wholeWord: checked })}
          label={t("settings.dictionary.form.wholeWordLabel")}
          description={t("settings.dictionary.form.wholeWordDescription")}
          grouped={true}
          disabled={disabled}
        />
        <ToggleSwitch
          checked={form.caseSensitive}
          onChange={(checked) => onChange({ ...form, caseSensitive: checked })}
          label={t("settings.dictionary.form.caseSensitiveLabel")}
          description={t("settings.dictionary.form.caseSensitiveDescription")}
          grouped={true}
          disabled={disabled}
        />
      </div>

      {showActions && (
        <div className="flex gap-2 pt-1">
          <Button onClick={onSubmit} disabled={disabled}>
            {submitLabel}
          </Button>
          {onCancel && (
            <Button variant="secondary" onClick={onCancel} disabled={disabled}>
              {t("settings.dictionary.form.cancel")}
            </Button>
          )}
        </div>
      )}
    </div>
  );
};

export const DictionaryEntryForm = React.memo(DictionaryEntryFormComponent);
DictionaryEntryForm.displayName = "DictionaryEntryForm";
