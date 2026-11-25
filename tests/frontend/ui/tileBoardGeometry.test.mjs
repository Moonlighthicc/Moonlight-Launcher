import { expect, test } from "bun:test";
import { tileBoardGeometry } from "../../../src/lib/utils/tileBoardGeometry.ts";

test.each([
	[1200, 240, 0.5],
	[450, 600, 0.5],
	[720, 350, 350 / 480],
	[1440, 800, 1],
])("the tile board fills a %i × %i viewport at one uniform scale", (width, height, scale) => {
	const board = tileBoardGeometry(width, height);
	expect(board.scale).toBeCloseTo(scale, 8);
	expect(board.width * board.scale).toBeCloseTo(width, 8);
	expect(board.height * board.scale).toBeCloseTo(height, 8);
});

test("an unmeasured tile viewport does not produce a zero scale or infinite dimensions", () => {
	expect(tileBoardGeometry(0, 480)).toBeNull();
	expect(tileBoardGeometry(900, 0)).toBeNull();
});