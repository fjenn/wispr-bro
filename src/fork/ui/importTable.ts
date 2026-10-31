// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * CSV parsing and column mapping for the import wizard (S-16; S-17 reuses
 * it for snippets). Parsing is papaparse (R4, MIT): quoted cells, line
 * breaks inside quotes, and the delimiter is detected (`,` `;` tab `|`),
 * so a spreadsheet saved with semicolons (many European locales) works.
 *
 * Pure functions, no React: `parseTable` → `guessHeader` / `guessMapping`
 * → `mapRows`.
 */
import Papa from "papaparse";

/** A field the importer needs, e.g. "word" and "replacement". */
export interface ImportField {
  id: string;
  label: string;
  required: boolean;
  /** Header names (lowercase) that mean this field, for the first guess. */
  headerNames: string[];
}

export interface ParsedTable {
  /** Every non-empty record, cells as written. */
  records: string[][];
  /** Widest record. */
  columns: number;
  /** papaparse could not make sense of the text. */
  failed: boolean;
}

/** One mapped record: `row` is its 1-based record number in the file. */
export interface MappedRow {
  row: number;
  values: Record<string, string>;
}

/** Column index per field id; `null` = not in the file. */
export type ColumnMapping = Record<string, number | null>;

export function parseTable(text: string): ParsedTable {
  const clean = text.replace(/^﻿/, "");
  const result = Papa.parse<string[]>(clean, {
    skipEmptyLines: "greedy",
    delimitersToGuess: [",", ";", "\t", "|"],
  });
  const records = result.data.filter(
    (r) => Array.isArray(r) && r.some((c) => c.trim() !== ""),
  );
  const columns = records.reduce((max, r) => Math.max(max, r.length), 0);
  // A quote that never closes swallows the rest of the file: refuse it
  // rather than import one giant cell.
  const failed = result.errors.some((e) => e.type === "Quotes");
  return { records, columns, failed };
}

const normalizeHeader = (cell: string) =>
  cell
    .trim()
    .toLowerCase()
    .replace(/[\s_-]+/g, " ");

/** Whether the first record looks like a header row. */
export function guessHeader(table: ParsedTable, fields: ImportField[]) {
  const first = table.records[0];
  if (!first) return false;
  const names = new Set(fields.flatMap((f) => f.headerNames));
  return first.some((cell) => names.has(normalizeHeader(cell)));
}

/** First guess: header names, else the columns in field order. */
export function guessMapping(
  table: ParsedTable,
  fields: ImportField[],
  hasHeader: boolean,
): ColumnMapping {
  const mapping: ColumnMapping = {};
  const used = new Set<number>();
  if (hasHeader && table.records[0]) {
    const header = table.records[0].map(normalizeHeader);
    for (const field of fields) {
      const index = header.findIndex(
        (h, i) => !used.has(i) && field.headerNames.includes(h),
      );
      if (index >= 0) {
        mapping[field.id] = index;
        used.add(index);
      }
    }
  }
  let next = 0;
  for (const field of fields) {
    if (mapping[field.id] !== undefined) continue;
    while (used.has(next)) next++;
    mapping[field.id] = next < table.columns ? next : null;
    if (next < table.columns) used.add(next);
  }
  return mapping;
}

/** The records as field values (header skipped, NFC-normalized). */
export function mapRows(
  table: ParsedTable,
  fields: ImportField[],
  mapping: ColumnMapping,
  hasHeader: boolean,
): MappedRow[] {
  const start = hasHeader ? 1 : 0;
  const out: MappedRow[] = [];
  for (let i = start; i < table.records.length; i++) {
    const record = table.records[i];
    const values: Record<string, string> = {};
    for (const field of fields) {
      const column = mapping[field.id];
      const cell =
        column === null || column === undefined ? "" : record[column];
      values[field.id] = (cell ?? "").normalize("NFC");
    }
    out.push({ row: i + 1, values });
  }
  return out;
}

/** "A", "B", … "Z", "AA", … like a spreadsheet. */
export function columnLetter(index: number): string {
  let n = index + 1;
  let s = "";
  while (n > 0) {
    const m = (n - 1) % 26;
    s = String.fromCharCode(65 + m) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}
