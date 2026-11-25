const REFERENCE_WIDTH = 900;
const REFERENCE_HEIGHT = 480;

export function tileBoardGeometry(availableWidth: number, availableHeight: number) {
	if (!(availableWidth > 0 && availableHeight > 0)) return null;

	const scale = Math.min(1, availableWidth / REFERENCE_WIDTH, availableHeight / REFERENCE_HEIGHT);
	// Grow the logical canvas on the non-limiting axis before scaling so the
	// rendered board fills the viewport without stretching the tile contents.
	return {
		width: availableWidth / scale,
		height: availableHeight / scale,
		scale,
	};
}