import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { interfacePreferences } from "$lib/utils/interfacePreferences";

export function applyInterfaceDensity(density: string): void {
	const value = interfacePreferences({ density }).density;
	const root = document.documentElement;
	const attribute = value === "theme" ? null : value;
	if (root.getAttribute("data-interface-density") === attribute) return;
	if (attribute === null) root.removeAttribute("data-interface-density");
	else root.setAttribute("data-interface-density", attribute);
}

/** "theme" uses the active theme's --font-family; any other value overrides it
 * globally via an inline custom property on <html>, which wins over the theme
 * stylesheet and survives theme switches. */
export function applyInterfaceFont(font: string): void {
	const value = interfacePreferences({ font }).font;
	const root = document.documentElement;
	if (value === "theme") root.style.removeProperty("--font-family");
	else
		root.style.setProperty(
			"--font-family",
			`"${value}", system-ui, sans-serif`,
		);
}

const settled = Promise.resolve();
let zoomQueue = settled;
let appliedScale: number | undefined;
let lastRequest:
	{ scale: number; promise: Promise<void>; signal?: AbortSignal } | undefined;

interface ScaleTransition {
	duration?: number;
	delay?: number;
	signal?: AbortSignal;
}

function wait(ms: number, signal?: AbortSignal): Promise<void> {
	if (signal?.aborted) return settled;
	return new Promise((resolve) => {
		const timer = setTimeout(finish, ms);
		function finish() {
			clearTimeout(timer);
			signal?.removeEventListener("abort", finish);
			resolve();
		}
		signal?.addEventListener("abort", finish, { once: true });
	});
}

/** Native page zoom keeps viewport units, portals, pointer coordinates and
 * ResizeObserver in the same CSS coordinate system. Never rewrite theme CSS. */
export function applyInterfaceScale(
	scale: number,
	{ duration = 0, delay = 0, signal }: ScaleTransition = {},
): Promise<void> {
	const normalized = interfacePreferences({ scale }).scale;
	if (!isTauri() || signal?.aborted) return settled;
	if (delay > 0)
		return wait(delay, signal).then(() =>
			applyInterfaceScale(scale, { duration, signal }),
		);
	if (lastRequest?.scale === normalized && !lastRequest.signal?.aborted)
		return lastRequest.promise;
	if (!lastRequest && appliedScale === normalized) return settled;
	const task = zoomQueue.then(async () => {
		if (signal?.aborted || appliedScale === normalized) return;
		const webview = getCurrentWebview();
		async function setScale(value: number) {
			await webview.setZoom(value / 100);
			appliedScale = value;
		}
		const from = appliedScale ?? 100;
		if (duration <= 0 || from === normalized) {
			await setScale(normalized);
			return;
		}
		const started = performance.now();
		while (!signal?.aborted) {
			await wait(16, signal);
			if (signal?.aborted) break;
			const t = Math.min(1, (performance.now() - started) / duration);
			const eased = t * t * (3 - 2 * t);
			await setScale(
				t === 1 ? normalized : from + (normalized - from) * eased,
			);
			if (t === 1) break;
		}
	});
	// A rejected native request must not block subsequent changes or a reset.
	zoomQueue = task.catch(() => {});
	const request = { scale: normalized, promise: task, signal };
	lastRequest = request;
	const finish = () => {
		if (lastRequest === request) lastRequest = undefined;
	};
	void task.then(finish, finish);
	return task;
}
