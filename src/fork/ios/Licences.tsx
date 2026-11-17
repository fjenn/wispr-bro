// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Acknowledgements: Wispr Bro's own licence and the open-source projects
 * it is built from, each with its licence text (the files in `LICENSES/`,
 * loaded only when opened). The complete generated list of every crate and
 * package is `THIRD-PARTY-NOTICES.md` in the source repository.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { LICENSE_URL, REPO_URL } from "../hub/links";
import { FORK_NAMESPACE } from "../i18n";
import { Dialog } from "../ui";
import { IosGroup, IosNavRow, IosTitle } from "./parts";

const NOTICES_URL = `${REPO_URL}/blob/wispr-bro/THIRD-PARTY-NOTICES.md`;

const texts = import.meta.glob<string>("../../../LICENSES/*.txt", {
  import: "default",
  query: "?raw",
});

/** The licence id a file name starts with (`<id>-<project>.txt`). */
const LICENCE_ID =
  /^(Apache-2\.0|BSD-2-Clause|BSD-3-Clause|MIT-or-Unlicense|Unlicense-or-MIT-0|MIT|OFL-1\.1|Unlicense)-/;

interface Entry {
  path: string;
  name: string;
  licence: string;
}

/** `LICENSES/MIT-llama.cpp.txt` → llama.cpp, MIT. */
const ENTRIES: Entry[] = Object.keys(texts)
  .map((path) => {
    const file = path.slice(path.lastIndexOf("/") + 1).replace(/\.txt$/, "");
    const match = file.match(LICENCE_ID);
    const licence = match ? match[0].slice(0, -1) : "";
    return {
      path,
      name: match ? file.slice(match[0].length) : file,
      licence: licence.replace(/-or-/g, " or "),
    };
  })
  .sort((a, b) => a.name.localeCompare(b.name, "en", { sensitivity: "base" }));

function open(url: string): void {
  void openUrl(url).catch(() => undefined);
}

export function Licences() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [shown, setShown] = useState<Entry | null>(null);
  const [text, setText] = useState<string | null>(null);

  const show = (entry: Entry) => {
    setShown(entry);
    setText(null);
    void texts[entry.path]()
      .then(setText)
      .catch(() => setText(t("ios.licences.failed")));
  };

  return (
    <div className="flex flex-col gap-5" data-testid="wb-ios-licences">
      <IosTitle>{t("ios.settings.pages.licences")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.licences.intro")}
      </p>
      <IosGroup label={t("rust.appName")}>
        <IosNavRow
          label={t("ios.licences.own")}
          value="GPL-3.0"
          onClick={() => open(LICENSE_URL)}
          testId="wb-ios-licence-own"
        />
        <IosNavRow
          label={t("ios.licences.source")}
          onClick={() => open(REPO_URL)}
          testId="wb-ios-licence-source"
        />
      </IosGroup>
      <IosGroup
        label={t("ios.licences.builtWith")}
        footer={t("ios.licences.footer")}
      >
        {ENTRIES.map((entry) => (
          <IosNavRow
            key={entry.path}
            label={entry.name}
            value={entry.licence}
            onClick={() => show(entry)}
          />
        ))}
        <IosNavRow
          label={t("ios.licences.all")}
          onClick={() => open(NOTICES_URL)}
          testId="wb-ios-licence-all"
        />
      </IosGroup>
      <Dialog
        open={shown !== null}
        onOpenChange={(open) => !open && setShown(null)}
        title={shown?.name ?? ""}
        description={shown?.licence}
        closeLabel={t("ios.close")}
        testId="wb-ios-licence-text"
      >
        <pre
          data-wb-selectable
          className="font-(family-name:--wb-font-mono) text-[12px] leading-[18px] break-words whitespace-pre-wrap text-(--wb-ink-2)"
        >
          {text ?? t("ios.licences.loading")}
        </pre>
      </Dialog>
    </div>
  );
}
