export interface TileItem {
	key: string;
	src: string | null;
	label: string | null;
	full?: string;
	totalCount?: number;
}

interface TileStorage {
	read(key: string): Promise<TileItem[] | undefined>;
	write(key: string, items: TileItem[]): Promise<void>;
}

const MAX_STORED_TILES = 72;
const MAX_TILE_CHARS = 4 * 1024 * 1024;
const STORE = "tiles";
let database: Promise<IDBDatabase> | undefined;

function openDatabase(): Promise<IDBDatabase> {
	return (database ??= new Promise<IDBDatabase>((resolve, reject) => {
		const request = indexedDB.open("moonlauncher-tile-content", 1);
		request.onupgradeneeded = () => {
			request.result
				.createObjectStore(STORE)
				.createIndex("usedAt", "usedAt");
		};
		request.onsuccess = () => resolve(request.result);
		request.onerror = () => reject(request.error);
	}).catch((error) => {
		database = undefined;
		throw error;
	}));
}

interface Snapshot {
	items: TileItem[];
	usedAt: number;
}

export const tileStorage: TileStorage = {
	async read(key) {
		const db = await openDatabase();
		return new Promise((resolve, reject) => {
			const request = db.transaction(STORE).objectStore(STORE).get(key);
			request.onsuccess = () => {
				const snapshot = request.result as Snapshot | undefined;
				resolve(
					Array.isArray(snapshot?.items) ? snapshot.items : undefined,
				);
			};
			request.onerror = () => reject(request.error);
		});
	},
	async write(key, items) {
		const db = await openDatabase();
		const size = items.reduce(
			(sum, { key, src, label, full }) =>
				sum +
				key.length +
				(src?.length ?? 0) +
				(label?.length ?? 0) +
				(full?.length ?? 0),
			0,
		);
		return new Promise((resolve, reject) => {
			const transaction = db.transaction(STORE, "readwrite");
			transaction.oncomplete = () => resolve();
			transaction.onabort = () => reject(transaction.error);
			const store = transaction.objectStore(STORE);
			if (size > MAX_TILE_CHARS) {
				store.delete(key);
				return;
			}
			store.put({ items, usedAt: Date.now() } satisfies Snapshot, key);
			const keys = store.index("usedAt").getAllKeys();
			keys.onsuccess = () => {
				for (const oldest of keys.result.slice(
					0,
					Math.max(0, keys.result.length - MAX_STORED_TILES),
				)) {
					store.delete(oldest);
				}
			};
		});
	},
};

export interface TileContent {
	items: TileItem[];
	/** A preliminary result is shown only if there is no complete snapshot. */
	preliminary?: boolean;
}

function sameItems(previous: TileItem[], next: TileItem[]): boolean {
	return (
		previous.length === next.length &&
		previous.every((item, index) => {
			const other = next[index];
			return (
				item.key === other.key &&
				item.src === other.src &&
				item.label === other.label &&
				item.full === other.full &&
				item.totalCount === other.totalCount
			);
		})
	);
}

/** Hydrate each tile independently; serialize only the expensive source reads. */
export function createTileContentLoader(
	cache: Map<string, TileItem[]>,
	storage: TileStorage,
) {
	const pending = new Map<string, { again: boolean; done: Promise<void> }>();
	let queue = Promise.resolve();

	return function request(
		key: string,
		fetch: () => Promise<TileContent>,
		refresh = false,
	): Promise<void> {
		const existing = pending.get(key);
		if (existing) {
			if (refresh) existing.again = true;
			return existing.done;
		}
		const job = { again: false, done: Promise.resolve() };
		const hydrate = async () => {
			if (cache.has(key)) return;
			try {
				const items = await storage.read(key);
				if (items) cache.set(key, items);
			} catch {
				// Persistent storage is optional; keep serving the in-memory cache.
			}
		};
		const load = async () => {
			try {
				const { items, preliminary } = await fetch();
				const previous = cache.get(key);
				if (preliminary && previous?.length) return;
				if (!previous || !sameItems(previous, items))
					cache.set(key, items);
				if (!preliminary) await storage.write(key, items);
			} catch {
				// A failed refresh must not replace a valid snapshot with an empty one.
			}
		};
		job.done = hydrate().then(async () => {
			do {
				job.again = false;
				const next = queue.then(load);
				queue = next;
				await next;
			} while (job.again);
			pending.delete(key);
		});
		pending.set(key, job);
		return job.done;
	};
}
