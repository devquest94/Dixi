import i18next from "eslint-plugin-i18next";
import tsParser from "@typescript-eslint/parser";

export default [
  {
    // Artefactos generados. `src-tauri/target/**` contiene el código que
    // Tauri genera al compilar (los assets del codegen), y `dist/**` el
    // bundle del frontend: no es codigo fuente y no se debe analizar.
    // Los errores reales del codigo fuente siguen visibles.
    ignores: [
      "src-tauri/target/**",
      "dist/**",
      "node_modules/**",
      "src/bindings.ts",
    ],
  },
  {
    // Los scripts de Node son TypeScript y se analizan, pero no tienen JSX
    // y no necesitan la regla de traduccion.
    files: ["scripts/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    plugins: {
      i18next,
    },
    rules: {
      // Catch text in JSX that should be translated
      "i18next/no-literal-string": [
        "error",
        {
          markupOnly: true, // Only check JSX content, not all strings
          ignoreAttribute: [
            "className",
            "style",
            "type",
            "id",
            "name",
            "key",
            "data-*",
            "aria-*",
          ], // Ignore common non-translatable attributes
        },
      ],
    },
  },
];
