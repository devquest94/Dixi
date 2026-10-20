import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Wand2 } from "lucide-react";
import { Textarea } from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { applyDictionarySubstitutions } from "./api";

export const DictionaryPreview: React.FC = () => {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [result, setResult] = useState<string | null>(null);
  const [isApplying, setIsApplying] = useState(false);

  const handleApply = async () => {
    if (!text.trim()) return;

    setIsApplying(true);
    try {
      const response = await applyDictionarySubstitutions(text);
      if (response.status === "ok") {
        setResult(response.data);
      } else {
        toast.error(response.error);
      }
    } catch (error) {
      console.error("Failed to apply dictionary substitutions:", error);
      toast.error(t("settings.dictionary.preview.error"));
    } finally {
      setIsApplying(false);
    }
  };

  return (
    <div className="space-y-3 w-full">
      <div className="space-y-2 flex flex-col">
        <label
          className="text-sm font-semibold"
          htmlFor="dictionary-preview-input"
        >
          {t("settings.dictionary.preview.inputLabel")}
        </label>
        <Textarea
          id="dictionary-preview-input"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={t("settings.dictionary.preview.inputPlaceholder")}
          disabled={isApplying}
        />
        <div className="flex items-center gap-2">
          <Button
            onClick={handleApply}
            disabled={isApplying || !text.trim()}
            className="flex items-center gap-2"
          >
            <Wand2 className="w-4 h-4" />
            {t("settings.dictionary.preview.apply")}
          </Button>
        </div>
      </div>

      {result !== null && (
        <div className="space-y-1">
          <p className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("settings.dictionary.preview.resultLabel")}
          </p>
          <p className="text-sm text-text bg-mid-gray/10 border border-mid-gray/20 rounded-md px-3 py-2 whitespace-pre-wrap break-words">
            {result}
          </p>
        </div>
      )}

      <p className="text-xs text-mid-gray">
        {t("settings.dictionary.preview.hint")}
      </p>
    </div>
  );
};
