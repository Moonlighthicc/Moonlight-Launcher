import { expect, test } from "bun:test";
import { resolveTileSliderEdgeInset } from "../../../src/lib/state/tileDefaults.ts";
import { tileSliderInsets } from "../../../src/lib/utils/tileSliderInsets.ts";

test("slider inset defaults to 8 px and is limited to the settings range", () => {
	expect(resolveTileSliderEdgeInset()).toBe(8);
	expect(resolveTileSliderEdgeInset(-1)).toBe(0);
	expect(resolveTileSliderEdgeInset(40)).toBe(24);
});

test("animated ribbons inset only the anchored horizontal edge", () => {
	expect(tileSliderInsets(8, 100, 1, "left")).toEqual({
		left: 8,
		right: 0,
	});
	expect(tileSliderInsets(8, 100, 1, "right")).toEqual({
		left: 0,
		right: 8,
	});
	expect(tileSliderInsets(8, 100, 1.25, "center")).toEqual({
		left: 0,
		right: 0,
	});
});

test("narrow tiles retain room for the ribbon", () => {
	expect(tileSliderInsets(24, 32, 1.25, "right")).toEqual({
		left: 0,
		right: 2,
	});
});

test("1–5 px insets stay unchanged at fractional ribbon scales", () => {
	for (let inset = 1; inset <= 5; inset++) {
		expect(tileSliderInsets(inset, 100.5, 1.125, "left")).toEqual({
			left: inset,
			right: 0,
		});
		expect(tileSliderInsets(inset, 100.5, 0.875, "right")).toEqual({
			left: 0,
			right: inset,
		});
	}
});
