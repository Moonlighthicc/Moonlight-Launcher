import { SvelteMap, SvelteSet } from "svelte/reactivity";
import {
	cancelDownload,
	getDownloadQueue,
	getDownloadSnapshot,
} from "$lib/api/cubicApi";
import { onAppEvent } from "$lib/api/launcherService";
import { t } from "$lib/i18n";

type SegKey =
	| "Library"
	| "Asset"
	| "Native"
	| "Client"
	| "Connecting"
	| "Verifying"
	| "Generic"
	| "Processing"
	| "Extracting"
	| "Finishing"
	| "Resolving"
	| "Jre";

interface SegProg {
	current: number;
	total: number;
}

export interface DownloadQueueItem {
	version: string;
	activeType: SegKey | null;
	/** Once work starts, per-file checks and batch resets must not restore the connecting label. */
	started?: boolean;
	segs: Record<SegKey, SegProg>;
	done: boolean;
	error: string | null;
	currentItem?: string | null;
	bytesCurrent?: number;
	bytesTotal?: number;
	speedBytesPerSecond?: number;
	speedSample?: { stage: SegKey; bytes: number; at: number };
	/** false para tareas que el backend no puede cancelar (modpacks, lotes de JRE). */
	cancellable?: boolean;
	/** true mientras se espera la respuesta del backend (aún sin entrar en la cola). */
	connecting?: boolean;
	/** true desde que el usuario pide cancelar hasta que el backend responde. */
	cancelling?: boolean;
	/** true mientras se eliminan los archivos de una instalación cancelada. */
	deleting?: boolean;
}

function emptySegs(): Record<SegKey, SegProg> {
	return {
		Library: { current: 0, total: 0 },
		Asset: { current: 0, total: 0 },
		Native: { current: 0, total: 0 },
		Client: { current: 0, total: 0 },
		Connecting: { current: 0, total: 0 },
		Verifying: { current: 0, total: 0 },
		Generic: { current: 0, total: 0 },
		Processing: { current: 0, total: 0 },
		Extracting: { current: 0, total: 0 },
		Finishing: { current: 0, total: 0 },
		Resolving: { current: 0, total: 0 },
		Jre: { current: 0, total: 0 },
	};
}

/**
 * Etapas que el backend emite justo antes de un lote nuevo o al terminar uno.
 * Al recibirlas la barra vuelve a "sin datos" en vez de conservar el lote anterior.
 */
const BATCH_RESET_STAGES = new Set(["pending", "resolving", "mc", "jre"]);
const SPEED_SAMPLE_MS = 2500;

function resetBatchFields(item: DownloadQueueItem): DownloadQueueItem {
	return {
		...item,
		segs: emptySegs(),
		bytesCurrent: 0,
		bytesTotal: 0,
		speedBytesPerSecond: 0,
		speedSample: undefined,
		currentItem: null,
	};
}

function normalizeStage(stage: string): SegKey {
	const map: Record<string, SegKey> = {
		library: "Library",
		asset: "Asset",
		native: "Native",
		client: "Client",
		connecting: "Connecting",
		verifying: "Verifying",
		generic: "Generic",
		processing: "Processing",
		finishing: "Finishing",
		jre: "Jre",
		extracting: "Extracting",
		resolving: "Resolving",
		mc: "Resolving",
	};
	return map[stage.toLowerCase()] ?? "Generic";
}

const PREPARATION_STAGES = new Set(["pending", "resolving", "mc", "jre", "connecting", "verifying", "deleting"]);

function hasWorkStarted(stage: string, fromProgress = false): boolean {
	// A stage event for Java can also mean we're still looking up the package.
	return (fromProgress && stage === "jre") || !PREPARATION_STAGES.has(stage);
}

const downloads = new SvelteMap<string, DownloadQueueItem>();
const removalTimers = new SvelteSet<ReturnType<typeof setTimeout>>();

/** Состояние раскрытия очереди переживает переключение обычной/компактной панели. */
export const downloadQueueUi = $state({
	expanded: false,
	manuallyCollapsed: false,
	lastActiveCount: 0,
});

/** Cada cuánto se consulta el estado real de la cola (además de los eventos). */
const POLL_INTERVAL_MS = 2500;
let pollTimer: ReturnType<typeof setInterval> | null = null;
let speedTimer: ReturnType<typeof setInterval> | null = null;
let pollInFlight = false;

let initialized = false;
let destroyed = false;
let unsubs: (() => void)[] = [];

export { downloads };
const failedVersions = new SvelteSet<string>();

export function hasDownloadFailed(version: string): boolean {
	return failedVersions.has(version) || Boolean(downloads.get(version)?.error);
}

