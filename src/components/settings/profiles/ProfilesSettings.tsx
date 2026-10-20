import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import {
  commands,
  type CleanupLevel,
  type EffectiveProfileInfo,
  type FormatRule,
  type ForegroundAppInfo,
  type ProfileOverride,
  type WritingProfile,
  type WritingStyle,
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

const CLEANUP_LEVELS: CleanupLevel[] = ["none", "light", "medium"];

const FORMAT_RULES: FormatRule[] = [
  "capitalizeSentences",
  "ensureFinalPeriod",
  "collapseSpaces",
  "spaceBeforePunctuation",
];

/** Sentinel used by the dropdowns for "no selection" and "inherit". */
const NONE = "";

const parseAppNames = (value: string): string[] =>
  value
    .split(",")
    .map((name) => name.trim().toLowerCase())
    .filter((name) => name.length > 0);

interface StyleFormState {
  name: string;
  base_prompt: string;
  cleanup_level: CleanupLevel;
  ai_cleanup: boolean;
  format_rules: FormatRule[];
  enabled: boolean;
}

interface ProfileFormState {
  name: string;
  app_names: string;
  style_id: string;
  language: string;
  enabled: boolean;
}

const EMPTY_STYLE_FORM: StyleFormState = {
  name: "",
  base_prompt: "",
  cleanup_level: "light",
  ai_cleanup: false,
  format_rules: [],
  enabled: true,
};

const EMPTY_PROFILE_FORM: ProfileFormState = {
  name: "",
  app_names: "",
  style_id: "",
  language: "",
  enabled: true,
};

export const ProfilesSettings: React.FC = () => {
  const { t } = useTranslation();

  const [styles, setStyles] = useState<WritingStyle[]>([]);
  const [profiles, setProfiles] = useState<WritingProfile[]>([]);
  const [overrides, setOverrides] = useState<ProfileOverride[]>([]);
  const [foregroundApp, setForegroundApp] = useState<ForegroundAppInfo | null>(
    null,
  );
  const [effective, setEffective] = useState<EffectiveProfileInfo | null>(null);
  const [manualOverride, setManualOverride] = useState<string | null>(null);

  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);

  const [styleForm, setStyleForm] = useState<StyleFormState | null>(null);
  const [editingStyleId, setEditingStyleId] = useState<string | null>(null);

  const [profileForm, setProfileForm] = useState<ProfileFormState | null>(null);
  const [editingProfileId, setEditingProfileId] = useState<string | null>(null);

  const [overrideProfileId, setOverrideProfileId] = useState<string>("");
  const [overrideAppName, setOverrideAppName] = useState<string>("");
  const [overrideStyleId, setOverrideStyleId] = useState<string>(NONE);
  const [overrideCleanup, setOverrideCleanup] = useState<string>(NONE);

  const loadAll = useCallback(async () => {
    setIsLoading(true);
    try {
      const [stylesResult, profilesResult, overridesResult, appResult] =
        await Promise.all([
          commands.listWritingStyles(),
          commands.listWritingProfiles(),
          commands.listProfileOverrides(),
          commands.getForegroundApp(),
        ]);

      if (stylesResult.status === "ok") {
        setStyles(stylesResult.data);
      } else {
        toast.error(stylesResult.error);
      }

      if (profilesResult.status === "ok") {
        setProfiles(profilesResult.data);
      } else {
        toast.error(profilesResult.error);
      }

      if (overridesResult.status === "ok") {
        setOverrides(overridesResult.data);
      } else {
        toast.error(overridesResult.error);
      }

      if (appResult.status === "ok") {
        setForegroundApp(appResult.data);
      } else {
        toast.error(appResult.error);
      }
    } catch (error) {
      console.error("Failed to load writing profiles:", error);
      toast.error(t("settings.profiles.errors.load"));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  const resolveEffective = useCallback(
    async (appName: string | null, override: string | null) => {
      try {
        const result = await commands.resolveWritingProfile(appName, override);
        if (result.status === "ok") {
          setEffective(result.data);
        } else {
          toast.error(result.error);
        }
      } catch (error) {
        console.error("Failed to resolve the writing profile:", error);
        toast.error(t("settings.profiles.errors.action"));
      }
    },
    [t],
  );

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  useEffect(() => {
    void resolveEffective(foregroundApp?.process_name ?? null, manualOverride);
  }, [foregroundApp, manualOverride, resolveEffective]);

  const styleOptions = useMemo(
    () => styles.map((style) => ({ value: style.id, label: style.name })),
    [styles],
  );

  const profileOptions = useMemo(
    () => profiles.map((profile) => ({ value: profile.id, label: profile.name })),
    [profiles],
  );

  const styleNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const style of styles) {
      map.set(style.id, style.name);
    }
    return map;
  }, [styles]);

  const profileNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const profile of profiles) {
      map.set(profile.id, profile.name);
    }
    return map;
  }, [profiles]);

  const cleanupLabel = useCallback(
    (level: CleanupLevel) => t(`settings.profiles.cleanup.${level}`),
    [t],
  );

  const ruleLabel = useCallback(
    (rule: FormatRule) => t(`settings.profiles.rule.${rule}`),
    [t],
  );

  const originLabel = useCallback(
    (origin: EffectiveProfileInfo["origin"]) =>
      t(`settings.profiles.origin.${origin}`),
    [t],
  );

  const handleStartCreateStyle = () => {
    setEditingStyleId(null);
    setStyleForm({ ...EMPTY_STYLE_FORM });
  };

  const handleStartEditStyle = (style: WritingStyle) => {
    setEditingStyleId(style.id);
    setStyleForm({
      name: style.name,
      base_prompt: style.base_prompt,
      cleanup_level: style.cleanup_level,
      ai_cleanup: style.ai_cleanup,
      format_rules: [...style.format_rules],
      enabled: style.enabled,
    });
  };

  const handleSaveStyle = async () => {
    if (!styleForm) return;
    if (!styleForm.name.trim()) {
      toast.error(t("settings.profiles.form.required"));
      return;
    }

    setIsMutating(true);
    try {
      const payload = {
        name: styleForm.name.trim(),
        base_prompt: styleForm.base_prompt.trim(),
        cleanup_level: styleForm.cleanup_level,
        ai_cleanup: styleForm.ai_cleanup,
        format_rules: styleForm.format_rules,
        enabled: styleForm.enabled,
      };

      const result = editingStyleId
        ? await commands.updateWritingStyle(editingStyleId, payload)
        : await commands.addWritingStyle(payload);

      if (result.status === "ok") {
        setStyleForm(null);
        setEditingStyleId(null);
        await loadAll();
        toast.success(t("settings.profiles.actions.save"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to save the writing style:", error);
      toast.error(t("settings.profiles.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleDeleteStyle = async (style: WritingStyle) => {
    if (style.kind === "builtin") return;

    setIsMutating(true);
    try {
      const result = await commands.deleteWritingStyle(style.id);
      if (result.status === "ok") {
        await loadAll();
        toast.success(t("settings.profiles.actions.delete"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to delete the writing style:", error);
      toast.error(t("settings.profiles.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleStartCreateProfile = () => {
    setEditingProfileId(null);
    setProfileForm({
      ...EMPTY_PROFILE_FORM,
      style_id: styleOptions[0]?.value ?? "",
    });
  };

  const handleStartEditProfile = (profile: WritingProfile) => {
    setEditingProfileId(profile.id);
    setProfileForm({
      name: profile.name,
      app_names: profile.app_names.join(", "),
      style_id: profile.style_id,
      language: profile.language ?? "",
      enabled: profile.enabled,
    });
  };

  const handleSaveProfile = async () => {
    if (!profileForm) return;
    if (!profileForm.name.trim()) {
      toast.error(t("settings.profiles.form.required"));
      return;
    }
    if (!profileForm.style_id) {
      toast.error(t("settings.profiles.form.required"));
      return;
    }

    setIsMutating(true);
    try {
      const existing = editingProfileId
        ? profiles.find((profile) => profile.id === editingProfileId)
        : undefined;

      const payload = {
        name: profileForm.name.trim(),
        app_names: parseAppNames(profileForm.app_names),
        language: profileForm.language.trim() || null,
        style_id: profileForm.style_id,
        cleanup_level: existing?.cleanup_level ?? null,
        ai_cleanup: existing?.ai_cleanup ?? null,
        format_rules: existing?.format_rules ?? null,
        enabled: profileForm.enabled,
      };

      const result = editingProfileId
        ? await commands.updateWritingProfile(editingProfileId, payload)
        : await commands.addWritingProfile(payload);

      if (result.status === "ok") {
        setProfileForm(null);
        setEditingProfileId(null);
        await loadAll();
        toast.success(t("settings.profiles.actions.save"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to save the writing profile:", error);
      toast.error(t("settings.profiles.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleDeleteProfile = async (profile: WritingProfile) => {
    if (profile.kind === "builtin") return;

    setIsMutating(true);
    try {
      const result = await commands.deleteWritingProfile(profile.id);
      if (result.status === "ok") {
        await loadAll();
        toast.success(t("settings.profiles.actions.delete"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to delete the writing profile:", error);
      toast.error(t("settings.profiles.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleAddOverride = async () => {
    if (!overrideProfileId || !overrideAppName.trim()) {
      toast.error(t("settings.profiles.form.required"));
      return;
    }

    setIsMutating(true);
    try {
      const result = await commands.addProfileOverride(
        overrideProfileId,
        overrideAppName.trim().toLowerCase(),
        overrideStyleId === NONE ? null : overrideStyleId,
        overrideCleanup === NONE
          ? null
          : (overrideCleanup as CleanupLevel),
      );

      if (result.status === "ok") {
        setOverrideAppName("");
        setOverrideStyleId(NONE);
        setOverrideCleanup(NONE);
        await loadAll();
        toast.success(t("settings.profiles.actions.save"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to add the application rule:", error);
      toast.error(t("settings.profiles.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const handleDeleteOverride = async (override: ProfileOverride) => {
    setIsMutating(true);
    try {
      const result = await commands.deleteProfileOverride(override.id);
      if (result.status === "ok") {
        await loadAll();
        toast.success(t("settings.profiles.actions.delete"));
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      console.error("Failed to delete the application rule:", error);
      toast.error(t("settings.profiles.errors.action"));
    } finally {
      setIsMutating(false);
    }
  };

  const toggleRule = (rule: FormatRule) => {
    setStyleForm((current) => {
      if (!current) return current;
      const has = current.format_rules.includes(rule);
      return {
        ...current,
        format_rules: has
          ? current.format_rules.filter((item) => item !== rule)
          : [...current.format_rules, rule],
      };
    });
  };

  const manualOverrideOptions = useMemo(
    () => [
      { value: NONE, label: t("settings.profiles.manual.none") },
      ...profiles.map((profile) => ({
        value: profile.id,
        label: profile.name,
      })),
    ],
    [profiles, t],
  );

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.profiles.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.profiles.description")}
        </p>
      </div>

      <SettingsGroup
        title={t("settings.profiles.detected.title")}
        description={t("settings.profiles.detected.description")}
      >
        <SettingContainer
          title={t("settings.profiles.detected.title")}
          description={t("settings.profiles.detected.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex items-center gap-2">
            <span className="text-sm text-text">
              {foregroundApp
                ? foregroundApp.process_name
                : t("settings.profiles.detected.unknown")}
            </span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void loadAll()}
              disabled={isLoading}
              aria-label={t("settings.profiles.detected.refresh")}
              title={t("settings.profiles.detected.refresh")}
              className="flex items-center"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </Button>
          </div>
        </SettingContainer>

        {foregroundApp?.window_title && (
          <SettingContainer
            title={t("settings.profiles.detected.window")}
            description={t("settings.profiles.detected.window")}
            descriptionMode="inline"
            grouped={true}
          >
            <span className="text-sm text-text break-words text-end">
              {foregroundApp.window_title}
            </span>
          </SettingContainer>
        )}

        <SettingContainer
          title={t("settings.profiles.effective.title")}
          description={t("settings.profiles.effective.description")}
          descriptionMode="inline"
          grouped={true}
        >
          {effective ? (
            <dl className="space-y-1 text-sm">
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.form.profile")}
                </dt>
                <dd className="font-medium text-text">
                  {effective.profile_name}
                </dd>
              </div>
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.form.style")}
                </dt>
                <dd className="font-medium text-text">
                  {effective.style_name}
                </dd>
              </div>
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.form.cleanupLevel")}
                </dt>
                <dd className="font-medium text-text">
                  {cleanupLabel(effective.cleanup_level)}
                </dd>
              </div>
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.form.aiCleanup")}
                </dt>
                <dd className="font-medium text-text">
                  {effective.ai_cleanup
                    ? t("settings.profiles.actions.enabled")
                    : t("settings.profiles.actions.disabled")}
                </dd>
              </div>
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.form.formatRules")}
                </dt>
                <dd className="font-medium text-text text-end">
                  {effective.format_rules.length === 0
                    ? t("settings.profiles.styles.noRules")
                    : effective.format_rules
                        .map((rule) => ruleLabel(rule))
                        .join(", ")}
                </dd>
              </div>
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.form.language")}
                </dt>
                <dd className="font-medium text-text">
                  {effective.language ?? "—"}
                </dd>
              </div>
              <div className="flex items-center gap-2 flex-wrap justify-end">
                <dt className="text-mid-gray">
                  {t("settings.profiles.effective.origin")}
                </dt>
                <dd className="font-medium text-text">
                  {originLabel(effective.origin)}
                </dd>
              </div>
            </dl>
          ) : (
            <span className="text-sm text-mid-gray">
              {t("settings.profiles.detected.unknown")}
            </span>
          )}
        </SettingContainer>

        <SettingContainer
          title={t("settings.profiles.manual.title")}
          description={t("settings.profiles.manual.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex flex-col items-end gap-1">
            <Dropdown
              options={manualOverrideOptions}
              selectedValue={manualOverride ?? NONE}
              onSelect={(value) => setManualOverride(value === NONE ? null : value)}
              placeholder={t("settings.profiles.manual.none")}
              disabled={isLoading}
              className="min-w-[200px]"
            />
            <p className="text-xs text-mid-gray text-end max-w-[280px]">
              {t("settings.profiles.manual.notPersisted")}
            </p>
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.profiles.styles.title")}
        description={t("settings.profiles.styles.description")}
      >
        <div className="px-4 py-3 flex items-center justify-end">
          <Button
            variant="primary"
            size="md"
            onClick={handleStartCreateStyle}
            disabled={isLoading}
            className="flex items-center gap-2"
          >
            <Plus className="w-4 h-4" />
            {t("settings.profiles.actions.add")}
          </Button>
        </div>

        {isLoading ? (
          <div className="flex items-center justify-center py-10">
            <div className="w-6 h-6 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
          </div>
        ) : styles.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.profiles.styles.empty")}
          </div>
        ) : (
          styles.map((style) => (
            <div
              key={style.id}
              className="px-4 py-3 flex items-start justify-between gap-3"
            >
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-medium text-text break-words">
                    {style.name}
                  </span>
                  {style.kind === "builtin" && (
                    <Badge variant="secondary" className="px-2 py-0.5">
                      {t("settings.profiles.styles.builtin")}
                    </Badge>
                  )}
                  <Badge variant="secondary" className="px-2 py-0.5">
                    {style.enabled
                      ? t("settings.profiles.actions.enabled")
                      : t("settings.profiles.actions.disabled")}
                  </Badge>
                </div>
                <div className="flex items-center gap-3 flex-wrap text-xs text-mid-gray">
                  <span>
                    {t("settings.profiles.form.cleanupLevel")}:{" "}
                    {cleanupLabel(style.cleanup_level)}
                  </span>
                  <span>
                    {t("settings.profiles.form.aiCleanup")}:{" "}
                    {style.ai_cleanup
                      ? t("settings.profiles.actions.enabled")
                      : t("settings.profiles.actions.disabled")}
                  </span>
                  <span>
                    {t("settings.profiles.form.formatRules")}:{" "}
                    {style.format_rules.length === 0
                      ? t("settings.profiles.styles.noRules")
                      : style.format_rules
                          .map((rule) => ruleLabel(rule))
                          .join(", ")}
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleStartEditStyle(style)}
                  disabled={isMutating}
                  aria-label={t("settings.profiles.actions.edit")}
                  title={t("settings.profiles.actions.edit")}
                  className="flex items-center"
                >
                  <Pencil className="w-3.5 h-3.5" />
                </Button>
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={() => void handleDeleteStyle(style)}
                  disabled={isMutating || style.kind === "builtin"}
                  aria-label={t("settings.profiles.actions.delete")}
                  title={
                    style.kind === "builtin"
                      ? t("settings.profiles.actions.builtinCannotDelete")
                      : t("settings.profiles.actions.delete")
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
        title={t("settings.profiles.profiles.title")}
        description={t("settings.profiles.profiles.description")}
      >
        <div className="px-4 py-3 flex items-center justify-end">
          <Button
            variant="primary"
            size="md"
            onClick={handleStartCreateProfile}
            disabled={isLoading || styles.length === 0}
            className="flex items-center gap-2"
          >
            <Plus className="w-4 h-4" />
            {t("settings.profiles.actions.add")}
          </Button>
        </div>

        {profiles.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.profiles.profiles.empty")}
          </div>
        ) : (
          profiles.map((profile) => (
            <div
              key={profile.id}
              className="px-4 py-3 flex items-start justify-between gap-3"
            >
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-medium text-text break-words">
                    {profile.name}
                  </span>
                  <Badge variant="secondary" className="px-2 py-0.5">
                    {profile.enabled
                      ? t("settings.profiles.actions.enabled")
                      : t("settings.profiles.actions.disabled")}
                  </Badge>
                </div>
                <div className="flex items-center gap-3 flex-wrap text-xs text-mid-gray">
                  <span>
                    {t("settings.profiles.form.applications")}:{" "}
                    {profile.app_names.length === 0
                      ? t("settings.profiles.profiles.everywhere")
                      : profile.app_names.join(", ")}
                  </span>
                  <span>
                    {t("settings.profiles.form.style")}:{" "}
                    {styleNameById.get(profile.style_id) ?? profile.style_id}
                  </span>
                  {profile.language && (
                    <span>
                      {t("settings.profiles.form.language")}: {profile.language}
                    </span>
                  )}
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleStartEditProfile(profile)}
                  disabled={isMutating}
                  aria-label={t("settings.profiles.actions.edit")}
                  title={t("settings.profiles.actions.edit")}
                  className="flex items-center"
                >
                  <Pencil className="w-3.5 h-3.5" />
                </Button>
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={() => void handleDeleteProfile(profile)}
                  disabled={isMutating || profile.kind === "builtin"}
                  aria-label={t("settings.profiles.actions.delete")}
                  title={
                    profile.kind === "builtin"
                      ? t("settings.profiles.actions.builtinCannotDelete")
                      : t("settings.profiles.actions.delete")
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
        title={t("settings.profiles.overrides.title")}
        description={t("settings.profiles.overrides.description")}
      >
        <SettingContainer
          title={t("settings.profiles.overrides.title")}
          description={t("settings.profiles.overrides.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <div className="flex flex-wrap items-center justify-end gap-2">
            <Dropdown
              options={profileOptions}
              selectedValue={overrideProfileId || null}
              onSelect={setOverrideProfileId}
              placeholder={t("settings.profiles.form.profile")}
              disabled={isMutating}
              className="min-w-[160px]"
            />
            <Input
              type="text"
              value={overrideAppName}
              onChange={(e) => setOverrideAppName(e.target.value)}
              placeholder={t(
                "settings.profiles.form.overrideApplicationPlaceholder",
              )}
              variant="compact"
              className="min-w-[160px]"
            />
            <Dropdown
              options={[
                { value: NONE, label: t("settings.profiles.overrides.inherit") },
                ...styleOptions,
              ]}
              selectedValue={overrideStyleId}
              onSelect={setOverrideStyleId}
              placeholder={t("settings.profiles.overrides.inherit")}
              disabled={isMutating}
              className="min-w-[160px]"
            />
            <Dropdown
              options={[
                { value: NONE, label: t("settings.profiles.overrides.inherit") },
                ...CLEANUP_LEVELS.map((level) => ({
                  value: level,
                  label: cleanupLabel(level),
                })),
              ]}
              selectedValue={overrideCleanup}
              onSelect={setOverrideCleanup}
              placeholder={t("settings.profiles.overrides.inherit")}
              disabled={isMutating}
              className="min-w-[160px]"
            />
            <Button
              variant="primary"
              size="md"
              onClick={() => void handleAddOverride()}
              disabled={isMutating}
            >
              {t("settings.profiles.actions.add")}
            </Button>
          </div>
        </SettingContainer>

        {overrides.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.profiles.overrides.empty")}
          </div>
        ) : (
          overrides.map((override) => (
            <div
              key={override.id}
              className="px-4 py-3 flex items-start justify-between gap-3"
            >
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-medium text-text break-words">
                    {override.scope_value}
                  </span>
                </div>
                <div className="flex items-center gap-3 flex-wrap text-xs text-mid-gray">
                  <span>
                    {t("settings.profiles.form.profile")}:{" "}
                    {profileNameById.get(override.profile_id) ??
                      override.profile_id}
                  </span>
                  <span>
                    {t("settings.profiles.form.style")}:{" "}
                    {override.style_id
                      ? (styleNameById.get(override.style_id) ??
                        override.style_id)
                      : t("settings.profiles.overrides.inherit")}
                  </span>
                  <span>
                    {t("settings.profiles.form.cleanupLevel")}:{" "}
                    {override.cleanup_level
                      ? cleanupLabel(override.cleanup_level)
                      : t("settings.profiles.overrides.inherit")}
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={() => void handleDeleteOverride(override)}
                  disabled={isMutating}
                  aria-label={t("settings.profiles.actions.delete")}
                  title={t("settings.profiles.actions.delete")}
                  className="flex items-center"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </Button>
              </div>
            </div>
          ))
        )}
      </SettingsGroup>

      <Dialog
        open={styleForm !== null}
        title={
          editingStyleId
            ? t("settings.profiles.actions.edit")
            : t("settings.profiles.actions.add")
        }
        description={t("settings.profiles.styles.description")}
        onOpenChange={(open) => {
          if (!open) {
            setStyleForm(null);
            setEditingStyleId(null);
          }
        }}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button
              variant="secondary"
              onClick={() => {
                setStyleForm(null);
                setEditingStyleId(null);
              }}
            >
              {t("settings.profiles.actions.cancel")}
            </Button>
            <Button onClick={() => void handleSaveStyle()} disabled={isMutating}>
              {t("settings.profiles.actions.save")}
            </Button>
          </>
        }
      >
        {styleForm && (
          <div className="space-y-3">
            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.name")}
              </label>
              <Input
                type="text"
                value={styleForm.name}
                onChange={(e) =>
                  setStyleForm({ ...styleForm, name: e.target.value })
                }
                placeholder={t("settings.profiles.form.namePlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.basePrompt")}
              </label>
              <Textarea
                value={styleForm.base_prompt}
                onChange={(e) =>
                  setStyleForm({ ...styleForm, base_prompt: e.target.value })
                }
                placeholder={t("settings.profiles.form.basePromptPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.cleanupLevel")}
              </label>
              <Dropdown
                options={CLEANUP_LEVELS.map((level) => ({
                  value: level,
                  label: cleanupLabel(level),
                }))}
                selectedValue={styleForm.cleanup_level}
                onSelect={(value) =>
                  setStyleForm({
                    ...styleForm,
                    cleanup_level: value as CleanupLevel,
                  })
                }
                className="min-w-[200px]"
              />
            </div>

            <ToggleSwitch
              checked={styleForm.ai_cleanup}
              onChange={(checked) =>
                setStyleForm({ ...styleForm, ai_cleanup: checked })
              }
              label={t("settings.profiles.form.aiCleanup")}
              description={t("settings.profiles.styles.aiCleanupAllowed")}
              grouped={true}
            />

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.formatRules")}
              </label>
              <div className="flex flex-col gap-1">
                {FORMAT_RULES.map((rule) => (
                  <label
                    key={rule}
                    className="flex items-center gap-2 text-sm text-text cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      className="accent-logo-primary"
                      checked={styleForm.format_rules.includes(rule)}
                      onChange={() => toggleRule(rule)}
                    />
                    {ruleLabel(rule)}
                  </label>
                ))}
              </div>
            </div>

            <ToggleSwitch
              checked={styleForm.enabled}
              onChange={(checked) =>
                setStyleForm({ ...styleForm, enabled: checked })
              }
              label={t("settings.profiles.actions.enabled")}
              description={t("settings.profiles.form.enabledDescription")}
              grouped={true}
            />
          </div>
        )}
      </Dialog>

      <Dialog
        open={profileForm !== null}
        title={
          editingProfileId
            ? t("settings.profiles.actions.edit")
            : t("settings.profiles.actions.add")
        }
        description={t("settings.profiles.profiles.description")}
        onOpenChange={(open) => {
          if (!open) {
            setProfileForm(null);
            setEditingProfileId(null);
          }
        }}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button
              variant="secondary"
              onClick={() => {
                setProfileForm(null);
                setEditingProfileId(null);
              }}
            >
              {t("settings.profiles.actions.cancel")}
            </Button>
            <Button onClick={() => void handleSaveProfile()} disabled={isMutating}>
              {t("settings.profiles.actions.save")}
            </Button>
          </>
        }
      >
        {profileForm && (
          <div className="space-y-3">
            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.name")}
              </label>
              <Input
                type="text"
                value={profileForm.name}
                onChange={(e) =>
                  setProfileForm({ ...profileForm, name: e.target.value })
                }
                placeholder={t("settings.profiles.form.namePlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.applications")}
              </label>
              <Input
                type="text"
                value={profileForm.app_names}
                onChange={(e) =>
                  setProfileForm({ ...profileForm, app_names: e.target.value })
                }
                placeholder={t("settings.profiles.form.applicationsPlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.style")}
              </label>
              <Dropdown
                options={styleOptions}
                selectedValue={profileForm.style_id || null}
                onSelect={(value) =>
                  setProfileForm({ ...profileForm, style_id: value })
                }
                placeholder={t("settings.profiles.form.style")}
                className="min-w-[200px]"
              />
            </div>

            <div className="space-y-1 flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.profiles.form.language")}
              </label>
              <Input
                type="text"
                value={profileForm.language}
                onChange={(e) =>
                  setProfileForm({ ...profileForm, language: e.target.value })
                }
                placeholder={t("settings.profiles.form.languagePlaceholder")}
                variant="compact"
              />
            </div>

            <ToggleSwitch
              checked={profileForm.enabled}
              onChange={(checked) =>
                setProfileForm({ ...profileForm, enabled: checked })
              }
              label={t("settings.profiles.actions.enabled")}
              description={t("settings.profiles.form.enabledDescription")}
              grouped={true}
            />
          </div>
        )}
      </Dialog>
    </div>
  );
};
