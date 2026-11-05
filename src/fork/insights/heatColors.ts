// SPDX-License-Identifier: GPL-3.0-or-later
/** The heatmap's five band colours (`insights.css`), band 0 first. */
export const HEAT_COLORS = [0, 1, 2, 3, 4].map((l) => `var(--wb-heat-${l})`);
