// SPDX-License-Identifier: GPL-3.0-or-later
//
// Pure analysis of a wb-egress capture (R5 §4.12 "Network: runtime egress").
// Inputs are the text logs written by `session.ts`:
//   - pflog.txt: `tcpdump -n -e -tttt -i pflog0`. The anchor only logs packets
//     it BLOCKS for the app user, so every packet line is a connection
//     attempt outside the allowlist.
//   - dns.txt: `tcpdump -n -l -k NP -i pktap,all port 53`; `-k NP` adds the
//     process (and, for mDNSResponder, the delegating "eproc") per packet.
//   - app.log: the app's own log (fork selfcheck lines).

import { hostInList } from "../wb-guard-lib";

export interface DnsQuery {
  name: string;
  proc?: string;
  eproc?: string;
  line: number;
}

export interface EgressReport {
  blocked: string[];
  dnsViolations: DnsQuery[];
  dnsUnattributed: DnsQuery[];
  dnsAllowed: string[];
  selfcheck: "PASS" | "FAIL" | "MISSING";
  selfcheckLine?: string;
  updateCheckSeen: boolean;
  failures: string[];
}

const QUERY_RE =
  /\b(?:A|AAAA|HTTPS|SVCB|CNAME|TXT|SRV|PTR|ANY)\?\s+(\S+?)\.?\s/;
const META_RE = /\bproc\s+([^:,()]+):\d+/;
const EMETA_RE = /\beproc\s+([^:,()]+):\d+/;

export function parseDns(text: string): DnsQuery[] {
  const out: DnsQuery[] = [];
  text.split("\n").forEach((line, i) => {
    const q = line.match(QUERY_RE);
    if (!q) return;
    // Responses repeat the question after the answer count; keep queries.
    if (/\s\d+\/\d+\/\d+\s/.test(line)) return;
    out.push({
      name: q[1].toLowerCase().replace(/\.$/, ""),
      proc: line.match(META_RE)?.[1]?.trim(),
      eproc: line.match(EMETA_RE)?.[1]?.trim(),
      line: i + 1,
    });
  });
  return out;
}

export function analyze(input: {
  pflog: string;
  dns: string;
  appLog: string;
  allowedHosts: string[];
  ourProcesses: string[];
  strictDns?: boolean;
}): EgressReport {
  const failures: string[] = [];
  const blocked = input.pflog
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => /\bblock\b/.test(l) && /\s>\s/.test(l));
  if (blocked.length) {
    failures.push(
      `${blocked.length} outbound packet(s) from the app user were blocked (destination outside R5 §1)`,
    );
  }

  const ours = new Set(input.ourProcesses.map((p) => p.toLowerCase()));
  const isOurs = (q: DnsQuery) =>
    [q.proc, q.eproc].some((p) => p && ours.has(p.toLowerCase()));
  const dnsViolations: DnsQuery[] = [];
  const dnsUnattributed: DnsQuery[] = [];
  const dnsAllowed = new Set<string>();
  for (const q of parseDns(input.dns)) {
    const allowed =
      hostInList(q.name, input.allowedHosts) ||
      q.name.endsWith(".local") ||
      q.name.endsWith(".arpa");
    if (allowed) {
      if (isOurs(q)) dnsAllowed.add(q.name);
      continue;
    }
    if (isOurs(q) || (input.strictDns && !q.proc && !q.eproc)) {
      dnsViolations.push(q);
    } else if (!q.proc && !q.eproc) {
      dnsUnattributed.push(q);
    }
  }
  if (dnsViolations.length) {
    failures.push(
      `DNS queries from the app outside R5 §1: ${[...new Set(dnsViolations.map((q) => q.name))].join(", ")}`,
    );
  }

  const pass = input.appLog.match(/fork selfcheck: PASS[^\n]*/);
  const fail = input.appLog.match(/fork selfcheck: FAIL[^\n]*/);
  const selfcheck = fail ? "FAIL" : pass ? "PASS" : "MISSING";
  if (selfcheck !== "PASS") {
    failures.push(
      selfcheck === "FAIL"
        ? `self-check failed: ${fail?.[0]}`
        : "self-check line missing: the app never reached fork::on_ready",
    );
  }
  const updateCheckSeen =
    dnsAllowed.has("github.com") ||
    /updater|check for update/i.test(input.appLog);

  return {
    blocked,
    dnsViolations,
    dnsUnattributed,
    dnsAllowed: [...dnsAllowed].sort(),
    selfcheck,
    selfcheckLine: (fail ?? pass)?.[0],
    updateCheckSeen,
    failures,
  };
}
