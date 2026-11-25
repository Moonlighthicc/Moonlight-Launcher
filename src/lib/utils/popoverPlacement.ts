export const POPOVER_MARGIN = 8;

export function clampPopoverStart(start: number, size: number, viewport: number) {
	return Math.max(POPOVER_MARGIN, Math.min(start, viewport - size - POPOVER_MARGIN));
}

export function verticalPopover(
	anchor: { top: number; bottom: number },
	viewport: number,
	preferredHeight: number,
	preferUp: boolean,
	gap: number,
) {
	const above = Math.max(0, anchor.top - gap - POPOVER_MARGIN);
	const below = Math.max(0, viewport - anchor.bottom - gap - POPOVER_MARGIN);
	const preferred = preferUp ? above : below;
	const alternative = preferUp ? below : above;
	const up = preferred < preferredHeight && alternative > preferred ? !preferUp : preferUp;
	const maxHeight = up ? above : below;
	return { up, maxHeight, top: anchor.bottom + gap, bottom: viewport - anchor.top + gap };
}

export function horizontalPopover(
	anchor: { left: number; right: number },
	viewport: number,
	width: number,
	preferLeft: boolean,
	gap: number,
) {
	const before = anchor.left - gap - POPOVER_MARGIN;
	const after = viewport - anchor.right - gap - POPOVER_MARGIN;
	const preferred = preferLeft ? before : after;
	const alternative = preferLeft ? after : before;
	const leftward = preferred < width && alternative > preferred ? !preferLeft : preferLeft;
	return {
		leftward,
		fits: (leftward ? before : after) >= width,
		left: clampPopoverStart(leftward ? anchor.left - gap - width : anchor.right + gap, width, viewport),
	};
}

/** Keep the window list on the content side of the game panel, scrolling if needed. */
export function windowManagerPopover(
	anchor: { left: number; right: number; top: number; bottom: number },
	viewport: { width: number; height: number },
	dock: "top" | "bottom",
) {
	const width = Math.min(220, viewport.width - 2 * POPOVER_MARGIN);
	return {
		...verticalPopover(anchor, viewport.height, 0, dock === "bottom", 5),
		left: clampPopoverStart((anchor.left + anchor.right - width) / 2, width, viewport.width),
	};
}

/** Layout and entry direction shared by sidebar flyouts. */
export function sidebarPopover(
	anchor: { left: number; right: number; top: number; bottom: number },
	viewport: { width: number; height: number },
	panel: { width: number; height: number; maxHeight: number },
	options: { side: "left" | "right" | "none"; up: boolean; alignRight: boolean },
) {
	const gap = 6;
	const side = horizontalPopover(anchor, viewport.width, panel.width, options.side === "left", gap);
	if (options.side !== "none" && side.fits) {
		const top = clampPopoverStart((anchor.top + anchor.bottom - panel.height) / 2, panel.height, viewport.height);
		return { left: side.left, top, bottom: undefined, maxHeight: panel.maxHeight, x: side.leftward ? 4 : -4, y: 0 };
	}
	const vertical = verticalPopover(anchor, viewport.height, panel.height, options.up, gap);
	const left = clampPopoverStart(options.alignRight ? anchor.right - panel.width : anchor.left, panel.width, viewport.width);
	return {
		left, top: vertical.up ? undefined : vertical.top, bottom: vertical.up ? vertical.bottom : undefined,
		maxHeight: Math.min(panel.maxHeight, vertical.maxHeight), x: 0, y: vertical.up ? 4 : -4,
	};
}

export function sidebarPopoverStyle({ left, top, bottom, maxHeight }: ReturnType<typeof sidebarPopover>) {
	return `left:${left}px;${top !== undefined ? `top:${top}` : `bottom:${bottom}`}px;max-height:${maxHeight}px`;
}