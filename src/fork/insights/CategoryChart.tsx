// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Where you dictate" (S-25, C-54): words per usage category as horizontal
 * bars on Recharts 3.10.1 (MIT, R4 "Charts"; this module is loaded lazily,
 * so Recharts never weighs on the Hub's first paint).
 *
 * One series, so one colour (`--wb-bar`, the reference's data teal; an
 * empty category is a short pale bar) and no legend; the six categories
 * keep a fixed order (an entity never moves or changes colour), each bar
 * carries its share inside and its name after it in micro capitals, and a
 * hover tooltip gives the words.
 * The chart is drawn for sighted users; the same numbers are a list for
 * screen readers (`sr-only`), so the picture is `aria-hidden`. In a
 * right-to-left language the bars grow from the right.
 */
import {
  Bar,
  BarChart,
  Cell,
  LabelList,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useTranslation } from "react-i18next";
import type { CategoryInsight } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";

export function formatShare(share: number, language: string): string {
  try {
    return new Intl.NumberFormat(language, {
      style: "percent",
      maximumFractionDigits: 0,
    }).format(share);
  } catch {
    return `${Math.round(share * 100)}%`;
  }
}

interface Row {
  id: string;
  name: string;
  words: number;
  share: number;
  label: string;
  wordsText: string;
}

const ROW_HEIGHT = 44;
/** Room after the longest bar for its category name. */
const LABEL_ROOM = 0.75;

export default function CategoryChart({
  categories,
}: {
  categories: CategoryInsight[];
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const rtl = i18n.dir(language) === "rtl";
  const rows: Row[] = categories.map((c) => ({
    id: c.category,
    name: t(`insights.categories.${c.category}`),
    words: c.words,
    share: c.share,
    label: formatShare(c.share, language),
    wordsText: t("insights.categories.words", { count: c.words }),
  }));

  return (
    <div data-testid="wb-insights-categories-chart">
      <ul className="sr-only" data-testid="wb-insights-categories-list">
        {rows.map((r) => (
          <li key={r.id} data-category={r.id}>
            {`${r.name}: ${r.label}, ${r.wordsText}`}
          </li>
        ))}
      </ul>
      {/* The SVG is laid out left to right (an inherited `rtl` flips SVG
          text anchors); right-to-left languages get a reversed axis with
          the labels on the right instead. */}
      <div
        aria-hidden="true"
        dir="ltr"
        style={{ height: rows.length * ROW_HEIGHT + 8 }}
      >
        <ResponsiveContainer width="100%" height="100%">
          <BarChart
            data={rows}
            layout="vertical"
            margin={{ top: 0, right: 0, bottom: 0, left: 0 }}
            barCategoryGap={6}
            accessibilityLayer={false}
          >
            <XAxis
              type="number"
              hide
              domain={[0, (max: number) => max * (1 + LABEL_ROOM)]}
              reversed={rtl}
            />
            <YAxis type="category" dataKey="name" hide />
            <Tooltip
              cursor={{ fill: "var(--wb-fill-hover)" }}
              isAnimationActive={false}
              content={({ active, payload }) => {
                const row = payload?.[0]?.payload as Row | undefined;
                if (!active || !row) return null;
                return (
                  <div className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) px-3 py-2 text-[13px] text-(--wb-ink) shadow-(--wb-elev-1)">
                    <div className="font-medium">{row.name}</div>
                    <div className="text-(--wb-ink-2)">
                      {row.label} · {row.wordsText}
                    </div>
                  </div>
                );
              }}
            />
            <Bar
              dataKey="share"
              radius={4}
              minPointSize={44}
              isAnimationActive={false}
            >
              {rows.map((r) => (
                <Cell
                  key={r.id}
                  fill={r.words > 0 ? "var(--wb-bar)" : "var(--wb-data-4)"}
                />
              ))}
              <LabelList
                dataKey="label"
                content={(props) => {
                  // The share inside the bar's start, the category name
                  // after its end (whichever way the axis runs).
                  const x = Number(props.x ?? 0);
                  const w = Number(props.width ?? 0);
                  const y = Number(props.y ?? 0);
                  const h = Number(props.height ?? 0);
                  const row = rows[Number(props.index ?? 0)];
                  const empty = !row || row.words === 0;
                  const start = rtl ? Math.max(x, x + w) : Math.min(x, x + w);
                  const end = rtl ? Math.min(x, x + w) : Math.max(x, x + w);
                  const anchor = rtl ? "end" : "start";
                  return (
                    <g>
                      <text
                        x={(start + end) / 2}
                        y={y + h / 2}
                        dominantBaseline="central"
                        textAnchor="middle"
                        fill={empty ? "var(--wb-ink-2)" : "var(--wb-panel)"}
                        fontSize={12}
                        fontWeight={600}
                        letterSpacing="0.08em"
                      >
                        {props.value}
                      </text>
                      <text
                        x={end + (rtl ? -12 : 12)}
                        y={y + h / 2}
                        dominantBaseline="central"
                        textAnchor={anchor}
                        fill="var(--wb-ink)"
                        fontSize={12}
                        fontWeight={500}
                        letterSpacing="0.08em"
                      >
                        {row?.name.toLocaleUpperCase(language)}
                      </text>
                    </g>
                  );
                }}
              />
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