export function getActiveDownloadCount(): number {
	return [...downloads.values()].filter((d) => !d.done && !d.error).length;
}

export function getDoneDownloadCount(): number {
	return [...downloads.values()].filter((d) => d.done && !d.error).length;
}

export function hasDownloads(): boolean {
	return downloads.size > 0;
}

export function getDownloadQueueItems(): DownloadQueueItem[] {
	return [...downloads.values()];
}

function sampleDownloadSpeeds(): void {
	const now = performance.now();
	for (const [version, item] of downloads) {
		const stage = item.activeType;
		if (item.done || item.error || item.deleting || !stage || !item.bytesTotal ||
			!hasWorkStarted(stage.toLowerCase(), true)) continue;
		const bytes = item.bytesCurrent ?? 0;
		const previous = item.speedSample;
		const sameBatch = previous && previous.stage === stage && bytes >= previous.bytes;
		const elapsed = sameBatch ? now - previous.at : 0;
		// Avoid showing a noisy first reading when a download starts just before the tick.
		if (sameBatch && elapsed < 1000) continue;
		downloads.set(version, {
			...item,
			speedBytesPerSecond: sameBatch ? (bytes - previous.bytes) * 1000 / elapsed : 0,
			speedSample: { stage, bytes, at: now },
		});
	}
}

function removeDownload(version: string, delay: number): void {
	const entry = downloads.get(version);
	const t = setTimeout(() => {
		removalTimers.delete(t);
		if (downloads.get(version) === entry) downloads.delete(version);
	}, delay);
	removalTimers.add(t);
}

/**
 * Sincroniza la UI con el estado real del backend. Los eventos pueden perderse
 * (red/VPN que se cae, webview dormida), así que cada 2-3 s se compara la cola
 * local con la del backend y se corrige cualquier diferencia: etapa, progreso,
 * finalización y errores.
 */
async function syncWithBackend(): Promise<void> {
	if (pollInFlight || destroyed) return;
	// Nada que vigilar: no molestar al backend.
	const hasPending = [...downloads.values()].some(
		(d) => !d.done && !d.error && !d.connecting,
	);
	if (!hasPending) return;
	pollInFlight = true;
	try {
		const snapshot = await getDownloadSnapshot();
		if (destroyed || !snapshot) return;
		const remote = new Map(snapshot.map((r) => [r.version, r]));

		for (const [version, item] of [...downloads.entries()]) {
			if (item.connecting || item.done || item.error || item.deleting) continue;
			const r = remote.get(version);

			if (!r) {
				// El backend ya no la conoce ni como activa ni como terminada:
				// cancelada (o resultado caducado). Se retira de la lista.
				downloads.delete(version);
				continue;
			}

			const status = r.status;
			if (status === "done") {
				failedVersions.delete(version);
				downloads.set(version, { ...item, done: true, activeType: null });
				removeDownload(version, 4000);
			} else if (typeof status === "object" && status && "error" in status) {
				failedVersions.add(version);
				downloads.set(version, {
					...item,
					done: true,
					activeType: null,
					error: status.error ?? null,
				});
				removeDownload(version, 8000);
			} else if (r.stage === "pending") {
				// Entre lotes: sin progreso conocido, no se conserva el lote anterior.
				downloads.set(version, {
					...resetBatchFields(item),
					activeType: null,
				});
			} else if (r.stage && r.stage !== "downloading") {
				const key = normalizeStage(r.stage);
				const useBytes = r.bytes_total > 0;
				const current = useBytes ? r.bytes_current : r.item_current;
				const total = useBytes ? r.bytes_total : r.item_total;
				const prev = item.segs[key];
				// No retroceder el progreso si un evento más reciente ya lo avanzó.
				if (item.activeType === key && prev.total === total && prev.current > current) {
					continue;
				}
				downloads.set(version, {
					...item,
					segs: { ...item.segs, [key]: { current, total } },
					currentItem: r.current_item ?? (item.activeType === key ? item.currentItem : null) ?? null,
					bytesCurrent: r.bytes_total > 0 ? r.bytes_current : 0,
					bytesTotal: r.bytes_total > 0 ? r.bytes_total : 0,
					activeType: key,
					started: item.started || hasWorkStarted(r.stage, true),
				});
			}
		}

		// Tareas activas que la UI aún no conoce (evento DEnqueue perdido).
		for (const r of snapshot) {
			if (downloads.has(r.version)) continue;
			if (r.status !== "pending" && r.status !== "downloading") continue;
			const key = r.stage && r.stage !== "pending" && r.stage !== "downloading"
				? normalizeStage(r.stage)
				: null;
			const segs = emptySegs();
			if (key) {
				const useBytes = r.bytes_total > 0;
				segs[key] = {
					current: useBytes ? r.bytes_current : r.item_current,
					total: useBytes ? r.bytes_total : r.item_total,
				};
			}
			downloads.set(r.version, {
				version: r.version,
				activeType: key,
				started: !!key && hasWorkStarted(r.stage, true),
				segs,
				currentItem: r.current_item ?? null,
				bytesCurrent: r.bytes_current,
				bytesTotal: r.bytes_total,
				done: false,
				error: null,
				cancellable: true,
			});
		}
	} catch {
		// Sin respuesta del backend: se reintenta en el siguiente ciclo.
	} finally {
		pollInFlight = false;
	}
}

