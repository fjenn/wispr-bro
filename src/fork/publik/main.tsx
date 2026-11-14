// SPDX-License-Identifier: GPL-3.0-or-later
// The publik edition's sign-in window (src-tauri/src/fork/publik/gate.rs).
import React from "react";
import ReactDOM from "react-dom/client";
import {
  applyTheme,
  getStoredTheme,
  syncThemeFromSettings,
} from "@/lib/utils/theme";
import "@/App.css";
import "../styles/fonts.css";
import "../styles/tokens.css";
import { Gate } from "./Gate";

applyTheme(getStoredTheme());
void syncThemeFromSettings();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Gate />
  </React.StrictMode>,
);
