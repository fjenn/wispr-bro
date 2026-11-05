// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Entry of the `fork-spotlight` web view (S-30): the click-through dim
 * around the Flow Bar with the first-use hint
 * (`src-tauri/src/fork/pill/spotlight.rs`). The window exists only while
 * the spotlight is on.
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
import { Spotlight } from "./Spotlight";
import "./spotlight.css";

applyTheme(getStoredTheme());
void syncThemeFromSettings();
void listen<Theme>("theme-changed", (event) => applyTheme(event.payload));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Spotlight />
  </React.StrictMode>,
);
