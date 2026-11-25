import { expect, test } from "bun:test";
import { createTileContentLoader } from "../../../src/lib/state/tileContentCache.ts";

const key = "instance:screenshots";
const saved = [{ key: "old", src: "data:image/jpeg;base64,b2xk", label: null }];
const updated = [
	{ key: "new", src: "data:image/jpeg;base64,bmV3", label: null },
];

function setup() {
	const disk = new Map();
	const storage = {
		read: async (key) => structuredClone(disk.get(key)),
		write: async (key, items) => {
			disk.set(key, structuredClone(items));
		},
	};
	return { disk, storage };
}

test("a new session restores tiles before a slow refresh and keeps instances separate", async () => {
	const { disk, storage } = setup();
	await createTileContentLoader(new Map(), storage)(key, async () => ({
		items: saved,
	}));
	const memory = new Map();
	const request = createTileContentLoader(memory, storage);
	const started = Promise.withResolvers();
	const response = Promise.withResolvers();
	const refresh = request(key, () => {
		started.resolve();
		return response.promise;
	});
	await started.promise;
	expect(memory.get(key)).toEqual(saved);
	expect(memory.has("another-instance:screenshots")).toBe(false);
	response.resolve({ items: updated });
	await refresh;
	expect(memory.get(key)).toEqual(updated);
	expect(disk.get(key)).toEqual(updated);
});

test("failures and partial results retain the snapshot; identical results do not replace it", async () => {
	const { disk, storage } = setup();
	const memory = new Map([[key, saved]]);
	disk.set(key, saved);
	const request = createTileContentLoader(memory, storage);
	await request(key, async () => {
		throw Error("unavailable");
	});
	await request(key, async () => ({ items: updated, preliminary: true }));
	expect(memory.get(key)).toBe(saved);
	expect(disk.get(key)).toBe(saved);
	await request(key, async () => ({ items: structuredClone(saved) }));
	expect(memory.get(key)).toBe(saved);
	await request(key, async () => ({ items: [] }));
	expect(memory.get(key)).toEqual([]);
	expect(disk.get(key)).toEqual([]);
});

test("concurrent requests share a load and enrichment schedules a follow-up", async () => {
	const { storage } = setup();
	const memory = new Map();
	const request = createTileContentLoader(memory, storage);
	const started = Promise.withResolvers();
	const response = Promise.withResolvers();
	let calls = 0;
	const fetch = async () => {
		if (++calls === 1) {
			started.resolve();
			return response.promise;
		}
		return { items: updated };
	};
	const first = request(key, fetch);
	expect(request(key, fetch)).toBe(first);
	await started.promise;
	expect(request(key, fetch, true)).toBe(first);
	response.resolve({ items: saved, preliminary: true });
	await first;
	expect(calls).toBe(2);
	expect(memory.get(key)).toEqual(updated);
});
