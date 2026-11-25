/**
 * Вспомогательные функции стиля плиток (настройки живут в `launcherStore.settings`).
 */
import type { Settings, TileSide, TileStyle, TileTextPos } from "$lib/types/types";
import { TILE_RANGE_DEFAULTS } from "./tileDefaults";

type TileSettings = Pick<Settings, "tile_side" | "tile_style" | "tile_text_pos">;

/** Сторона текста плитки; по умолчанию «Автоматически» (определяется по положению плитки). */
export function resolveTileSide(settings: TileSettings): TileSide {
	const side = settings.tile_side;
	return side === "left" || side === "right" ? side : "auto";
}

/**
 * Итоговая сторона конкретной плитки. В режиме «auto» плитка в правой половине
 * сетки получает текст справа, в левой — слева.
 */
export function sideForRect(
	side: TileSide,
	rect: { x: number; w: number },
	gridCols: number,
): "left" | "right" {
	if (side === "left" || side === "right") return side;
	return rect.x + rect.w / 2 > gridCols / 2 ? "right" : "left";
}

/** Интенсивность размытия стеклянной подложки, % (0–80, по умолчанию 80). */
export function resolveGlassBlur(settings: Pick<Settings, "tile_centered_blur">): number {
	const value = Number(settings.tile_centered_blur);
	if (settings.tile_centered_blur == null || !Number.isFinite(value)) return TILE_RANGE_DEFAULTS.tile_centered_blur;
	return Math.min(80, Math.max(0, Math.round(value)));
}

export function resolveTileStyle(settings: TileSettings): TileStyle {
	const style = settings.tile_style;
	return style === "stylish" || style === "glass" ? style : "standard";
}

export function resolveTextPos(settings: TileSettings): TileTextPos {
	return settings.tile_text_pos ?? "auto";
}

/**
 * Процент, который видит пользователь для размера полосы. Сохранённое значение 80 —
 * исходный масштаб, поэтому показываем его как 100%. Тот же зажим, что и в InstanceView.
 */
export function sliderScalePercent(size: number): number {
	const scale = Math.min(1.25, Math.max(0.3 / 0.8, size / 80));
	return Math.round(scale * 100);
}
