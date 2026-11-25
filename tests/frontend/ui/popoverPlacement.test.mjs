import { describe, expect, test } from "bun:test";
import { clampPopoverStart, horizontalPopover, sidebarPopover, verticalPopover, windowManagerPopover } from "../../../src/lib/utils/popoverPlacement.ts";

describe("launcher popover placement", () => {
	test("keeps the Play window list inside a right-aligned launcher", () => {
		expect(clampPopoverStart(632, 220, 800)).toBe(572);
		expect(clampPopoverStart(-40, 220, 800)).toBe(8);
	});

	test("opens context lists to either side without exceeding the viewport", () => {
		expect(horizontalPopover({ left: 20, right: 220 }, 800, 180, false, -2)).toEqual({ leftward: false, fits: true, left: 218 });
		expect(horizontalPopover({ left: 580, right: 780 }, 800, 180, false, -2)).toEqual({ leftward: true, fits: true, left: 402 });
	});

	test("opens the window manager below the top panel and above the whole bottom launch control", () => {
		const viewport = { width: 800, height: 600 };
		expect(windowManagerPopover({ left: 320, right: 480, top: 90, bottom: 144 }, viewport, "top"))
			.toMatchObject({ up: false, left: 290, top: 149, maxHeight: 443 });
		expect(windowManagerPopover({ left: 320, right: 480, top: 520, bottom: 574 }, viewport, "bottom"))
			.toMatchObject({ up: true, left: 290, bottom: 85, maxHeight: 507 });
	});

	test("keeps the assigned direction and clamps the window manager in a narrow viewport", () => {
		const viewport = { width: 204, height: 240 };
		expect(windowManagerPopover({ left: 44, right: 192, top: 152, bottom: 206 }, viewport, "top"))
			.toMatchObject({ up: false, left: 8, top: 211, maxHeight: 21 });
		expect(windowManagerPopover({ left: 10, right: 158, top: 34, bottom: 88 }, viewport, "bottom"))
			.toMatchObject({ up: true, left: 8, bottom: 211, maxHeight: 21 });
	});

	test("keeps a bottom-docked download panel above the button at both heights", () => {
		const anchor = { top: 540, bottom: 575 };
		for (const height of [320, 132]) {
			const placement = verticalPopover(anchor, 600, height, true, 6);
			expect(placement.up).toBe(true);
			expect(placement.bottom).toBe(66);
		}
	});

	test("flips a tall panel and caps its height to the space above the anchor", () => {
		expect(verticalPopover({ top: 220, bottom: 252 }, 300, 240, false, 6)).toEqual({ up: true, maxHeight: 206, top: 258, bottom: 86 });
	});

	test("sidebar flyouts enter from the trigger on all four docks", () => {
		const viewport = { width: 800, height: 600 };
		const panel = { width: 190, height: 90, maxHeight: 300 };
		const anchor = { left: 20, right: 52, top: 20, bottom: 52 };
		const place = (bounds, side, up = false, alignRight = false) =>
			sidebarPopover(bounds, viewport, panel, { side, up, alignRight });
		expect(place(anchor, "right")).toMatchObject({ left: 58, top: 8, x: -4, y: 0 });
		expect(place({ ...anchor, left: 748, right: 780 }, "left")).toMatchObject({ left: 552, top: 8, x: 4, y: 0 });
		expect(place(anchor, "none")).toMatchObject({ left: 20, top: 58, x: 0, y: -4 });
		expect(place({ left: 748, right: 780, top: 548, bottom: 580 }, "none", true, true)).toMatchObject({ left: 590, bottom: 58, x: 0, y: 4 });
	});

	test("a narrow sidebar flyout falls back to vertical placement and stays in bounds", () => {
		const placement = sidebarPopover(
			{ left: 12, right: 44, top: 190, bottom: 222 },
			{ width: 200, height: 240 },
			{ width: 184, height: 200, maxHeight: 224 },
			{ side: "right", up: false, alignRight: false },
		);
		expect(placement).toMatchObject({ left: 8, bottom: 56, maxHeight: 176, x: 0, y: 4 });
	});
});