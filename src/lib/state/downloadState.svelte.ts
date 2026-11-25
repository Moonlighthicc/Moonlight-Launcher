import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { SvelteSet } from "svelte/reactivity";
import { getDownloadQueue, getDownloadSnapshot } from "$lib/api/cubicApi";
import type { AppEvent } from "$lib/types/types";

const activeVersionDownloads = new SvelteSet<string>();

let _initialized = false;
let _unlisten: Promise<UnlistenFn> | null = null;
let _pollTimer: ReturnType<typeof setInterval> | null = null;

/** Reconcilia el conjunto local con el backend por si se perdió algún evento. */
async function reconcile(): Promise<void> {
	if (activeVersionDownloads.size === 0) return;
	try {
		const snapshot = await getDownloadSnapshot();
		if (!snapshot) return;
		const active = new Set(
			snapshot
				.filter((r) => r.status === "pending" || r.status === "downloading")
				.map((r) => r.version),
		);
		for (const v of [...activeVersionDownloads]) {
			if (!active.has(v)) activeVersionDownloads.delete(v);
		}
	} catch {
		// se reintenta en el siguiente ciclo
	}
}

export async function initDownloadState(): Promise<void> {
	if (_initialized) return;
	_initialized = true;

	try {
		const queue = await getDownloadQueue();
		for (const item of queue) {
			if (item.status === "pending" || item.status === "downloading") {
				activeVersionDownloads.add(item.version);
			}
		}
	} catch (e) {
		console.error("Error initializing download state:", e);
	}

	_pollTimer = setInterval(() => void reconcile(), 2500);

	_unlisten = listen<AppEvent>("app-event", (event) => {
		const payload = event.payload;
		if (payload.type === "DEnqueue") {
			activeVersionDownloads.add(payload.data.version);
		} else if (
			payload.type === "DFinish" ||
			payload.type === "DError" ||
			payload.type === "DCancel"
		) {
			activeVersionDownloads.delete(payload.data.version);
		}
	});
}

export function isVersionDownloading(versionId: string): boolean {
	return activeVersionDownloads.has(versionId);
}

export function destroyDownloadState(): void {
	if (_pollTimer) clearInterval(_pollTimer);
	_pollTimer = null;
	if (_unlisten) {
		_unlisten.then((u) => u()).catch(() => {});
		_unlisten = null;
	}
	_initialized = false;
	activeVersionDownloads.clear();
}
