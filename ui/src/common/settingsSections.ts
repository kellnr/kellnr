/**
 * Names of the settings sections, shared by the settings navigation and the
 * section headings so both always show the same name.
 */
export const SETTINGS_SECTION_TITLES = {
  password: "Change Password",
  tokens: "Auth Tokens",
  users: "Users",
  groups: "Groups",
  config: "Startup Config",
  toolchains: "Toolchains",
} as const;

export type SettingsTab = keyof typeof SETTINGS_SECTION_TITLES;
