// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the wb-egress log analysis and the generated pf anchor.

import { describe, expect, test } from "bun:test";
import { analyze, parseDns } from "./analyze";
import { anchorRules } from "./session";

const allowedHosts = ["github.com", "huggingface.co", "*.hf.co", "localhost"];
const PASS = "[INFO] fork selfcheck: PASS (release build) ok\n";

const dnsLine = (name: string, meta: string) =>
  `12:00:00.000000 ${meta} IP 192.168.64.2.53000 > 8.8.8.8.53: 4242+ A? ${name}. (32)`;

describe("egress analysis", () => {
  test("clean session passes", () => {
    const r = analyze({
      pflog: "",
      dns: [
        dnsLine("github.com", "(proc mDNSResponder:99, eproc handy:1234)"),
        dnsLine(
          "pipelines.actions.githubusercontent.com",
          "(proc Runner.Listener:55)",
        ),
      ].join("\n"),
      appLog: PASS,
      allowedHosts,
      ourProcesses: ["handy"],
    });
    expect(r.failures).toEqual([]);
    expect(r.dnsAllowed).toEqual(["github.com"]);
    expect(r.updateCheckSeen).toBe(true);
  });

  test("a blocked packet, an app DNS query outside §1 and a missing self-check fail", () => {
    const r = analyze({
      pflog:
        "2026-10-29 00:00:00.0 rule 3/0(match): block out on en0: 192.168.64.2.50000 > 203.0.113.9.443: Flags [S]",
      dns: dnsLine(
        "api.openai.com",
        "(proc mDNSResponder:99, eproc handy:1234)",
      ),
      appLog: "",
      allowedHosts,
      ourProcesses: ["handy"],
    });
    expect(r.failures.length).toBe(3);
    expect(r.dnsViolations.map((q) => q.name)).toEqual(["api.openai.com"]);
    expect(r.selfcheck).toBe("MISSING");
  });

  test("unattributed queries are reported, and fail only with strictDns", () => {
    const input = {
      pflog: "",
      dns: dnsLine("example.org", ""),
      appLog: PASS,
      allowedHosts,
      ourProcesses: ["handy"],
    };
    expect(analyze(input).failures).toEqual([]);
    expect(analyze(input).dnsUnattributed.map((q) => q.name)).toEqual([
      "example.org",
    ]);
    expect(analyze({ ...input, strictDns: true }).failures.length).toBe(1);
  });

  test("responses are not counted as queries", () => {
    const resp =
      "12:00:00.1 (proc mDNSResponder:99) IP 8.8.8.8.53 > 192.168.64.2.53000: 4242 1/0/0 A? api.openai.com. A 1.2.3.4 (48)";
    expect(parseDns(resp)).toEqual([]);
  });
});

describe("pf anchor", () => {
  test("blocks and logs the app user except loopback, DNS and allowed IPs", () => {
    const rules = anchorRules("wbegress", ["140.82.112.3"]);
    expect(rules).toContain("pass out quick on lo0 all");
    expect(rules).toContain("port 53 user wbegress");
    expect(rules).toContain("to <wb_allowed> port 443 user wbegress");
    expect(rules.trim().split("\n").pop()).toBe(
      "block drop out log quick proto { tcp, udp } from any to any user wbegress",
    );
  });
});
