// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The History tab: the Hub's own list (`../history/HistoryList`) under a
 * large title. On the Mac the list sits on Home; a phone gives it a tab.
 */
import { useTranslation } from "react-i18next";
import { HistoryList } from "../history/HistoryList";
import { FORK_NAMESPACE } from "../i18n";
import { IosTitle } from "./parts";

export function IosHistory() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="flex flex-col gap-1" data-testid="wb-ios-history">
      <IosTitle>{t("ios.tabs.history")}</IosTitle>
      <HistoryList />
    </div>
  );
}
