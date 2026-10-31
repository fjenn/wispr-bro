// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Sound › Microphone (S-08): the microphone list in its fixed
 * order (built-in mic "(recommended)", "Auto-detect (system setting)", then
 * the others, wired before wireless), a caution on wireless mics, a note
 * when a Bluetooth default is being skipped, and a live level meter
 * ("Test microphone"). Mounted on the fork debug panel until S-14's
 * Settings modal; S-29a's mic test reuses `MicLevelMeter`.
 */
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  events,
  type ForkMicEntry,
  type ForkMicList,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

/** Number of bars in the meter. */
export const METER_BARS = 12;

/** How many bars a 0..1 level lights. */
export function litBars(level: number, bars = METER_BARS): number {
  if (!Number.isFinite(level) || level <= 0) return 0;
  return Math.min(bars, Math.ceil(level * bars));
}

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; list: ForkMicList }
  | { kind: "error"; message: string };

function errorText(e: unknown): string {
  if (e && typeof e === "object" && "kind" in e) {
    const err = e as { kind: string; message?: string };
    return err.message ?? err.kind;
  }
  return String(e);
}

/** A reply that can be rendered (a backend not ready yet may send none). */
function isList(v: unknown): v is ForkMicList {
  return (
    !!v && typeof v === "object" && Array.isArray((v as ForkMicList).entries)
  );
}

function EntryLabel({
  entry,
  list,
}: {
  entry: ForkMicEntry;
  list: ForkMicList;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (entry.kind === "auto") {
    return (
      <span className="flex flex-col">
        <span>{t("settings.mic.auto")}</span>
        {list.system_default && (
          <span className="text-xs text-(--wb-ink-3)">
            {t("settings.mic.autoNow", { name: list.system_default })}
          </span>
        )}
      </span>
    );
  }
  return (
    <span className="flex flex-wrap items-center gap-x-2">
      <span>
        {entry.recommended
          ? t("settings.mic.recommended", { name: entry.name })
          : entry.name}
      </span>
      {entry.wireless && (
        <span
          className="rounded-full border border-(--wb-line-strong) px-1.5 text-[11px] text-(--wb-ink-2)"
          data-testid="wb-mic-wireless-tag"
        >
          {t("settings.mic.wirelessTag")}
        </span>
      )}
    </span>
  );
}

export function MicrophonePicker() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  // The choice shows at once; the backend's reply confirms or reverts it.
  const [pending, setPending] = useState<string | null>(null);
  const [chooseError, setChooseError] = useState<string | null>(null);
  const groupId = useId();
  const cautionId = useId();
  const inUseId = useId();

  const load = useCallback(async () => {
    try {
      const r = await commands.forkMicList();
      if (r.status === "ok" && isList(r.data)) {
        setState({ kind: "ready", list: r.data });
      } else {
        setState({
          kind: "error",
          message: errorText(r.status === "ok" ? r.data : r.error),
        });
      }
    } catch (e) {
      setState({ kind: "error", message: errorText(e) });
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    void load();
    const unlisten = events.forkMicDevicesChanged.listen(() => {
      if (!disposed) void load();
    });
    return () => {
      disposed = true;
      void unlisten.then((f) => f()).catch(() => undefined);
    };
  }, [load]);

  const choose = useCallback(async (value: string) => {
    setBusy(true);
    setPending(value);
    setChooseError(null);
    try {
      const r = await commands.forkMicChoose(value);
      if (r.status === "ok" && isList(r.data)) {
        setState({ kind: "ready", list: r.data });
      } else {
        setChooseError(errorText(r.status === "ok" ? r.data : r.error));
      }
    } catch (e) {
      setChooseError(errorText(e));
    } finally {
      setBusy(false);
      setPending(null);
    }
  }, []);

  if (state.kind === "loading") {
    return (
      <p className="text-xs text-(--wb-ink-3)" data-testid="wb-mic-loading">
        {t("settings.mic.loading")}
      </p>
    );
  }
  if (state.kind === "error") {
    return (
      <div className="flex flex-col gap-2 text-sm" data-testid="wb-mic-picker">
        <p role="alert" className="text-xs font-medium text-(--wb-ink)">
          {t("settings.mic.loadError", { message: state.message })}
        </p>
        <Button className="self-start" onClick={() => void load()}>
          {t("settings.mic.retry")}
        </Button>
      </div>
    );
  }

  const { list } = state;
  const anyWireless = list.entries.some((e) => e.wireless);

  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="wb-mic-picker">
      <fieldset
        className="flex flex-col gap-1"
        aria-describedby={anyWireless ? cautionId : undefined}
        aria-busy={busy}
      >
        <legend className="mb-1 font-medium">{t("settings.mic.legend")}</legend>
        {list.entries.map((entry) => {
          const id = `${groupId}-${entry.value}`;
          return (
            <label
              key={entry.value}
              htmlFor={id}
              className="flex cursor-pointer items-start gap-2 rounded-(--wb-radius-control) px-1 py-1 hover:bg-(--wb-hover)"
              data-testid="wb-mic-option"
              data-value={entry.value}
            >
              <input
                id={id}
                type="radio"
                name={groupId}
                className="mt-0.5"
                value={entry.value}
                checked={(pending ?? list.selected) === entry.value}
                onChange={() => void choose(entry.value)}
              />
              <EntryLabel entry={entry} list={list} />
            </label>
          );
        })}
      </fieldset>
      {anyWireless && (
        <p
          id={cautionId}
          className="text-xs text-(--wb-ink-2)"
          data-testid="wb-mic-caution"
        >
          {t("settings.mic.caution")}
        </p>
      )}
      {list.avoided && (
        <div
          className="flex flex-col gap-2 rounded-(--wb-radius-control) border border-(--wb-line) p-2 text-xs"
          data-testid="wb-mic-avoided"
        >
          <p>{t("settings.mic.avoided", { name: list.avoided })}</p>
          <Button
            className="self-start"
            data-testid="wb-mic-use-anyway"
            disabled={busy}
            onClick={() => void choose("auto")}
          >
            {t("settings.mic.useAnyway", { name: list.avoided })}
          </Button>
        </div>
      )}
      {list.in_use && (
        <p
          id={inUseId}
          className="text-xs text-(--wb-ink-3)"
          data-testid="wb-mic-in-use"
        >
          {t("settings.mic.inUse", { name: list.in_use })}
        </p>
      )}
      {chooseError && (
        <p role="alert" className="text-xs font-medium text-(--wb-ink)">
          {t("settings.mic.chooseError", { message: chooseError })}
        </p>
      )}
      <MicLevelMeter describedBy={list.in_use ? inUseId : undefined} />
    </div>
  );
}