export function initDownloadQueueState(): void {
	if (initialized) return;
	initialized = true;
	destroyed = false;

	pollTimer = setInterval(() => void syncWithBackend(), POLL_INTERVAL_MS);
	speedTimer = setInterval(sampleDownloadSpeeds, SPEED_SAMPLE_MS);
	// Al volver la red o la ventana, sincroniza de inmediato sin esperar al ciclo.
	if (typeof window !== "undefined") {
		window.addEventListener("online", onResync);
		window.addEventListener("focus", onResync);
		document.addEventListener("visibilitychange", onResync);
	}

	getDownloadQueue().then((queue) => {
		if (destroyed) return;
		for (const item of queue) {
			if (!downloads.has(item.version)) {
				downloads.set(item.version, {
					version: item.version,
					activeType: null,
					segs: emptySegs(),
					done: item.status === "done",
					error: null,
				});
			}
		}
	});

	unsubs.push(
		onAppEvent("DEnqueue", (payload) => {
			const { version, cancellable } = payload.data as {
				version: string;
				cancellable?: boolean;
			};
			const existing = downloads.get(version);
			if (existing && !existing.done && !existing.error) {
				downloads.set(version, {
					...existing,
					cancellable: cancellable !== false,
					connecting: false,
				});
				return;
			}
			downloads.set(version, {
				version,
				activeType: null,
				segs: emptySegs(),
				done: false,
				error: null,
				cancellable: cancellable !== false,
				connecting: false,
			});
		}),
	);

	unsubs.push(
		onAppEvent("DProgress", (payload) => {
			const {
				version,
				stage,
				item_current,
				item_total,
				bytes_current,
				bytes_total,
				current_item,
			} = payload.data as {
				version: string;
				stage: string;
				item_current: number;
				item_total: number;
				bytes_current: number;
				bytes_total: number;
				current_item?: string | null;
			};
			const existing = downloads.get(version) ?? {
				version,
				activeType: null,
				segs: emptySegs(),
				done: false,
				error: null,
			};
			const key = normalizeStage(stage);
			const currentItem = current_item?.trim() || null;
			const useBytes = bytes_total > 0;
			const current = useBytes ? bytes_current : item_current;
			const total = useBytes ? bytes_total : item_total;
			const canSample = useBytes && hasWorkStarted(stage, true);
			const previous = existing.speedSample;
			const sameBatch = canSample && previous?.stage === key &&
				bytes_current >= (existing.bytesCurrent ?? previous.bytes);
			const speedSample = sameBatch ? previous : canSample
				? { stage: key, bytes: bytes_current, at: performance.now() }
				: undefined;
			downloads.set(version, {
				...existing,
				currentItem,
				bytesCurrent: useBytes ? bytes_current : 0,
				bytesTotal: useBytes ? bytes_total : 0,
				speedBytesPerSecond: sameBatch ? existing.speedBytesPerSecond : 0,
				speedSample,
				segs: { ...existing.segs, [key]: { current, total } },
				activeType: key,
				started: existing.started || hasWorkStarted(stage, true),
				done: false,
				error: null,
			});
		}),
	);

	unsubs.push(
		onAppEvent("DStage", (payload) => {
			const { version, stage, info } = payload.data as {
				version: string;
				stage: string;
				info?: string | null;
			};
			const existing = downloads.get(version);
			if (existing) {
				const base = BATCH_RESET_STAGES.has(stage)
					? resetBatchFields(existing)
					: existing;
				const activeType = stage === "pending" ? null : normalizeStage(stage);
				downloads.set(version, {
					...base,
					activeType,
					started: existing.started || hasWorkStarted(stage),
					currentItem: info?.trim() || (activeType === base.activeType ? base.currentItem : null),
					deleting: stage === "deleting",
				});
			}
		}),
	);

	unsubs.push(
		onAppEvent("DFinish", (payload) => {
			const { version } = payload.data as { version: string };
			failedVersions.delete(version);
			const item = downloads.get(version);
			if (item) {
				downloads.set(version, {
					...item,
					done: true,
					activeType: null,
				});
			}
			removeDownload(version, 4000);
		}),
	);

	unsubs.push(
		onAppEvent("DCancel", (payload) => {
			const { version } = payload.data as { version: string };
			downloads.delete(version);
		}),
	);

	unsubs.push(
		onAppEvent("DError", (payload) => {
			const { version, message } = payload.data as {
				version: string;
				message?: string;
			};
			failedVersions.add(version);
			const item = downloads.get(version);
			if (item) {
				downloads.set(version, {
					...item,
					done: true,
					activeType: null,
					error: message ?? null,
				});
			} else {
				downloads.set(version, {
					version,
					activeType: null,
					segs: emptySegs(),
					done: true,
					error: message ?? null,
				});
			}
			removeDownload(version, 8000);
		}),
	);
}

