/**
 * Estado compartido de la pantalla de instancia:
 *  - `instanceNav.section`: sección abierta del menú "Datos" ("detalles" = pantalla principal).
 *    Vive aquí (y no dentro de InstanceView) para que la barra lateral sepa si hay un
 *    submenú abierto y el primer clic sobre la instancia lo cierre en lugar de deseleccionarla.
 *  - `tileLayout`: modo de mosaicos y posición/tamaño de cada mosaico, guardado en
 *    localStorage para que sobreviva al cierre del lanzador y al cambio de instancia.
 */

export const GRID_COLS = 60;
export const GRID_ROWS = 40;
// Minimum tile size in grid cells, independent of the window dimensions.
export const MIN_TILE_W = 11;
export const MIN_TILE_H = 3;

export const TILE_IDS = [
	"market",
	"resources",
	"shaderpacks",
	"worlds",
	"servers",
	"screenshots",
] as const;

export type TileId = (typeof TILE_IDS)[number];

export interface TileRect {
	x: number;
	y: number;
	w: number;
	h: number;
}

const STORAGE_KEY = "cubic.instanceTiles.v1";

export const instanceNav = $state({ section: "detalles", openInstalled: false, editActionGuard: null as ((action: () => void) => boolean) | null });

/** Mantiene el rectángulo dentro de la cuadrícula y con tamaño mínimo, en celdas enteras. */
export function clampRect(rect: TileRect): TileRect {
	const w = Math.min(GRID_COLS, Math.max(MIN_TILE_W, Math.round(rect.w)));
	const h = Math.min(GRID_ROWS, Math.max(MIN_TILE_H, Math.round(rect.h)));
	const x = Math.min(GRID_COLS - w, Math.max(0, Math.round(rect.x)));
	const y = Math.min(GRID_ROWS - h, Math.max(0, Math.round(rect.y)));
	return { x, y, w, h };
}

/** Disposición inicial: seis mosaicos en 2 columnas × 3 filas, abajo a la izquierda. */
function defaultRects(): Record<TileId, TileRect> {
	const rects = {} as Record<TileId, TileRect>;
	TILE_IDS.forEach((id, index) => {
		rects[id] = clampRect({
			x: (index % 2) * 25,
			y: 22 + Math.floor(index / 2) * 6,
			w: 24,
			h: 5,
		});
	});
	return rects;
}

/** Disposición inicial de la primera versión (mosaicos estrechos, sin espacio para la cinta). */
function legacyDefaultRects(): Record<TileId, TileRect> {
	const rects = {} as Record<TileId, TileRect>;
	TILE_IDS.forEach((id, index) => {
		rects[id] = clampRect({
			x: (index % 3) * 15,
			y: 28 + Math.floor(index / 3) * 6,
			w: 14,
			h: 5,
		});
	});
	return rects;
}

function sameRects(
	a: Record<TileId, TileRect>,
	b: Record<TileId, TileRect>,
): boolean {
	return TILE_IDS.every(
		(id) =>
			a[id].x === b[id].x &&
			a[id].y === b[id].y &&
			a[id].w === b[id].w &&
			a[id].h === b[id].h,
	);
}

function isRect(value: unknown): value is TileRect {
	if (typeof value !== "object" || value === null) return false;
	const v = value as Record<string, unknown>;
	return ["x", "y", "w", "h"].every(
		(key) => typeof v[key] === "number" && Number.isFinite(v[key]),
	);
}

function load(): {
	enabled: boolean;
	rects: Record<TileId, TileRect>;
	zOrder: TileId[];
	hidden: TileId[];
	noBlur: TileId[];
} {
	const rects = defaultRects();
	let enabled = false;
	let zOrder: TileId[] = [...TILE_IDS];
	let hidden: TileId[] = [];
	let noBlur: TileId[] = [];
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (raw) {
			const saved = JSON.parse(raw) as {
				enabled?: unknown;
				rects?: Record<string, unknown>;
				zOrder?: unknown;
				hidden?: unknown;
				noBlur?: unknown;
			};
			if (Array.isArray(saved.noBlur))
				noBlur = saved.noBlur.filter((id): id is TileId => TILE_IDS.includes(id as TileId));
			enabled = saved.enabled === true;
			if (Array.isArray(saved.zOrder)) {
				const valid = saved.zOrder.filter((id): id is TileId =>
					TILE_IDS.includes(id as TileId),
				);
				zOrder = [...new Set([...valid, ...TILE_IDS.filter((id) => !valid.includes(id))])];
			}
			if (Array.isArray(saved.hidden))
				hidden = saved.hidden.filter((id): id is TileId => TILE_IDS.includes(id as TileId));
			for (const id of TILE_IDS) {
				const candidate = saved.rects?.[id];
				if (isRect(candidate)) rects[id] = clampRect(candidate);
			}
		}
	} catch {
		/* localStorage no disponible o JSON dañado: se usan los valores por defecto */
	}
	// Quien nunca movió los mosaicos pasa a la nueva disposición, más ancha.
	if (sameRects(rects, legacyDefaultRects()))
		return { enabled, rects: defaultRects(), zOrder, hidden, noBlur };
	return { enabled, rects, zOrder, hidden, noBlur };
}

export const tileLayout = $state(load());

function save() {
	try {
		localStorage.setItem(
			STORAGE_KEY,
			JSON.stringify({
				enabled: tileLayout.enabled,
				rects: tileLayout.rects,
				zOrder: tileLayout.zOrder,
				hidden: tileLayout.hidden,
				noBlur: tileLayout.noBlur,
			}),
		);
	} catch {
		/* sin almacenamiento: el diseño solo dura esta sesión */
	}
}

export function moveTile(id: TileId, direction: -1 | 1 | "front" | "back") {
	const order = tileLayout.zOrder.filter((item) => item !== id);
	if (direction === "front") order.push(id);
	else if (direction === "back") order.unshift(id);
	else {
		const current = tileLayout.zOrder.indexOf(id);
		const next = Math.max(0, Math.min(TILE_IDS.length - 1, current + direction));
		order.splice(next, 0, id);
	}
	tileLayout.zOrder = order;
}

export function setTileHidden(id: TileId, hidden: boolean) {
	tileLayout.hidden = hidden
		? [...new Set([...tileLayout.hidden, id])]
		: tileLayout.hidden.filter((item) => item !== id);
}

/** Отключает размытие стеклянной подложки для отдельной плитки. */
export function setTileNoBlur(id: TileId, noBlur: boolean) {
	tileLayout.noBlur = noBlur
		? [...new Set([...tileLayout.noBlur, id])]
		: tileLayout.noBlur.filter((item) => item !== id);
	save();
}

export function setTileMode(enabled: boolean) {
	tileLayout.enabled = enabled;
	save();
}

export function setTileRect(id: TileId, rect: TileRect) {
	tileLayout.rects[id] = clampRect(rect);
}

/** Guarda en disco; se llama al soltar el mosaico, no en cada movimiento. */
export function commitTileLayout() {
	save();
}
