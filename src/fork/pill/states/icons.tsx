// SPDX-License-Identifier: GPL-3.0-or-later
// Small line icons drawn for the bar (our own, 16 × 16 grid).

const common = {
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
  focusable: false,
};

export function CloseIcon() {
  return (
    <svg {...common}>
      <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
    </svg>
  );
}

export function CheckIcon() {
  return (
    <svg {...common}>
      <path d="M3.5 8.5l3 3 6-7" />
    </svg>
  );
}

export function MicIcon() {
  return (
    <svg {...common} strokeWidth={1.5}>
      <rect x="5.75" y="2" width="4.5" height="7.5" rx="2.25" />
      <path d="M3.5 7.5a4.5 4.5 0 009 0M8 12v2" />
    </svg>
  );
}

/** Return key (S-19a "Sends automatically"). */
export function ReturnIcon() {
  return (
    <svg {...common} strokeWidth={1.6}>
      <path d="M12.5 3.5v4.75a1.75 1.75 0 01-1.75 1.75H3.75M6.5 7l-2.75 3 2.75 3" />
    </svg>
  );
}

export function AlertIcon() {
  return (
    <svg {...common} strokeWidth={1.6}>
      <circle cx="8" cy="8" r="6" />
      <path d="M8 4.75v3.75M8 11.1v.15" />
    </svg>
  );
}

export function InfoIcon() {
  return (
    <svg {...common} strokeWidth={1.6}>
      <circle cx="8" cy="8" r="6" />
      <path d="M8 7.25v4M8 4.9v.15" />
    </svg>
  );
}

export function CopyIcon() {
  return (
    <svg {...common} strokeWidth={1.5}>
      <rect x="5.5" y="5.5" width="8" height="8" rx="1.75" />
      <path d="M10.5 3.5v-.25A1.25 1.25 0 009.25 2h-6A1.25 1.25 0 002 3.25v6A1.25 1.25 0 003.25 10.5h.25" />
    </svg>
  );
}

export function LockIcon() {
  return (
    <svg {...common} strokeWidth={1.5}>
      <rect x="3.25" y="7" width="9.5" height="7" rx="1.75" />
      <path d="M5.5 7V5a2.5 2.5 0 015 0v2" />
    </svg>
  );
}
