/**
 * El estado del atajo de captura, traducido para la interfaz.
 *
 * El backend no dice solo "registrado" o "no registrado": dice que paso, con
 * que backend y con que codigo de fallo. Eso es lo que permite que el aviso
 * sea honesto: un atajo que otra aplicacion ya tiene es una cosa distinta
 * de un fallo interno de Dixi, y acusar a otra aplicacion cuando el
 * problema es nuestro manda al usuario a cambiar los atajos de un programa
 * que no tiene nada que ver con esto.
 */

import type { CaptureShortcutStatus } from "@/bindings";

/** El codigo estable del motivo de un fallo de registro. */
export type CaptureRegistrationErrorCode = NonNullable<
  CaptureShortcutStatus["error_code"]
>;

/**
 * Lo que el atajo de captura dice, o `null` cuando no hay nada que decir.
 *
 * `null` cuando esta registrado, cuando este sistema no puede capturar la
 * seleccion de otra aplicacion, y tambien cuando **nunca se intento**: sin
 * un intento no hay nada que achacar a nadie.
 */
export const describeShortcutProblem = (
  t: (key: string, options?: Record<string, unknown>) => string,
  status: CaptureShortcutStatus | null,
): string | null => {
  if (!status) return null;
  // En los sistemas que no pueden capturar la seleccion de otra aplicacion
  // el atajo no se registra a proposito: no hay nada que avisar.
  if (!status.available) return null;
  if (status.registered) return null;
  // Todavia no se ha intentado nada: acusar a alguien sin haberlo probado
  // seria mentira.
  if (!status.attempted) return null;

  const shortcut = status.binding || "";

  switch (status.error_code) {
    case "conflict":
      return t("settings.transforms.runner.shortcutConflict", { shortcut });
    case "duplicate":
      return t("settings.transforms.runner.shortcutDuplicate", { shortcut });
    case "invalid_combination":
      return t("settings.transforms.runner.shortcutInvalidCombination", {
        shortcut,
      });
    case "not_initialized":
      return t("settings.transforms.runner.shortcutNotInitialized", { shortcut });
    case "backend_unavailable":
      return t("settings.transforms.runner.shortcutBackendUnavailable", {
        shortcut,
      });
    default:
      return t("settings.transforms.runner.shortcutNotRegistered", { shortcut });
  }
};
