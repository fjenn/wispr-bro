// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Sidebar Help popover, "Help and Feedback" (S-26b, R2 C-13; R1 §10).
 *
 * Three groups, as Wispr's popover: **Updates** (What's new: the release
 * notes on GitHub), **Essentials** (Shortcuts, Microphone, Languages,
 * Documentation, plus lessons S-31 registers with `setHelpLessons`) and
 * **Get in touch** (Report a problem…, Send feedback). Hosted help, support
 * chat and Contact sales are out (R2 C-102, C-215): GitHub is the help desk.
 *
 * It is a Radix menu (`../ui/Menu`): arrow keys move, typeahead, Esc closes,
 * focus returns to the Help button. Report a problem opens a dialog with
 * Copy diagnostics; Send feedback opens the prefilled GitHub form at once.
 */
import { useRef, useState, type ReactElement } from "react";
import {
  BookOpen,
  Bug,
  Keyboard,
  Languages,
  MessageSquareText,
  Mic,
  Sparkles,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Menu, type MenuItem } from "../ui";
import { DOCS_URL, RELEASES_URL } from "./links";
import { navigate } from "./nav";
import { ReportProblemDialog } from "./ReportProblemDialog";

/** Lesson entries for Essentials (S-31); null shows none. */
let helpLessons: (() => MenuItem[]) | null = null;

/** S-31 adds its lessons to Help › Essentials here. */
export function setHelpLessons(items: (() => MenuItem[]) | null): void {
  helpLessons = items;
}

function open(url: string): void {
  void openUrl(url).catch(() => undefined);
}

export function HelpPopover({ trigger }: { trigger: ReactElement }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [reporting, setReporting] = useState(false);
  // Report a problem opens its dialog once the menu has closed, instead of
  // the menu handing focus back to the Help button.
  const reportNext = useRef(false);
  const lessons = helpLessons?.() ?? [];

  return (
    <>
      <Menu
        label={t("hub.help.title")}
        side="right"
        align="end"
        testId="wb-help-menu"
        groups={[
          {
            label: t("hub.help.updates"),
            items: [
              {
                id: "whats-new",
                label: t("hub.help.whatsNew"),
                icon: Sparkles,
                onSelect: () => open(RELEASES_URL),
              },
            ],
          },
          {
            label: t("hub.help.essentials"),
            items: [
              {
                id: "shortcuts",
                label: t("hub.help.shortcuts"),
                icon: Keyboard,
                onSelect: () => navigate("settings", "hotkeys"),
              },
              {
                id: "microphone",
                label: t("hub.help.microphone"),
                icon: Mic,
                onSelect: () => navigate("settings", "microphone"),
              },
              {
                id: "languages",
                label: t("hub.help.languages"),
                icon: Languages,
                onSelect: () => navigate("settings", "languages"),
              },
              ...lessons,
              {
                id: "documentation",
                label: t("hub.help.documentation"),
                icon: BookOpen,
                onSelect: () => open(DOCS_URL),
              },
            ],
          },
          {
            label: t("hub.help.getInTouch"),
            items: [
              {
                id: "report-problem",
                label: t("hub.help.reportProblem"),
                icon: Bug,
                onSelect: () => {
                  reportNext.current = true;
                },
              },
              {
                id: "send-feedback",
                label: t("hub.help.sendFeedback"),
                icon: MessageSquareText,
                onSelect: () => void commands.forkOpenIssue("feedback"),
              },
            ],
          },
        ]}
        trigger={trigger}
        onCloseAutoFocus={(event) => {
          if (!reportNext.current) return;
          reportNext.current = false;
          event.preventDefault();
          setReporting(true);
        }}
      />
      <ReportProblemDialog open={reporting} onOpenChange={setReporting} />
    </>
  );
}
