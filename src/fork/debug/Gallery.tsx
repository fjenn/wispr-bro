// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The primitives gallery (Design Phase 0): every fork UI primitive in every
 * state, light and dark side by side, for the Phase 1 area agents and the
 * owner. Debug builds (and `--debug` launches) only, at `#/fork/gallery`;
 * `tests/fork/gallery.spec.ts` screenshots it and runs axe on both themes.
 *
 * The sample words are developer-facing and fixed (`SAMPLE`), never shown
 * to users, so they are not translated.
 */
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getLanguageDirection } from "@/lib/utils/rtl";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Dialog,
  EmptyState,
  Hero,
  IconButton,
  Kbd,
  KbdCombo,
  ListContainer,
  ListRow,
  Menu,
  PageHeader,
  Popover,
  ProgressDashes,
  ProgressLine,
  SearchField,
  SectionLabel,
  SegmentedControl,
  Select,
  Sheet,
  StatNumber,
  Stepper,
  Switch,
  TabPanel,
  Tabs,
  TextArea,
  TextField,
  Tooltip,
} from "../ui";
import {
  BookA,
  Copy,
  Flag,
  Mic,
  MoreHorizontal,
  Play,
  Plus,
  Search,
  Settings,
  Sparkles,
  Trash2,
  WandSparkles,
} from "../ui/icons";
import * as Icons from "../ui/icons";

const SAMPLE = {
  title: "Primitives",
  type: "Type",
  colours: "Colours",
  buttons: "Buttons",
  controls: "Controls",
  fields: "Fields",
  surfaces: "Cards and lists",
  feedback: "Progress, badges, keys",
  overlays: "Overlays",
  empty: "Empty state and hero",
  icons: "Icons",
  primary: "Continue",
  secondary: "Import",
  ghost: "Cancel",
  destructive: "Delete",
  cream: "See how it works",
  outline: "Change",
  disabled: "Disabled",
  withIcon: "New note",
  hero: "Hero 96 serif",
  display: "Display 44",
  headingXl: "Heading XL 32 sans",
  sheet: "Sheet heading,",
  sheetEm: "whispered",
  serifTitle: "Serif title 28",
  pageTitle: "Page title 24",
  heading: "Dialog heading 17",
  cardTitle: "Card title 16",
  body: "Body 15: hold fn, speak, and let go; clean text lands where your cursor is.",
  small: "Small 13: captions, chips and secondary buttons.",
  micro: "Micro label",
  tiny: "Tiny 11: progress caption",
  mono: "Mono 15: the quick brown fox",
  switchOn: "Sounds",
  switchOff: "Launch at login",
  check: "Include audio",
  checkMixed: "Select all",
  segA: "Light",
  segB: "Medium",
  segC: "High",
  selectLabel: "Microphone",
  selectA: "MacBook Pro Microphone",
  selectB: "AirPods Pro",
  selectBDesc: "Wireless: slower to start",
  selectC: "Studio USB mic",
  placeholder: "Search history",
  filled: "postgres",
  invalid: "@@@",
  textarea: "A longer note that wraps over several lines.",
  tabAll: "All",
  tabMine: "Added by you",
  tabSuggested: "Suggested",
  cardPlain: "Plain card",
  cardTint: "Tinted card",
  cardRaised: "Raised test card",
  cardSelected: "Selected option",
  cardBody: "One short line of description.",
  today: "Today",
  rowTime: "6:45 pm",
  row1: "Hey, just did it. Did you pick that up?",
  row2: "I'll go through the steps, like the little tutorial. Just follow along.",
  play: "Play",
  copy: "Copy",
  flag: "Flag",
  more: "More",
  del: "Delete",
  add: "Add",
  settings: "Settings",
  progress: "Setup progress",
  stepper: "Setup stages",
  stages: ["Get started", "Permissions", "Set up", "Learn"],
  badgeNew: "New",
  badgeCount: "12",
  badgeBest: "Best match",
  badgeWarn: "Wireless",
  badgeErr: "Failed",
  badgeInk: "Beta",
  tooltip: "Collapse sidebar",
  openDialog: "Open dialog",
  openSheet: "Open sheet",
  menu: "Open menu",
  popover: "Open popover",
  popoverBody: "A popover: solid surface, 12 pt radius, soft shadow.",
  dialogTitle: "Add word",
  dialogDesc: "Words you add are spelled your way everywhere.",
  close: "Close",
  save: "Save",
  sheetLabel: "Lesson",
  sheetBody: "Large sheet: centred serif heading, one italic word.",
  emptyTitle: "Nothing here yet",
  emptyBody: "Your dictations will show up here, kept on this Mac.",
  emptyAction: "Try it now",
  heroTitle: "Hold fn to dictate",
  heroBody: "Works in every app: email, messages, docs, anywhere you type.",
  heroAction: "See how it works",
  heroSecondary: "Not now",
  stat: "words",
  page: "Dictation",
  light: "Light",
  dark: "Dark",
  fn: "fn",
};

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-4 border-t border-(--wb-line) pt-6">
      <SectionLabel>{title}</SectionLabel>
      {children}
    </section>
  );
}

