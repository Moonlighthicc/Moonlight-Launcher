import { expect, test } from "bun:test";
import { limitSidebarSize, sidebarGeometry, sidebarSizes } from "../../../src/lib/utils/sidebarResize.ts";

for (const axis of ["vertical", "horizontal"]) {
	const { min, compact, compactMax, threshold, normalMin, normal, max } = sidebarSizes[axis];
	test(`${axis} sidebar snaps to compact and can be resized in both modes`, () => {
		expect(sidebarGeometry(axis, compact)).toEqual({ compact: true, size: compact, scale: 1 });
		expect(sidebarGeometry(axis, -1).size).toBe(min);
		expect(sidebarGeometry(axis, threshold)).toEqual({ compact: true, size: compactMax, scale: 1 });
		expect(sidebarGeometry(axis, threshold + 1)).toMatchObject({ compact: false, size: normalMin });
		expect(sidebarGeometry(axis, max + 100).size).toBe(max);
		expect(limitSidebarSize(axis, max, true)).toBe(compactMax);
	});
	test(`${axis} sidebar limits card scaling to 15 percent`, () => {
		expect(sidebarGeometry(axis, normal).scale).toBe(1);
		expect(sidebarGeometry(axis, normalMin).scale).toBeGreaterThanOrEqual(0.85);
		expect(sidebarGeometry(axis, normalMin).scale).toBeLessThan(1);
	});
}