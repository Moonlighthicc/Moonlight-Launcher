/** Clip only the anchored edge, without resizing the animation viewport. */
export function tileSliderInsets(
	inset: number,
	width: number,
	scale: number,
	side: "left" | "right" | "center",
) {
	const x = Math.min(inset, Math.max(0, width - 24 * scale));
	return {
		left: side === "left" ? x : 0,
		right: side === "right" ? x : 0,
	};
}
