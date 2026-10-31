// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * An app's Finder icon (`fork_style_app_icon`), or its first letter while
 * the icon loads or when the app has none. Decorative: the app's name is
 * always next to it or in its button's label.
 */
import { useAppIcon } from "./useStyle";

export function AppIcon({
  bundleId,
  name,
  size = 28,
}: {
  bundleId: string;
  name: string;
  size?: number;
}) {
  const url = useAppIcon(bundleId);
  if (url) {
    return (
      <img
        src={url}
        alt=""
        aria-hidden="true"
        width={size}
        height={size}
        className="shrink-0"
        draggable={false}
      />
    );
  }
  return (
    <span
      aria-hidden="true"
      style={{ width: size, height: size }}
      className="grid shrink-0 place-items-center rounded-[7px] bg-(--wb-selected) text-[12px] font-semibold text-(--wb-ink-2)"
    >
      {[...name.trim()][0]?.toLocaleUpperCase() ?? ""}
    </span>
  );
}
