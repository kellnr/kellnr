import globals from "globals";
import pluginJs from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";


/** @type {import('eslint').Linter.Config[]} */
export default [
  {files: ["**/*.{js,mjs,cjs,ts,jsx,tsx,ctx,mts,vue}"]},
  {ignores: [
    "*.DS_Store*", "*node_modules*", "*dist*",
    "*/.env.local", "*/.env.*.local",
    "*/npm-debug.log*", "*/yarn-debug.log*", "*/yarn-error.log*", "*/pnpm-debug.log*",
    "*/.idea", "*/.vscode", "*/*.suo", "*.ntvs*", "*/*.njsproj", "*.sln", "*.sw?"
  ]},
  {languageOptions: { globals: globals.browser }},
  pluginJs.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/essential"],
  {files: ["**/*.vue"], languageOptions: {parserOptions: {parser: tseslint.parser}}},
  {rules: {"vue/multi-word-component-names": "off"}}
];