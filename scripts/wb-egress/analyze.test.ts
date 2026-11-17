// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the wb-egress log analysis and the generated pf anchor.

import { describe, expect, test } from "bun:test";
import { allowedAnswers, analyze, parseDns } from "./analyze";
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

describe("wildcard hosts resolved during the session", () => {
  const allowed = ["github.com", "*.hf.co"];
  const dns = [
    "11:45:15.848114 (proc mDNSResponder:187, eproc wispr-bro:1) IP 10.0.0.9.53302 > 10.0.0.1.53: 5908+ A? us.aws.cdn.hf.co. (34)",
    "11:45:15.871361 (proc mDNSResponder:187, eproc wispr-bro:1) IP 10.0.0.1.53 > 10.0.0.9.53302: 5908 2/0/0 A 54.68.202.150, A 52.88.162.194 (66)",
    "11:45:16.000000 (proc mDNSResponder:187, eproc wispr-bro:1) IP 10.0.0.9.53303 > 10.0.0.1.53: 77+ A? evil.example. (30)",
    "11:45:16.100000 (proc mDNSResponder:187, eproc wispr-bro:1) IP 10.0.0.1.53 > 10.0.0.9.53303: 77 1/0/0 A 9.9.9.9 (46)",
  ].join("\n");
  const block = (ip: string) =>
    `2026-11-16 11:45:15.969314 rule 0.250.WisprBroEgr.4/0(match): block out on en0: 10.0.0.9.49501 > ${ip}.443: Flags [S]`;
  const run = (pflog: string) =>
    analyze({
      pflog,
      dns,
      appLog: "fork selfcheck: PASS",
      allowedHosts: allowed,
      ourProcesses: ["wispr-bro"],
    });

  test("an address answered for an allowed wildcard name is not a violation", () => {
    expect(allowedAnswers(dns, allowed)).toEqual(
      new Set(["54.68.202.150", "52.88.162.194"]),
    );
    expect(run(block("54.68.202.150")).blocked).toEqual([]);
  });

  test("an address answered for any other name still fails", () => {
    expect(run(block("9.9.9.9")).blocked).toHaveLength(1);
    expect(run(block("1.2.3.4")).failures[0]).toContain("blocked");
  });
});
