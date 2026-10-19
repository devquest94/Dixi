// Modulo de actualizacion retirado en Dixi.
//
// Dixi es una aplicacion personal y local: no se publica, no hay servidor de
// releases y el actualizador no tendria a que comprobar nada. El flujo
// heredado de Handy (comprobar actualizaciones, reinstalar la version
// portable) queda desmontado:
//
//   - no se monta el componente en la interfaz;
//   - el plugin `tauri-plugin-updater` no se registra en Rust;
//   - el permiso `updater:default` no esta en las capabilities.
//
// Lo que SI se conserva es la logica pura de resolucion del instalador
// portable (`portableInstaller.ts`), con su test, por si Dixi llegara a
// distribuirse: es independiente del plugin y reutilizable tal cual.
export {};
