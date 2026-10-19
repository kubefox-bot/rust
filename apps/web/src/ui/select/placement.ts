import type { MenuPlacement } from "./types";
import { MENU_MAX_HEIGHT_PX, MENU_GAP_PX, VIEWPORT_MARGIN_PX } from "./conts";

export const menuPlacement = ({
  top,
  bottom,
  viewportHeight,
}: Readonly<{
  top: number;
  bottom: number;
  viewportHeight: number;
}>): MenuPlacement => {
  const below = Math.max(
    0,
    viewportHeight - bottom - MENU_GAP_PX - VIEWPORT_MARGIN_PX,
  );
  const above = Math.max(0, top - MENU_GAP_PX - VIEWPORT_MARGIN_PX);
  const side =
    below >= MENU_MAX_HEIGHT_PX || below >= above ? "below" : "above";
  return {
    side,
    maxHeight: Math.min(MENU_MAX_HEIGHT_PX, side === "below" ? below : above),
  };
};
