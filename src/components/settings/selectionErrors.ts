/**
 * Los errores de la captura de la seleccion, traducidos para la interfaz.
 *
 * El backend manda el motivo con un codigo estable en snake_case
 * ("nothing_selected") y, ademas, que aplicacion tenia el foco y en que
 * paso fallo. Sin eso el aviso es "no funciona", que no dice nada: el atajo
 * solo puede capturar desde la aplicacion donde el texto esta seleccionado.
 */

// El codigo del backend, en snake_case, y la clave con la que se traduce.
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

/** La clave con la que se traduce un codigo del backend. */
export const selectionErrorKey = (code: string): string => {
  const camel = code.replace(/_([a-z])/g, (_match, letter: string) =>
    letter.toUpperCase(),
  );
  return (SELECTION_ERROR_KEYS as readonly string[]).includes(camel)
    ? camel
    : "write";
};

/** Lo que el backend sabe de un fallo. */
export interface SelectionFailureDiagnostics {
  app_name?: string | null;
  control_class?: string | null;
  step?: string | null;
}

/** Un fallo del backend, con lo que se sabia del intento. */
export interface SelectionFailure {
  code: string;
  message: string;
  diagnostics?: SelectionFailureDiagnostics | null;
}

/**
 * El aviso de un fallo, con la aplicacion que tenia el foco.
 */
export const describeFailure = (
  t: (key: string, options?: Record<string, unknown>) => string,
  failure: SelectionFailure,
): string => {
  const message = t(`settings.selection.errors.${selectionErrorKey(failure.code)}`);
  const app = failure.diagnostics?.app_name;
  if (!app) return message;
  // Si el foco estaba en Dixi, el atajo se ha pulsado donde no podia
  // capturar nada: hay que decirlo, porque es la causa mas habitual.
  if (app.toLowerCase().includes("dixi")) {
    return `${message} ${t("settings.selection.errors.focusedAppIsDixi")}`;
  }
  return `${message} ${t("settings.selection.errors.focusedApp", { app })}`;
};
