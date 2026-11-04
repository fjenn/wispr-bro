// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Add a website" to a style group (S-19b, OT2-C): the user types a domain
 * or pastes an address and picks the group (the tab's by default). Only the
 * host is kept (`fork_style_set_site_group` reduces it again): a site rule
 * covers that host and its subdomains, because a take only ever learns the
 * page's host. Websites the user dictated on that have no group yet are
 * offered below for one click.
 */
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Globe, Plus } from "../ui/icons";
import { Dialog } from "../ui/Dialog";
import { commands, type StyleGroup, type StyleSite } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING, Select, TextField } from "../ui";
import { STYLE_GROUPS } from "./useStyle";

/**
 * The host a typed domain or address names, as the backend reduces it
 * (`commands::style::site_host`): lower case, no scheme, `www.`, `*.`,
 * port, path or query. `null` when it is not a domain.
 */
export function siteHost(input: string): string | null {
  let text = input.trim().toLowerCase();
  if (!text || text.length > 512 || /\s/.test(text)) return null;
  if (text.startsWith("*.")) text = text.slice(2);
  const scheme = /^([a-z][a-z0-9+.-]*):\/\//.exec(text);
  if (scheme) {
    if (scheme[1] !== "http" && scheme[1] !== "https") return null;
    text = text.slice(scheme[0].length);
  }
  let host = text.split(/[/?#]/)[0] ?? "";
  host = host.split("@").pop() ?? "";
  if (host.includes(":")) host = host.split(":")[0] ?? "";
  host = host.replace(/\.$/, "");
  if (host.startsWith("www.")) host = host.slice(4);
  const labels = host.split(".");
  const tld = labels[labels.length - 1] ?? "";
  const ok =
    labels.length >= 2 &&
    labels.every(
      (l) =>
        l.length > 0 &&
        l.length <= 63 &&
        /^[a-z0-9-]+$/.test(l) &&
        !l.startsWith("-") &&
        !l.endsWith("-"),
    ) &&
    /^[a-z]{2,}$/.test(tld);
  return ok ? host : null;
}

/** Sites dictated on that no built-in list or rule has put in a group. */
export function recentUnassigned(sites: StyleSite[]): StyleSite[] {
  return sites.filter((s) => s.recent && !s.builtin && s.user_group === null);
}

export interface AddSiteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  group: StyleGroup;
  sites: StyleSite[];
  /** Puts `host` in `group`; resolves to whether it worked. */
  onAdd: (host: string, group: StyleGroup) => Promise<boolean>;
}

export function AddSiteDialog({
  open,
  onOpenChange,
  group,
  sites,
  onAdd,
}: AddSiteDialogProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [value, setValue] = useState("");
  const [target, setTarget] = useState<StyleGroup>(group);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [touched, setTouched] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const inputId = useId();
  const hintId = useId();
  const groupId = useId();
  const recentHeading = useId();

  useEffect(() => {
    if (!open) return;
    setValue("");
    setTarget(group);
    setBusy(false);
    setFailed(false);
    setTouched(false);
  }, [open, group]);

  const host = siteHost(value);
  const existing = host
    ? sites.find((s) => s.host === host || host.endsWith(`.${s.host}`))
    : undefined;
  const recent = useMemo(() => recentUnassigned(sites), [sites]);
  const groupLabel = (g: StyleGroup) => t(`style.groups.${g}.label`);
  const invalid = touched && value.trim() !== "" && host === null;

  const add = async (h: string) => {
    setBusy(true);
    setFailed(false);
    const ok = await onAdd(h, target);
    setBusy(false);
    if (ok) onOpenChange(false);
    else setFailed(true);
  };

  let hint = t("style.sites.add.hint");
  if (invalid) hint = t("style.sites.add.invalid");
  else if (host && existing && existing.host !== host)
    hint = t("style.sites.add.covered", {
      host,
      parent: existing.host,
      group: groupLabel(existing.group),
    });
  else if (host && existing && existing.group === target)
    // Mac wave 7: a site already in the chosen group (often a built-in
    // one) said "Adding it here moves it" although nothing would move.
    hint = t("style.sites.add.same", {
      host,
      group: groupLabel(existing.group),
    });
  else if (host && existing)
    hint = t("style.sites.add.already", {
      host,
      group: groupLabel(existing.group),
    });
  else if (host) hint = t("style.sites.add.preview", { host });

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !busy && onOpenChange(o)}
      title={t("style.sites.add.title")}
      description={t("style.sites.add.description")}
      closeLabel={t("style.picker.close")}
      initialFocusRef={inputRef as React.RefObject<HTMLElement>}
      className="max-w-lg!"
      contentFades={false}
      footer={
        <>
          <Button
            variant="secondary"
            onClick={() => onOpenChange(false)}
            disabled={busy}
          >
            {t("style.picker.cancel")}
          </Button>
          <Button
            variant="primary"
            type="submit"
            form={`${inputId}-form`}
            disabled={busy || host === null}
            data-testid="wb-style-site-add-submit"
          >
            {t("style.sites.add.submit", { group: groupLabel(target) })}
          </Button>
        </>
      }
    >
      <form
        id={`${inputId}-form`}
        className="flex flex-col gap-4"
        data-testid="wb-style-site-add"
        onSubmit={(event) => {
          event.preventDefault();
          setTouched(true);
          if (host && !busy) void add(host);
        }}
      >
        <div className="flex flex-col gap-1.5">
          <label htmlFor={inputId} className="text-[13px] font-medium">
            {t("style.sites.add.label")}
          </label>
          <TextField
            id={inputId}
            ref={inputRef}
            value={value}
            invalid={invalid}
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            dir="ltr"
            placeholder="example.com"
            aria-describedby={hintId}
            aria-invalid={invalid || undefined}
            leading={<Globe size={16} aria-hidden="true" />}
            onChange={(event) => setValue(event.currentTarget.value)}
            onBlur={() => setTouched(true)}
            data-testid="wb-style-site-add-input"
          />
          <p
            id={hintId}
            aria-live="polite"
            className={`text-[13px] ${invalid ? "text-(--wb-danger)" : "text-(--wb-ink-2)"}`}
            data-testid="wb-style-site-add-hint"
          >
            {hint}
          </p>
        </div>
        <div className="flex flex-col gap-1.5">
          <span id={groupId} className="text-[13px] font-medium">
            {t("style.sites.add.groupLabel")}
          </span>
          <Select
            labelledBy={groupId}
            value={target}
            onValueChange={(v) => setTarget(v as StyleGroup)}
            options={STYLE_GROUPS.map((g) => ({
              value: g,
              label: groupLabel(g),
            }))}
            testId="wb-style-site-add-group"
          />
        </div>
        {failed && (
          <p role="alert" className="text-[13px] text-(--wb-danger)">
            {t("style.apps.changeFailed")}
          </p>
        )}
      </form>
      {recent.length > 0 && (
        <section
          aria-labelledby={recentHeading}
          className="mt-5 flex flex-col gap-2"
          data-testid="wb-style-site-add-recent"
        >
          <h3
            id={recentHeading}
            className="text-[13px] font-medium text-(--wb-ink-2)"
          >
            {t("style.sites.recent.heading")}
          </h3>
          <ul className="flex flex-wrap gap-2">
            {recent.map((site) => (
              <li key={site.host}>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void add(site.host)}
                  aria-label={t("style.sites.recent.addLabel", {
                    host: site.host,
                    group: groupLabel(target),
                  })}
                  data-testid="wb-style-site-add-recent-item"
                  data-host={site.host}
                  dir="ltr"
                  className={`flex h-8 cursor-pointer items-center gap-1.5 rounded-(--wb-radius-sm) border border-(--wb-line-2) bg-(--wb-surface) px-2.5 text-[13px] text-(--wb-ink) hover:bg-(--wb-surface-tint) disabled:cursor-default disabled:opacity-60 ${FOCUS_RING}`}
                >
                  <Plus size={14} aria-hidden="true" />
                  {site.host}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </Dialog>
  );
}
