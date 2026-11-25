import { launcherStore } from "$lib/state/state.svelte";

export const WINDOW_EASING = "cubic-bezier(0.32, 0.72, 0, 1)";
export const WINDOW_ENTER_MS = 350;
export const WINDOW_EXIT_MS = 250;

/**
 * Devuelve `true` si el usuario habilitó la reducción de animaciones.
 * Úsalo dentro de componentes Svelte para que la lectura sea reactiva.
 */
export function shouldReduceAnimations(): boolean {
	return launcherStore.settings.reduce_animations ||
		(typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
}

/**
 * Devuelve la duración de animación correspondiente.
 * @param normal duración normal en ms
 * @param reduced duración cuando están reducidas (por defecto 0 = instantáneo)
 */
export function animDuration(normal: number, reduced = 0): number {
	return shouldReduceAnimations() ? reduced : normal;
}

type WindowEdge = "top" | "right" | "bottom" | "left";

/** Slide a surface fully past its edge, preserving the existing drawer easing. */
export function windowSlide(
	_node: HTMLElement,
	{ edge = "bottom", viewport = false, distance, fade = false }: {
		edge?: WindowEdge;
		viewport?: boolean;
		distance?: number;
		fade?: boolean;
	} = {},
	{ direction }: { direction: "in" | "out" | "both" },
) {
	const horizontal = edge === "left" || edge === "right";
	const amount = distance ?? (viewport ? (horizontal ? "100vw" : "100vh") : "100%");
	const offset = `${edge === "left" || edge === "top" ? "-" : ""}${typeof amount === "number" ? `${amount}px` : amount}`;
	return {
		duration: animDuration(direction === "out" ? WINDOW_EXIT_MS : WINDOW_ENTER_MS),
		css: (t: number, u: number) =>
			`transform: translate${horizontal ? "X" : "Y"}(calc(${u} * ${offset}));${fade ? `opacity: ${t};` : ""}`,
		easing: windowEase,
	};
}

// Invert x on the same cubic Bézier curve used by CSS transitions.
function windowEase(x: number): number {
	if (x <= 0 || x >= 1) return x;
	let low = 0;
	let high = 1;
	for (let i = 0; i < 16; i++) {
		const t = (low + high) / 2;
		if (3 * (1 - t) ** 2 * t * 0.32 + t ** 3 < x) low = t;
		else high = t;
	}
	const t = (low + high) / 2;
	return 3 * (1 - t) ** 2 * t * 0.72 + 3 * (1 - t) * t ** 2 + t ** 3;
}