/**
 * "Test microphone": live bars from `ForkMicLevel` while a monitor session
 * runs. Starting does not record a take; the session ends on Stop, after a
 * minute, when a take starts, or when this unmounts.
 */
export function MicLevelMeter({ describedBy }: { describedBy?: string }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [running, setRunning] = useState(false);
  const [level, setLevel] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const session = useRef<number | null>(null);
  const runningRef = useRef(false);

  useEffect(() => {
    let disposed = false;
    const unlisten = events.forkMicLevel.listen((e) => {
      if (disposed) return;
      const p = e.payload;
      if (session.current !== null && p.session !== session.current) return;
      if (p.running) {
        setLevel(p.level);
      } else {
        session.current = null;
        runningRef.current = false;
        setRunning(false);
        setLevel(0);
        if (p.error) setError(p.error);
      }
    });
    return () => {
      disposed = true;
      void unlisten.then((f) => f()).catch(() => undefined);
      if (runningRef.current) void commands.forkMicMonitorStop();
    };
  }, []);

  const toggle = useCallback(async () => {
    if (runningRef.current) {
      runningRef.current = false;
      setRunning(false);
      setLevel(0);
      await commands.forkMicMonitorStop();
      return;
    }
    setError(null);
    runningRef.current = true;
    setRunning(true);
    try {
      const r = await commands.forkMicMonitorStart();
      if (r.status === "ok" && typeof r.data === "number") {
        session.current = r.data;
      } else {
        runningRef.current = false;
        setRunning(false);
        setError(r.status === "ok" ? "failed" : r.error.kind);
      }
    } catch (e) {
      runningRef.current = false;
      setRunning(false);
      setError(errorText(e));
    }
  }, []);

  const lit = litBars(level);
  const percent = Math.round(level * 100);
  const errorKey =
    error === "permission_denied" || error === "no_input_device"
      ? `settings.mic.meter.errors.${error}`
      : "settings.mic.meter.errors.failed";

  return (
    <div className="flex flex-col gap-2" data-testid="wb-mic-meter">
      <div className="flex items-center gap-3">
        <Button
          onClick={() => void toggle()}
          aria-pressed={running}
          data-testid="wb-mic-test"
          aria-describedby={describedBy}
        >
          {running
            ? t("settings.mic.meter.stop")
            : t("settings.mic.meter.start")}
        </Button>
        <div
          role="meter"
          aria-label={t("settings.mic.meter.label")}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          aria-valuetext={
            running
              ? t("settings.mic.meter.value", { percent })
              : t("settings.mic.meter.off")
          }
          className="flex h-5 items-end gap-0.5"
          data-testid="wb-mic-bars"
          data-lit={lit}
        >
          {Array.from({ length: METER_BARS }, (_, i) => (
            <span
              key={i}
              aria-hidden="true"
              className={`w-1.5 rounded-sm motion-safe:transition-[height,background-color] motion-safe:duration-75 ${
                i < lit ? "bg-(--wb-accent)" : "bg-(--wb-line-strong)"
              }`}
              style={{ height: `${30 + (i * 70) / (METER_BARS - 1)}%` }}
            />
          ))}
        </div>
      </div>
      {running && (
        <p className="text-xs text-(--wb-ink-3)" aria-live="polite">
          {t("settings.mic.meter.hint")}
        </p>
      )}
      {error && (
        <p
          role="alert"
          className="text-xs font-medium text-(--wb-ink)"
          data-testid="wb-mic-meter-error"
        >
          {t(errorKey)}
        </p>
      )}
    </div>
  );
}
