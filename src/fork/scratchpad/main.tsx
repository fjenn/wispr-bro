// SPDX-License-Identifier: GPL-3.0-or-later
import React from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import type { Theme } from "@/bindings";
import {
  applyTheme,
  getStoredTheme,
  syncThemeFromSettings,
} from "@/lib/utils/theme";
import "@/i18n";
import "@/App.css";
import { NoteWindow } from "./NoteWindow";
import { installBeforeCaret } from "../ui/beforeCaret";
import "../styles/fonts.css";
import "../styles/tokens.css";
import "./scratchpad.css";

applyTheme(getStoredTheme());
void syncThemeFromSettings();
void listen<Theme>("theme-changed", (event) => applyTheme(event.payload));
// The smart leading space for dictations into the note.
installBeforeCaret();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <NoteWindow />
  </React.StrictMode>,
);
