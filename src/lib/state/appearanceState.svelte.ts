/**
 * Ajustes visuales que viven solo en la interfaz (se guardan en localStorage):
 *  - `sliderMotion`: cuándo se mueven las cintas de los mosaicos.
 */
export type SliderMotion = "always" | "hover" | "off";
export const SLIDER_MOTIONS: SliderMotion[] = ["always", "hover", "off"];

const KEY = "cubic.appearance.v1";

function validAccent(color: unknown): color is string {
	return typeof color === "string" && /^#[\da-f]{6}$/i.test(color);
}

function load() {
	const saved = { sliderMotion: "always" as SliderMotion, quickActionsDisabled: true, accentColor: null as string | null };
	try {
		const raw = localStorage.getItem(KEY);
		if (!raw) return saved;
		const data = JSON.parse(raw) as { sliderMotion?: unknown; quickActionsDisabled?: unknown; accentColor?: unknown };
		if (validAccent(data.accentColor)) saved.accentColor = data.accentColor;
		if (SLIDER_MOTIONS.includes(data.sliderMotion as SliderMotion))
			saved.sliderMotion = data.sliderMotion as SliderMotion;
		if (typeof data.quickActionsDisabled === "boolean")
			saved.quickActionsDisabled = data.quickActionsDisabled;
	} catch {
		// localStorage no disponible: se usan los valores por defecto
	}
	return saved;
}

export const appearance = $state(load());

export function setAccentColor(color: string | null, persist = true) {
	appearance.accentColor = validAccent(color) ? color : null;
	if (persist) {
		try { localStorage.setItem(KEY, JSON.stringify(appearance)); } catch { /* optional persistence */ }
	}
}

export function applyAccentColor(color: string | null) {
	const style = document.documentElement.style;
	const properties = ["--accent", "--accent-rgb", "--accent-hover", "--accent-text", "--accent-primary", "--focus-ring", "--text-on-accent"];
	if (!validAccent(color)) {
		for (const property of properties) style.removeProperty(property);
		return;
	}
	const rgb = [1, 3, 5].map((start) => parseInt(color.slice(start, start + 2), 16));
	const [r, g, b] = rgb.map((channel) => {
		const s = channel / 255;
		return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
	});
	const darkText = 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.179;
	const text = darkText ? "#000000" : "#ffffff";
	const hover = `color-mix(in srgb, ${color} 92%, ${darkText ? "white" : "black"})`;
	const values = [color, rgb.join(", "), hover, text, color, color, text];
	properties.forEach((property, index) => style.setProperty(property, values[index]));
}

export function setSliderMotion(mode: SliderMotion) {
	appearance.sliderMotion = mode;
	try {
		localStorage.setItem(KEY, JSON.stringify(appearance));
	} catch {
		// sin persistencia
	}
}

export function setQuickActionsDisabled(disabled: boolean) {
	appearance.quickActionsDisabled = disabled;
	try {
		localStorage.setItem(KEY, JSON.stringify(appearance));
	} catch {
		// sin persistencia
	}
}
