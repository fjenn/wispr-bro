// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Entry of the `fork-pill` web view (S-03). The window exists only around a
 * take (`src-tauri/src/fork/pill/panel.rs`); the resting bar is native.
 */
import React from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import {
  applyTheme,
  getStoredTheme,
  syncThemeFromSettings,
} from "@/lib/utils/theme";
import type { Theme } from "@/bindings";
import "@/i18n";
import "../styles/fonts.css";
import "../styles/tokens.css";
import { Pill } from "./Pill";
import "./pill.css";

// Same theme handling as Handy's overlay: last-known theme before render,
// then the persisted setting, then live changes.
applyTheme(getStoredTheme());
void syncThemeFromSettings();
void listen<Theme>("theme-changed", (event) => applyTheme(event.payload));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Pill />
  </React.StrictMode>,
);
