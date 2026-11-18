import type { Theme } from "./api";

export const defaultTheme: Theme = {
  background: "#1d1e1c",
  background_soft: "#262825",
  background_raised: "#30332f",
  foreground: "#ebe8de",
  foreground_muted: "#9c9d94",
  accent: "#b4d47c",
  accent_soft: "#56663d",
  warm: "#e3b766",
  danger: "#ec7a6f",
  radius: 22,
  font: "Nunito Variable, Nunito, system-ui, sans-serif",
};

export const colorLabels: [keyof Theme, string][] = [
  ["background", "Background"],
  ["background_soft", "Panels"],
  ["background_raised", "Raised"],
  ["foreground", "Text"],
  ["foreground_muted", "Muted text"],
  ["accent", "Accent"],
  ["accent_soft", "Soft accent"],
  ["warm", "Warm highlight"],
  ["danger", "Danger"],
];
