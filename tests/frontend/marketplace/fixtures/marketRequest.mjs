import assert from "node:assert/strict";
import { mock } from "bun:test";

let resolve, reject;
let delayedCancellation = false;
const calls = [];
mock.module("@tauri-apps/api/core", () => ({
	invoke: (command, args) => {
		calls.push({ command, args });
		if (command === "cancel_market_request") {
			if (!delayedCancellation) reject("MARKET_CANCELLED");
			return Promise.resolve();
		}
		return new Promise((yes, no) => {
			resolve = yes;
			reject = no;
		});
	},
}));
const { invokeCatalogSearch, invokeMarket } =
	await import("../../../../src/lib/api/marketRequest.ts");

for (const mode of ["success", "error", "cancel"]) {
	const controller = new AbortController();
	let listeners = 0;
	const add = controller.signal.addEventListener.bind(controller.signal);
	const remove = controller.signal.removeEventListener.bind(
		controller.signal,
	);
	controller.signal.addEventListener = (...args) => {
		listeners++;
		return add(...args);
	};
	controller.signal.removeEventListener = (...args) => {
		listeners--;
		return remove(...args);
	};
	const request = invokeMarket(
		"get_modrinth_project",
		{ projectId: "test" },
		controller.signal,
	);
	assert.equal(listeners, 1);
	if (mode === "success") {
		resolve({ id: "test" });
		assert.deepEqual(await request, { id: "test" });
	} else {
		const checked = assert.rejects(request);
		if (mode === "cancel") controller.abort();
		else reject(new Error("HTTP failed"));
		await checked;
	}
	assert.equal(listeners, 0);
	const count = calls.length;
	controller.abort();
	assert.equal(calls.length, count);
}

const stopped = new AbortController();
stopped.abort();
const count = calls.length;
await assert.rejects(invokeMarket("get_modrinth_project", {}, stopped.signal));
assert.equal(calls.length, count);

delayedCancellation = true;
const active = new AbortController();
const skipped = new AbortController();
const live = new AbortController();
const serialStart = calls.length;
const first = assert.rejects(
	invokeCatalogSearch("search_modrinth", { query: "old" }, active.signal),
);
await Bun.sleep(0);
const cancelledWhileQueued = assert.rejects(
	invokeCatalogSearch("search_curseforge", { query: "skip" }, skipped.signal),
);
const next = invokeCatalogSearch(
	"search_curseforge",
	{ query: "new" },
	live.signal,
);
active.abort();
skipped.abort();
await Bun.sleep(0);
assert.deepEqual(
	calls.slice(serialStart).map(({ command }) => command),
	["market_request", "cancel_market_request"],
);
reject("MARKET_CANCELLED");
await first;
await cancelledWhileQueued;
await Bun.sleep(0);
assert.equal(calls.length, serialStart + 3);
assert.equal(calls.at(-1).args.request.args.query, "new");
resolve("loaded");
assert.equal(await next, "loaded");
