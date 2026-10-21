import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { type } from "@tauri-apps/plugin-os";
import { toast } from "sonner";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Button } from "../../ui/Button";
import { ShortcutInput } from "../ShortcutInput";
import { useSettings } from "../../../hooks/useSettings";
import { commands } from "@/bindings";
import type {
  RemoteDesktopDetection,
  RemoteTransferPhase,
  RemoteTransferStatus,
} from "@/bindings";

/**
 * The phase of the transfer, as the message key that describes it.
 *
 * The key is resolved in the component so every language says the same thing
 * about the same phase.
 */
const PHASE_MESSAGE_KEY: Record<RemoteTransferPhase, string> = {
  idle: "settings.remoteDesktop.status.idle",
  ready_to_paste: "settings.remoteDesktop.status.ready",
  remote_read: "settings.remoteDesktop.status.remoteRead",
  pasted: "settings.remoteDesktop.status.pasted",
  cancelled: "settings.remoteDesktop.status.cancelled",
  expired: "settings.remoteDesktop.status.expired",
  failed: "settings.remoteDesktop.status.failed",
};

export const RemoteDesktopSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [detection, setDetection] = useState<RemoteDesktopDetection | null>(
    null,
  );
  const [status, setStatus] = useState<RemoteTransferStatus | null>(null);
  const [busy, setBusy] = useState(false);

  // The integration only exists on Windows: the remote client it targets is
  // a Windows application.
  const isWindows = type() === "windows";

  const enabled = getSetting("remote_desktop_enabled") ?? false;
  const manualMode = getSetting("remote_desktop_manual_mode") ?? true;
  const autoMode = getSetting("remote_desktop_auto_mode") ?? false;
  const restoreClipboard =
    getSetting("remote_desktop_restore_clipboard") ?? true;
  const notify = getSetting("remote_desktop_notify") ?? true;

  const refreshDetection = useCallback(async () => {
    try {
      // The detection is best-effort: a failure leaves the panel showing
      // that nothing is known, instead of breaking the section.
      setDetection(await commands.detectRemoteDesktop());
    } catch {
      setDetection(null);
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await commands.remoteTransferStatus());
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    if (!isWindows) {
      return;
    }
    refreshDetection();
    refreshStatus();

    // The backend publishes the state of the transfer whenever it changes,
    // including when the transcript is prepared from the global shortcut.
    const unlisten = listen<RemoteTransferStatus>(
      "remote-transfer-status",
      (event) => {
        setStatus(event.payload);
        refreshDetection();
      },
    );

    return () => {
      unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, [isWindows, refreshDetection, refreshStatus]);

  const prepare = useCallback(async () => {
    setBusy(true);
    try {
      const result = await commands.prepareRemotePaste();
      if (result.status === "ok") {
        setStatus(result.data);
        if (notify) {
          toast.success(t("settings.remoteDesktop.status.ready"));
        }
      } else {
        toast.error(t("settings.remoteDesktop.status.failed"), {
          description: result.error,
        });
      }
    } finally {
      setBusy(false);
    }
  }, [notify, t]);

  const confirm = useCallback(async () => {
    try {
      setStatus(await commands.confirmRemotePaste());
    } catch {
      // The backend owns the state; the next refresh shows the truth.
      refreshStatus();
    }
  }, [refreshStatus]);

  const cancel = useCallback(async () => {
    try {
      setStatus(await commands.cancelRemoteTransfer());
    } catch {
      refreshStatus();
    }
  }, [refreshStatus]);

  if (!isWindows) {
    return null;
  }

  const phase = status?.phase ?? "idle";
  const pending = status?.pending ?? false;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup
        title={t("settings.remoteDesktop.title")}
        description={t("settings.remoteDesktop.description")}
      >
        <ToggleSwitch
          checked={enabled}
          onChange={(value) => updateSetting("remote_desktop_enabled", value)}
          isUpdating={isUpdating("remote_desktop_enabled")}
          label={t("settings.remoteDesktop.enable.title")}
          description={t("settings.remoteDesktop.enable.description")}
          descriptionMode="inline"
          grouped={true}
        />
        <ToggleSwitch
          checked={manualMode}
          onChange={(value) =>
            updateSetting("remote_desktop_manual_mode", value)
          }
          isUpdating={isUpdating("remote_desktop_manual_mode")}
          disabled={!enabled}
          label={t("settings.remoteDesktop.manualMode.title")}
          description={t("settings.remoteDesktop.manualMode.description")}
          descriptionMode="inline"
          grouped={true}
        />
        <ToggleSwitch
          checked={autoMode}
          onChange={(value) => updateSetting("remote_desktop_auto_mode", value)}
          isUpdating={isUpdating("remote_desktop_auto_mode")}
          disabled={!enabled || !manualMode}
          label={t("settings.remoteDesktop.autoMode.title")}
          description={t("settings.remoteDesktop.autoMode.description")}
          descriptionMode="inline"
          grouped={true}
        />
        {enabled && autoMode && (
          <p className="px-4 py-2 text-xs text-mid-gray">
            {t("settings.remoteDesktop.autoModeNotValidated")}
          </p>
        )}
        <ToggleSwitch
          checked={restoreClipboard}
          onChange={(value) =>
            updateSetting("remote_desktop_restore_clipboard", value)
          }
          isUpdating={isUpdating("remote_desktop_restore_clipboard")}
          disabled={!enabled}
          label={t("settings.remoteDesktop.restoreClipboard.title")}
          description={t("settings.remoteDesktop.restoreClipboard.description")}
          descriptionMode="inline"
          grouped={true}
        />
        <ToggleSwitch
          checked={notify}
          onChange={(value) => updateSetting("remote_desktop_notify", value)}
          isUpdating={isUpdating("remote_desktop_notify")}
          disabled={!enabled}
          label={t("settings.remoteDesktop.notify.title")}
          description={t("settings.remoteDesktop.notify.description")}
          descriptionMode="inline"
          grouped={true}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.remoteDesktop.actions.title")}>
        <ShortcutInput shortcutId="prepare_remote_paste" grouped={true} />
        <div className="px-4 py-3 flex gap-2 items-center">
          <Button
            variant="primary"
            size="sm"
            disabled={!enabled || busy}
            onClick={prepare}
          >
            {t("settings.remoteDesktop.actions.prepare")}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            disabled={!pending}
            onClick={confirm}
          >
            {t("settings.remoteDesktop.actions.confirm")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={!pending}
            onClick={cancel}
          >
            {t("settings.remoteDesktop.actions.cancel")}
          </Button>
        </div>
        <div className="px-4 py-2 text-xs text-mid-gray">
          {t(PHASE_MESSAGE_KEY[phase])}
          {status?.warning ? ` — ${status.warning}` : ""}
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("settings.remoteDesktop.detection.title")}>
        <div className="px-4 py-3 text-xs text-mid-gray space-y-1">
          <div>
            {t("settings.remoteDesktop.detection.installed")}:{" "}
            {detection?.client_installed
              ? `${detection.client_version ?? "?"} (${detection.client_location ?? ""})`
              : t("settings.remoteDesktop.detection.notInstalled")}
          </div>
          <div>
            {detection?.client_running
              ? t("settings.remoteDesktop.detection.running")
              : t("settings.remoteDesktop.detection.stopped")}
          </div>{" "}
          <div>
            {t("settings.remoteDesktop.detection.sessions")}:{" "}
            {detection?.rdp_session_count ?? 0}
          </div>
          <div>
            {detection?.remote_window_focused
              ? t("settings.remoteDesktop.detection.focused")
              : t("settings.remoteDesktop.detection.notFocused")}
          </div>
          {detection?.blocked_reason && (
            <div>
              {t("settings.remoteDesktop.detection.blocked", {
                reason: detection.blocked_reason,
              })}
            </div>
          )}
        </div>
      </SettingsGroup>
    </div>
  );
};