function onResync(): void {
	void syncWithBackend();
}

export function destroyDownloadQueueState(): void {
	destroyed = true;
	if (pollTimer) clearInterval(pollTimer);
	pollTimer = null;
	if (speedTimer) clearInterval(speedTimer);
	speedTimer = null;
	if (typeof window !== "undefined") {
		window.removeEventListener("online", onResync);
		window.removeEventListener("focus", onResync);
		document.removeEventListener("visibilitychange", onResync);
	}
	for (const unsub of unsubs) unsub();
	unsubs = [];
	for (const t of removalTimers) clearTimeout(t);
	removalTimers.clear();
	downloads.clear();
	failedVersions.clear();
	initialized = false;
}

/** Pide cancelar una tarea; la UI la retira cuando el backend confirma la cancelación. */
export async function cancelDownloadTask(version: string): Promise<boolean> {
	const item = downloads.get(version);
	if (!item || item.cancelling || item.cancellable === false) return false;
	downloads.set(version, { ...item, cancelling: true });
	try {
		const ok = await cancelDownload(version);
		if (!ok) {
			const current = downloads.get(version);
			if (current) downloads.set(version, { ...current, cancelling: false });
		}
		return ok;
	} catch (error) {
		// A rejected IPC call must not leave the cancel button permanently disabled.
		const current = downloads.get(version);
		if (current) downloads.set(version, { ...current, cancelling: false });
		console.error(`Could not cancel download ${version}:`, error);
		return false;
	}
}

/** Clave de la cola para una instalación de cargador; coincide con el ID que usa el backend. */
export function loaderVersionId(
	loader: string,
	gameVersion: string,
	loaderVersion: string,
): string | null {
	switch (loader) {
		case "fabric":
			return `fabric-loader-${loaderVersion}-${gameVersion}`;
		case "quilt":
			return `quilt-loader-${loaderVersion}-${gameVersion}`;
		case "forge":
			return `${gameVersion}-forge-${loaderVersion}`;
		case "neoforge":
			return `${gameVersion}-neoforge-${loaderVersion}`;
		default:
			return null;
	}
}

/**
 * Muestra la tarea de inmediato ("Подключение...") mientras el backend la prepara.
 * Cuando llega el evento real, la fila continúa sin duplicarse. Si la operación
 * termina sin entrar en la cola (p. ej. ya instalada), la fila se retira.
 */
export async function trackConnecting<T>(
	version: string | null,
	run: () => Promise<T>,
): Promise<T> {
	if (version && !downloads.has(version)) {
		downloads.set(version, {
			version,
			activeType: null,
			segs: emptySegs(),
			done: false,
			error: null,
			cancellable: true,
			connecting: true,
		});
	}
	try {
		return await run();
	} finally {
		if (version) {
			const item = downloads.get(version);
			if (item?.connecting) downloads.delete(version);
		}
	}
}

/**
 * Porcentaje de la tarea. Se calcula sobre los bytes del lote actual.
 * No llega a 100 hasta que la tarea termina de verdad (DFinish): mientras
 * quede trabajo —aunque sea post-proceso o la siguiente fase— se queda en 99.
 */
export function getOverallPct(item: DownloadQueueItem): number {
	if (item.done) return 100;
	if (item.bytesTotal && item.bytesTotal > 0) {
		return clampPct(item.bytesCurrent ?? 0, item.bytesTotal);
	}
	const seg = item.activeType ? item.segs[item.activeType] : undefined;
	if (seg && seg.total > 0) return clampPct(seg.current, seg.total);
	return 0;
}

function clampPct(current: number, total: number): number {
	return Math.min(99, Math.max(0, Math.floor((current / total) * 100)));
}

export function getStatusLabel(item: DownloadQueueItem): string {
	return t(item.started ? "downloadProgress.statusGeneric" : "downloadProgress.statusConnecting");
}
