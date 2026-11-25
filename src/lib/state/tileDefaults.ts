export const TILE_RANGE_DEFAULTS = {
	tile_slider_size: 80,
	tile_slider_size_y: 80,
	tile_slider_edge_inset: 8,
	tile_slider_speed: 80,
	tile_slider_rows: 2,
	tile_slider_border: 50,
	tile_slider_grid_gap: 8,
	tile_slider_gap: 8,
	tile_centered_blur: 80,
} as const;

export const TILE_SLIDER_EDGE_INSET = { min: 0, max: 24 };

export function resolveTileSliderEdgeInset(value?: number): number {
	return value == null || !Number.isFinite(value)
		? TILE_RANGE_DEFAULTS.tile_slider_edge_inset
		: Math.min(
				TILE_SLIDER_EDGE_INSET.max,
				Math.max(TILE_SLIDER_EDGE_INSET.min, Math.round(value)),
			);
}