function Row({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-3">{children}</div>;
}

const SWATCHES = [
  "chrome",
  "panel",
  "surface",
  "surface-tint",
  "canvas-warm",
  "sheet",
  "line",
  "line-2",
  "line-strong",
  "fill-active",
  "secondary",
  "ink",
  "ink-2",
  "ink-3",
  "accent",
  "accent-2",
  "accent-soft",
  "accent-meter",
  "data-1",
  "data-2",
  "data-3",
  "data-4",
  "success",
  "warning-bg",
  "danger",
  "tag-new-bg",
];

const ICON_NAMES = Object.keys(Icons).filter(
  (k) => /^[A-Z]/.test(k) && k !== "Icon" && !k.startsWith("ICON"),
) as (keyof typeof Icons)[];

/** One full set of primitives, pinned to a theme. */
function Board({ theme }: { theme: "light" | "dark" }) {
  const [on, setOn] = useState(true);
  const [off, setOff] = useState(false);
  const [seg, setSeg] = useState("medium");
  const [mic, setMic] = useState("builtin");
  const [tab, setTab] = useState("all");
  const [checked, setChecked] = useState(true);
  const [dialog, setDialog] = useState(false);
  const [sheet, setSheet] = useState(false);
  const id = (s: string) => `${theme}-${s}`;
  return (
    <div
      data-wb-theme={theme}
      data-testid={`wb-gallery-${theme}`}
      className="flex min-w-0 flex-1 flex-col gap-6 bg-(--wb-panel) p-8 text-(--wb-ink)"
    >
      <PageHeader
        title={`${SAMPLE.title} · ${theme === "light" ? SAMPLE.light : SAMPLE.dark}`}
      />

      <Section title={SAMPLE.type}>
        <div className="flex flex-col gap-2">
          <p className="type-hero">{SAMPLE.hero}</p>
          <p className="type-display">{SAMPLE.display}</p>
          <p className="type-heading-xl">{SAMPLE.headingXl}</p>
          <p className="type-sheet">
            {SAMPLE.sheet} <em>{SAMPLE.sheetEm}</em>
          </p>
          <p className="type-serif-title">{SAMPLE.serifTitle}</p>
          <p className="type-title">{SAMPLE.pageTitle}</p>
          <p className="type-heading">{SAMPLE.heading}</p>
          <p className="type-card-title">{SAMPLE.cardTitle}</p>
          <p className="type-body">{SAMPLE.body}</p>
          <p className="type-body text-(--wb-ink-2)">{SAMPLE.body}</p>
          <p className="type-small text-(--wb-ink-3)">{SAMPLE.small}</p>
          <p className="type-micro text-(--wb-ink-3)">{SAMPLE.micro}</p>
          <p className="type-tiny text-(--wb-ink-3)">{SAMPLE.tiny}</p>
          <p className="type-mono">{SAMPLE.mono}</p>
          <p className="type-serif-title">
            <StatNumber value={1284} />{" "}
            <span className="type-body">{SAMPLE.stat}</span>
          </p>
        </div>
      </Section>

      <Section title={SAMPLE.colours}>
        <div className="grid grid-cols-[repeat(auto-fill,minmax(92px,1fr))] gap-2">
          {SWATCHES.map((name) => (
            <div key={name} className="flex flex-col gap-1">
              <div
                className="h-10 rounded-(--wb-radius-sm) border border-(--wb-line)"
                style={{ background: `var(--wb-${name})` }}
              />
              <code className="type-tiny text-(--wb-ink-2)">{name}</code>
            </div>
          ))}
        </div>
      </Section>

      <Section title={SAMPLE.buttons}>
        {(["lg", "md", "sm"] as const).map((size) => (
          <Row key={size}>
            <Button variant="primary" size={size}>
              {SAMPLE.primary}
            </Button>
            <Button size={size}>{SAMPLE.secondary}</Button>
            <Button variant="ghost" size={size}>
              {SAMPLE.ghost}
            </Button>
            <Button variant="outline" size={size}>
              {SAMPLE.outline}
            </Button>
            <Button variant="destructive" size={size}>
              {SAMPLE.destructive}
            </Button>
            <Button variant="primary" size={size} leading={<Plus size={16} />}>
              {SAMPLE.withIcon}
            </Button>
          </Row>
        ))}
        <Row>
          <Button variant="primary" disabled>
            {SAMPLE.disabled}
          </Button>
          <Button disabled>{SAMPLE.disabled}</Button>
          <div className="rounded-(--wb-radius-sm) bg-(--wb-hero) p-3">
            <Button variant="cream">{SAMPLE.cream}</Button>
          </div>
          <IconButton aria-label={SAMPLE.settings}>
            <Settings size={18} />
          </IconButton>
          <IconButton aria-label={SAMPLE.add} variant="secondary">
            <Plus size={18} />
          </IconButton>
          <IconButton aria-label={SAMPLE.add} variant="primary" round>
            <Plus size={18} />
          </IconButton>
          <div className="rounded-full bg-(--wb-hud) p-1.5">
            <IconButton aria-label={SAMPLE.close} variant="hud" round size="sm">
              <Icons.X size={14} />
            </IconButton>
          </div>
        </Row>
      </Section>

      <Section title={SAMPLE.controls}>
        <Row>
          <label className="flex items-center gap-3 text-[15px]">
            <Switch checked={on} onChange={setOn} testId={id("switch-on")} />
            {SAMPLE.switchOn}
          </label>
          <label className="flex items-center gap-3 text-[15px]">
            <Switch checked={off} onChange={setOff} />
            {SAMPLE.switchOff}
          </label>
          <Switch
            checked
            disabled
            onChange={() => {}}
            ariaLabel={SAMPLE.disabled}
          />
          <Switch
            checked={on}
            onChange={setOn}
            size="sm"
            ariaLabel={SAMPLE.switchOn}
          />
        </Row>
        <Row>
          <Checkbox
            label={SAMPLE.check}
            checked={checked}
            onChange={(e) => setChecked(e.target.checked)}
          />
          <Checkbox label={SAMPLE.check} checked={false} onChange={() => {}} />
          <Checkbox
            label={SAMPLE.checkMixed}
            indeterminate
            checked={false}
            onChange={() => {}}
          />
          <Checkbox
            label={SAMPLE.disabled}
            disabled
            checked
            onChange={() => {}}
          />
        </Row>
        <Row>
          <SegmentedControl
            label={SAMPLE.title}
            value={seg}
            onChange={setSeg}
            options={[
              { value: "light", label: SAMPLE.segA },
              { value: "medium", label: SAMPLE.segB },
              { value: "high", label: SAMPLE.segC },
            ]}
          />
          <SegmentedControl
            label={SAMPLE.title}
            size="sm"
            value={seg}
            onChange={setSeg}
            options={[
              { value: "light", label: SAMPLE.segA },
              { value: "medium", label: SAMPLE.segB },
              { value: "high", label: SAMPLE.segC, disabled: true },
            ]}
          />
        </Row>
        <Row>
          <Select
            label={SAMPLE.selectLabel}
            value={mic}
            onValueChange={setMic}
            testId={id("select")}
            className="w-64"
            options={[
              {
                value: "builtin",
                label: SAMPLE.selectA,
                icon: <Mic size={16} />,
              },
              {
                value: "airpods",
                label: SAMPLE.selectB,
                description: SAMPLE.selectBDesc,
              },
              { value: "usb", label: SAMPLE.selectC },
            ]}
          />
          <Select
            label={SAMPLE.selectLabel}
            value={mic}
            onValueChange={setMic}
            variant="ghost"
            size="sm"
            fitContent
            options={[
              { value: "builtin", label: SAMPLE.selectA },
              { value: "airpods", label: SAMPLE.selectB },
            ]}
          />
          <Select
            label={SAMPLE.selectLabel}
            value={undefined}
            placeholder={SAMPLE.selectLabel}
            onValueChange={setMic}
            disabled
            className="w-48"
            options={[{ value: "builtin", label: SAMPLE.selectA }]}
          />
        </Row>
        <Tabs
          label={SAMPLE.title}
          value={tab}
          onValueChange={setTab}
          tabs={[
            { value: "all", label: SAMPLE.tabAll },
            { value: "mine", label: SAMPLE.tabMine },
            { value: "suggested", label: SAMPLE.tabSuggested },
          ]}
        >
          <TabPanel value="all" className="pt-3 text-[15px]">
            {SAMPLE.tabAll}
          </TabPanel>
          <TabPanel value="mine" className="pt-3 text-[15px]">
            {SAMPLE.tabMine}
          </TabPanel>
          <TabPanel value="suggested" className="pt-3 text-[15px]">
            {SAMPLE.tabSuggested}
          </TabPanel>
        </Tabs>
      </Section>

      <Section title={SAMPLE.fields}>
        <div className="grid max-w-xl gap-3">
          <TextField
            aria-label={SAMPLE.placeholder}
            placeholder={SAMPLE.placeholder}
          />
          <TextField aria-label={SAMPLE.filled} defaultValue={SAMPLE.filled} />
          <TextField
            aria-label={SAMPLE.invalid}
            defaultValue={SAMPLE.invalid}
            invalid
          />
          <TextField
            aria-label={SAMPLE.disabled}
            defaultValue={SAMPLE.disabled}
            disabled
          />
          <SearchField
            aria-label={SAMPLE.placeholder}
            placeholder={SAMPLE.placeholder}
          />
          <TextArea
            aria-label={SAMPLE.textarea}
            defaultValue={SAMPLE.textarea}
          />
        </div>
      </Section>

      <Section title={SAMPLE.surfaces}>
        <div className="grid grid-cols-2 gap-4">
          <Card>
            <p className="type-card-title">{SAMPLE.cardPlain}</p>
            <p className="text-[15px] text-(--wb-ink-2)">{SAMPLE.cardBody}</p>
          </Card>
          <Card tone="tint">
            <p className="type-card-title">{SAMPLE.cardTint}</p>
            <p className="text-[15px] text-(--wb-ink-2)">{SAMPLE.cardBody}</p>
          </Card>
          <Card tone="raised">
            <p className="type-card-title">{SAMPLE.cardRaised}</p>
            <p className="text-[15px] text-(--wb-ink-2)">{SAMPLE.cardBody}</p>
          </Card>
          <Card selected>
            <p className="type-card-title">{SAMPLE.cardSelected}</p>
            <p className="text-[15px] text-(--wb-ink-2)">{SAMPLE.cardBody}</p>
          </Card>
        </div>
        <div className="flex items-center justify-between">
          <SectionLabel as="h3">{SAMPLE.today}</SectionLabel>
          <IconButton aria-label={SAMPLE.placeholder} size="sm">
            <Search size={16} />
          </IconButton>
        </div>
        <ListContainer>
          <ListRow leading={SAMPLE.rowTime}>{SAMPLE.row1}</ListRow>
          <ListRow
            leading={SAMPLE.rowTime}
            actionsVisible
            actions={
              <>
                <IconButton aria-label={SAMPLE.play} size="sm">
                  <Play size={16} />
                </IconButton>
                <IconButton aria-label={SAMPLE.copy} size="sm">
                  <Copy size={16} />
                </IconButton>
                <IconButton aria-label={SAMPLE.flag} size="sm">
                  <Flag size={16} />
                </IconButton>
                <IconButton
                  aria-label={SAMPLE.del}
                  size="sm"
                  className="hover:text-(--wb-danger)"
                >
                  <Trash2 size={16} />
                </IconButton>
              </>
            }
          >
            {SAMPLE.row2}
          </ListRow>
        </ListContainer>
      </Section>

      <Section title={SAMPLE.feedback}>
        <Row>
          <ProgressDashes current={2} total={4} />
          <div className="w-48">
            <ProgressLine value={0.4} label={SAMPLE.progress} />
          </div>
          <div className="w-48">
            <ProgressLine
              value={0.66}
              label={SAMPLE.progress}
              tone="ink"
              thickness={4}
            />
          </div>
        </Row>
        <Stepper
          label={SAMPLE.stepper}
          current="setup"
          stages={[
            { id: "start", label: SAMPLE.stages[0] },
            { id: "perm", label: SAMPLE.stages[1] },
            { id: "setup", label: SAMPLE.stages[2] },
            { id: "learn", label: SAMPLE.stages[3] },
          ]}
        />
        <Row>
          <Badge>{SAMPLE.badgeCount}</Badge>
          <Badge tone="ink">{SAMPLE.badgeInk}</Badge>
          <Badge tone="accent">{SAMPLE.badgeNew}</Badge>
          <Badge tone="success">{SAMPLE.badgeBest}</Badge>
          <Badge tone="warning">{SAMPLE.badgeWarn}</Badge>
          <Badge tone="danger">{SAMPLE.badgeErr}</Badge>
          <span className="rounded-full bg-(--wb-tag-new-bg) px-2 py-0.5 text-[11px] font-semibold text-(--wb-tag-new-ink)">
            {SAMPLE.badgeNew}
          </span>
        </Row>
        <Row>
          <KbdCombo keys={["⌥", "O"]} />
          <Kbd>{SAMPLE.fn}</Kbd>
          <Kbd pressed>{SAMPLE.fn}</Kbd>
          <KbdCombo keys={["⌃", "⌘", "V"]} tone="dark" />
        </Row>
      </Section>

      <Section title={SAMPLE.overlays}>
        <Row>
          <Tooltip text={SAMPLE.tooltip} open={theme === "light"} side="right">
            <IconButton aria-label={SAMPLE.tooltip}>
              <Icons.PanelLeft size={20} />
            </IconButton>
          </Tooltip>
          <Menu
            label={SAMPLE.more}
            trigger={
              <Button trailing={<MoreHorizontal size={16} />}>
                {SAMPLE.menu}
              </Button>
            }
            groups={[
              {
                items: [
                  {
                    id: "copy",
                    label: SAMPLE.copy,
                    icon: Copy,
                    onSelect: () => {},
                  },
                  {
                    id: "flag",
                    label: SAMPLE.flag,
                    icon: Flag,
                    onSelect: () => {},
                  },
                ],
              },
              {
                label: SAMPLE.settings,
                items: [
                  {
                    id: "auto",
                    label: SAMPLE.switchOn,
                    checked: true,
                    onSelect: () => {},
                  },
                ],
              },
            ]}
          />
          <Popover
            label={SAMPLE.popover}
            align="start"
            trigger={<Button>{SAMPLE.popover}</Button>}
          >
            <p className="p-4 text-[15px]">{SAMPLE.popoverBody}</p>
          </Popover>
          <Button variant="primary" onClick={() => setDialog(true)}>
            {SAMPLE.openDialog}
          </Button>
          <Button onClick={() => setSheet(true)}>{SAMPLE.openSheet}</Button>
        </Row>
        <Dialog
          open={dialog}
          onOpenChange={setDialog}
          title={SAMPLE.dialogTitle}
          description={SAMPLE.dialogDesc}
          closeLabel={SAMPLE.close}
          footer={
            <>
              <Button onClick={() => setDialog(false)}>{SAMPLE.ghost}</Button>
              <Button variant="primary" onClick={() => setDialog(false)}>
                {SAMPLE.save}
              </Button>
            </>
          }
        >
          <TextField aria-label={SAMPLE.filled} defaultValue={SAMPLE.filled} />
        </Dialog>
        <Sheet
          open={sheet}
          onOpenChange={setSheet}
          label={SAMPLE.sheetLabel}
          onBack={() => setSheet(false)}
          progress={{ current: 1, total: 3 }}
          footer={
            <Button variant="primary" onClick={() => setSheet(false)}>
              {SAMPLE.primary}
            </Button>
          }
        >
          <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
            <h2 className="type-sheet">
              {SAMPLE.sheet}{" "}
              <em className="text-(--wb-accent)">{SAMPLE.sheetEm}</em>
            </h2>
            <p className="text-[15px] text-(--wb-ink-2)">{SAMPLE.sheetBody}</p>
          </div>
        </Sheet>
      </Section>

      <Section title={SAMPLE.empty}>
        <Hero
          id={id("hero")}
          title={SAMPLE.heroTitle}
          body={SAMPLE.heroBody}
          action={{ label: SAMPLE.heroAction, onClick: () => {} }}
          secondaryAction={{ label: SAMPLE.heroSecondary, onClick: () => {} }}
          onDismiss={() => {}}
          dismissLabel={SAMPLE.close}
        />
        <Card padding="none">
          <EmptyState
            icon={BookA}
            title={SAMPLE.emptyTitle}
            body={SAMPLE.emptyBody}
            action={{ label: SAMPLE.emptyAction, onClick: () => {} }}
          />
        </Card>
        <Card padding="none" className="max-w-xs">
          <EmptyState
            compact
            art={<WandSparkles size={32} className="text-(--wb-accent)" />}
            title={SAMPLE.emptyTitle}
            body={SAMPLE.emptyBody}
          />
        </Card>
      </Section>

      <Section title={SAMPLE.icons}>
        <div className="grid grid-cols-[repeat(auto-fill,minmax(40px,1fr))] gap-2 text-(--wb-ink)">
          {ICON_NAMES.map((name) => {
            const G = Icons[name] as Icons.Glyph;
            return (
              <span
                key={name}
                title={name}
                className="grid h-10 place-items-center"
              >
                <G size={18} />
              </span>
            );
          })}
        </div>
        <Row>
          {([14, 16, 18, 20, 24] as const).map((size) => (
            <Sparkles key={size} size={size} />
          ))}
        </Row>
      </Section>
    </div>
  );
}

export function Gallery() {
  const { i18n } = useTranslation();
  return (
    <div
      data-testid="wb-gallery"
      dir={getLanguageDirection(i18n.language)}
      className="fixed inset-0 z-[45] overflow-y-auto bg-(--wb-chrome) font-(family-name:--wb-font-ui)"
    >
      <div className="flex min-w-[1200px]">
        <Board theme="light" />
        <Board theme="dark" />
      </div>
    </div>
  );
}
