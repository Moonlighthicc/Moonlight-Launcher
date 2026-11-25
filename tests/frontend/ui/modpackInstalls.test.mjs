import { expect, test } from "bun:test";
import { dismissModpackInstall, modpackInstalls, retryModpackInstall, runModpackInstall } from "../../../src/lib/state/modpackInstalls.svelte.ts";

test("an install stays visible to a new view and cannot be started twice", async () => {
	const key = "modrinth:background-pack";
	const task = { archive: "mrpack-dl-test", version: "test", name: "Background Pack" };
	const finish = Promise.withResolvers();
	const install = runModpackInstall(key, task, () => finish.promise);
	expect(modpackInstalls.get(key)).toEqual({ ...task, running: true, error: null });
	dismissModpackInstall(key);
	expect(modpackInstalls.get(key)?.running).toBe(true);
	expect(await runModpackInstall(key, task, () => { throw Error("Duplicate install"); })).toBe(false);
	finish.resolve();
	expect(await install).toBe(true);
	expect(modpackInstalls.has(key)).toBe(false);
});

test("retry reruns the original install and clears the error while it runs", async () => {
	const key = "curseforge:retry-pack";
	const task = { archive: "cfpack-dl-test", version: "test", name: "Retry Pack" };
	const finish = Promise.withResolvers();
	let attempts = 0;
	expect(await runModpackInstall(key, task, async () => {
		if (++attempts === 1) throw "Connection lost";
		await finish.promise;
	})).toBe(false);
	expect(modpackInstalls.get(key)).toEqual({ ...task, running: false, error: "Connection lost" });
	expect(await runModpackInstall(key, task, async () => {})).toBe(false);
	expect(modpackInstalls.get(key)?.error).toBe("Connection lost");
	const retry = retryModpackInstall(key);
	expect(modpackInstalls.get(key)).toEqual({ ...task, running: true, error: null });
	expect(await retryModpackInstall(key)).toBe(false);
	finish.resolve();
	expect(await retry).toBe(true);
	expect(attempts).toBe(2);
	expect(modpackInstalls.has(key)).toBe(false);
});

test("a second failure replaces the error, and Cancel dismisses the failed job", async () => {
	const key = "modrinth:dismiss-pack";
	const task = { archive: "mrpack-dl-test", version: "test", name: "Dismiss Pack" };
	let attempts = 0;
	await runModpackInstall(key, task, async () => { throw `Failure ${++attempts}`; });
	expect(await retryModpackInstall(key)).toBe(false);
	expect(modpackInstalls.get(key)?.error).toBe("Failure 2");
	dismissModpackInstall(key);
	expect(await retryModpackInstall(key)).toBe(false);
	expect(modpackInstalls.has(key)).toBe(false);
	expect(await runModpackInstall(key, task, async () => {})).toBe(true);
});

test("explicit cancellation clears the active task without retaining a failure", async () => {
	const key = "modrinth:cancelled-pack";
	const task = { archive: "mrpack-dl-cancel", version: "cancel", name: "Cancelled Pack" };
	const cancel = new Error("Download cancelled");
	expect(await runModpackInstall(key, task, async () => { throw cancel; }, (error) => error === cancel)).toBe(false);
	expect(modpackInstalls.has(key)).toBe(false);
});