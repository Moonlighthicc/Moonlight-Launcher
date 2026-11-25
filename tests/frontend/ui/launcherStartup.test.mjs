import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { compileModule } from "svelte/compiler";

// Exercise the real event service and installed-version state together, without
// process-wide mocks or a native Tauri runtime.
async function fixture() {
	const root = resolve(import.meta.dir, "../../..");
	const service = resolve(root, "src/lib/api/launcherService.ts");
	const stub = `
export const calls={downloads:0,queue:0,settings:0,themes:[],scans:0,unlistens:0};
export const launcherStore={settings:{theme:'test'},loadedInstances:[],pendingJreLaunch:null};
export const hooks={listener:null,scan:async()=>['1.20.1'],invoke:async()=>[]};
export const listen=async(_,handler)=>{hooks.listener=handler;return ()=>{calls.unlistens++;hooks.listener=null}};
export const emit=payload=>hooks.listener?.({payload});
export const getInstalledVersions=()=>{calls.scans++;return hooks.scan()};
export const getDownloadQueue=async()=>[];
export const getDownloadSnapshot=async()=>hooks.downloadSnapshot??[];
export const cancelDownload=async()=>{};
export const t=key=>key;
export const getSettings=async()=>{calls.settings++;return {theme:'test',language:'en'}};
export const applyTheme=(...args)=>calls.themes.push(args);
export const initDownloadState=()=>calls.downloads++;
export const initDownloadQueueState=()=>calls.queue++;
export const destroyDownloadState=()=>{};
export const destroyDownloadQueueState=()=>{};
export const showErrorParsed=()=>{};
export const clearPendingJreLaunch=()=>{launcherStore.pendingJreLaunch=null};
export const updateSettings=async()=>{};
export const killInstance=()=>{};
export const initDiscordPresence=()=>{};
export const shutdownDiscordPresence=()=>{};
export const launchInstance=()=>{};
export const markGameReady=()=>{};
export const invoke=(...args)=>hooks.invoke(...args);
`;
	const bundle = await Bun.build({
		entrypoints: ["startup-entry"],
		target: "bun",
		conditions: ["browser"],
		plugins: [
			{
				name: "launcher-startup",
				setup(build) {
					build.onResolve(
						{ filter: /^startup-(entry|stub)$/ },
						({ path }) => ({ path, namespace: "fixture" }),
					);
					build.onResolve({ filter: /.*/ }, ({ path, importer }) => {
						if (
							[
								"$lib/utils/saveQueue",
								"$lib/utils/versionUtils",
								"$lib/state/versionsState.svelte",
							].includes(path) ||
							(path === "$lib/state/downloadQueueState.svelte" && importer !== service)
						) {
							return {
								path: resolve(
									root,
									path.replace("$lib", "src/lib") + ".ts",
								),
							};
						}
						if (
							importer === service ||
							path === "$lib/api/cubicApi" || path === "$lib/i18n"
						)
							return {
								path: "startup-stub",
								namespace: "fixture",
							};
						if (path === "$lib/api/launcherService") return { path: service };
					});
					build.onLoad(
						{ filter: /.*/, namespace: "fixture" },
						({ path }) => ({
							loader: "js",
							resolveDir: root,
							contents:
								path === "startup-stub"
									? stub
									: `
export {initEventListeners,destroyEventListeners,getVersions,deleteInst,registerModsRefreshCallback} from ${JSON.stringify(service)};
export {loadInstalledVersions,versionsState} from '$lib/state/versionsState.svelte';
export {downloads,getStatusLabel,initDownloadQueueState as startQueue,destroyDownloadQueueState as stopQueue} from '$lib/state/downloadQueueState.svelte';
export * from 'startup-stub';`,
						}),
					);
					build.onLoad(
						{ filter: /(?:versionsState|downloadQueueState)\.svelte\.ts$/ },
						async ({ path }) => ({
							loader: "js",
							contents: compileModule(
								new Bun.Transpiler({
									loader: "ts",
								}).transformSync(await readFile(path, "utf8")),
								{ filename: path, generate: "client" },
							).js.code,
						}),
					);
				},
			},
		],
	});
	expect(bundle.success, bundle.logs.join("\n")).toBe(true);
	const directory = await mkdtemp(join(tmpdir(), "launcher-startup-"));
	try {
		const entry = join(directory, "fixture.mjs");
		await writeFile(entry, await bundle.outputs[0].text());
		return await import(pathToFileURL(entry).href);
	} finally {
		await rm(directory, { recursive: true, force: true });
	}
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 100));

test("download speed samples every 2.5 seconds independently of progress events", async () => {
	const f = await fixture();
	const version = "speed-test";
	const emit = (type, data = {}) => f.emit({ type, data: { version, ...data } });
	const progress = (bytes_current, current_item, stage = "library") => emit("DProgress", {
		stage, bytes_current, current_item, bytes_total: 1000, item_current: 0, item_total: 5,
	});
	const item = () => f.downloads.get(version);
	const realInterval = globalThis.setInterval;
	const realClearInterval = globalThis.clearInterval;
	const realNow = performance.now;
	const intervals = new Map();
	let now = 0;
	globalThis.setInterval = (callback, delay) => {
		const id = Symbol();
		intervals.set(id, { callback, delay });
		return id;
	};
	globalThis.clearInterval = (id) => intervals.delete(id);
	performance.now = () => now;
	try {
		f.startQueue();
		f.initEventListeners();
		// The snapshot poll registers first; tick only the speed sampler.
		const [, sampler] = intervals.values();
		expect(sampler.delay).toBe(2500);
		const tick = () => {
			now += sampler.delay;
			sampler.callback();
		};
		emit("DEnqueue");
		progress(0, "a.jar");
		progress(100, "a.jar");
		expect(item().bytesCurrent).toBe(100);
		expect(item().speedBytesPerSecond).toBe(0);
		progress(250, "b.jar");
		expect(item().speedBytesPerSecond).toBe(0);
		tick();
		expect(item().speedBytesPerSecond).toBe(100);
		progress(375, "c.jar");
		expect(item().currentItem).toBe("c.jar");
		expect(item().bytesCurrent).toBe(375);
		expect(item().speedBytesPerSecond).toBe(100);
		tick();
		expect(item().speedBytesPerSecond).toBe(50);
		tick();
		expect(item().speedBytesPerSecond).toBe(0);
		emit("DError", { message: "Network failure" });
		emit("DEnqueue");
		progress(0, "retry.jar");
		expect(item().speedBytesPerSecond).toBe(0);
	} finally {
		f.stopQueue();
		f.destroyEventListeners();
		performance.now = realNow;
		globalThis.setInterval = realInterval;
		globalThis.clearInterval = realClearInterval;
	}
	expect(intervals.size).toBe(0);
});

test("download status switches once per task and resets only on a new attempt", async () => {
	const f = await fixture();
	const version = "1.21-forge-test";
	const connecting = "downloadProgress.statusConnecting";
	const downloading = "downloadProgress.statusGeneric";
	const label = () => f.getStatusLabel(f.downloads.get(version));
	const event = (type, data = {}) => f.emit({ type, data: { version, ...data } });
	const stage = (stage) => event("DStage", { stage });
	const progress = (stage, current_item, bytes_current = 0) => event("DProgress", {
		stage, current_item, bytes_current, bytes_total: 100, item_current: 0, item_total: 2,
	});
	f.startQueue();
	f.initEventListeners();
	try {
		event("DEnqueue");
		for (const phase of ["resolving", "mc", "jre", "verifying", "connecting"]) {
			stage(phase);
			expect(label()).toBe(connecting);
		}
		progress("client", "client.jar", 10);
		expect(label()).toBe(downloading);
		for (const phase of ["pending", "resolving", "mc", "jre", "processing", "extracting"]) {
			stage(phase);
			expect(label()).toBe(downloading);
		}
		for (const phase of ["verifying", "connecting", "library"]) {
			progress(phase, "library.jar");
			expect(label()).toBe(downloading);
			expect(f.downloads.get(version).currentItem).toBe("library.jar");
		}
		event("DError", { message: "Network failure" });
		expect(f.downloads.get(version).error).toBe("Network failure");
		event("DEnqueue");
		expect(f.downloads.get(version).error).toBeNull();
		expect(label()).toBe(connecting);
		progress("jre", "Java");
		expect(label()).toBe(downloading);
		event("DFinish");
		expect(f.downloads.get(version).done).toBe(true);
	} finally {
		f.stopQueue();
		f.destroyEventListeners();
	}
});

test("InstanceEdited refreshes only subscribers of that instance and honors unsubscribe", async () => {
	const f = await fixture();
	const refreshed = [];
	const stopA = f.registerModsRefreshCallback("A", () => refreshed.push("A"));
	const stopB = f.registerModsRefreshCallback("B", () => refreshed.push("B"));
	f.initEventListeners();
	try {
		const event = (id) => ({
			type: "InstanceEdited",
			data: { id, dto: { uuid: id, name: id } },
		});
		f.emit(event("A"));
		expect(refreshed).toEqual(["A"]);
		stopA();
		f.emit(event("A"));
		f.emit(event("B"));
		expect(refreshed).toEqual(["A", "B"]);
	} finally {
		stopA();
		stopB();
		f.destroyEventListeners();
	}
});

test("a snapshot collected before deletion cannot resurrect the instance", async () => {
	const f = await fixture();
	const old = { uuid: "old", name: "Pack", pinned: false };
	f.launcherStore.loadedInstances.push(old);
	f.launcherStore.pendingJreLaunch = { instance: old, version: 21 };
	let release;
	f.hooks.invoke = () =>
		new Promise((resolve) => {
			release = resolve;
		});
	f.initEventListeners();
	try {
		const refresh = f.getVersions();
		f.emit({ type: "InstanceDeleted", data: { id: old.uuid } });
		expect(f.launcherStore.loadedInstances).toEqual([]);
		expect(f.launcherStore.pendingJreLaunch).toBeNull();
		f.hooks.invoke = async () => [];
		release([old]);
		await refresh;
		expect(f.launcherStore.loadedInstances).toEqual([]);
		f.emit({ type: "InstanceCreated", data: { id: old.uuid, dto: old } });
		expect(f.launcherStore.loadedInstances).toEqual([]);
		const replacement = { ...old, uuid: "new" };
		f.emit({
			type: "InstanceCreated",
			data: { id: replacement.uuid, dto: replacement },
		});
		f.emit({
			type: "InstanceCreated",
			data: { id: replacement.uuid, dto: replacement },
		});
		expect(f.launcherStore.loadedInstances).toEqual([replacement]);
	} finally {
		f.destroyEventListeners();
	}
});

test("an older instance refresh cannot overwrite a newer response", async () => {
	const f = await fixture();
	let release;
	f.hooks.invoke = () =>
		new Promise((resolve) => {
			release = resolve;
		});
	const older = f.getVersions();
	const current = { uuid: "new", name: "New", pinned: false };
	f.hooks.invoke = async () => [current];
	await f.getVersions();
	release([{ uuid: "old", name: "Old", pinned: false }]);
	await older;
	expect(f.launcherStore.loadedInstances).toEqual([current]);
});

test("delete IPC success invalidates snapshots even before its event arrives", async () => {
	const f = await fixture();
	const old = { uuid: "old", name: "Pack", pinned: false };
	f.launcherStore.loadedInstances.push(old);
	let release;
	f.hooks.invoke = () =>
		new Promise((resolve) => {
			release = resolve;
		});
	const refresh = f.getVersions();
	f.hooks.invoke = async () => [];
	expect(await f.deleteInst(old.uuid)).toBe(true);
	release([old]);
	await refresh;
	expect(f.launcherStore.loadedInstances).toEqual([]);
});

test("failed deletion preserves the instance and remains retryable", async () => {
	const f = await fixture();
	const old = { uuid: "old", name: "Pack", pinned: false };
	f.launcherStore.loadedInstances.push(old);
	f.hooks.invoke = async () => {
		throw Error("busy");
	};
	expect(await f.deleteInst(old.uuid)).toBe(false);
	f.hooks.invoke = async () => [old];
	await f.getVersions();
	expect(f.launcherStore.loadedInstances).toEqual([old]);
	f.hooks.invoke = async () => [];
	expect(await f.deleteInst(old.uuid)).toBe(true);
	expect(f.launcherStore.loadedInstances).toEqual([]);
});

test("destroying listeners invalidates outstanding instance snapshots", async () => {
	const f = await fixture();
	let release;
	f.hooks.invoke = () =>
		new Promise((resolve) => {
			release = resolve;
		});
	const refresh = f.getVersions();
	f.destroyEventListeners();
	release([{ uuid: "old", name: "Old", pinned: false }]);
	await refresh;
	expect(f.launcherStore.loadedInstances).toEqual([]);
});

test("log consoles keep live settings and themes without download or instance work", async () => {
	const f = await fixture();
	try {
		f.initEventListeners("logs");
		f.initEventListeners("logs");
		f.emit({ type: "InstanceCreated", data: { dto: { uuid: "unused" } } });
		f.emit({ type: "DFinish", data: { version: "1.20.1" } });
		f.emit({ type: "STChanged" });
		f.emit({ type: "ThemeChanged", data: { id: "other" } });
		f.emit({ type: "ThemeChanged", data: { id: "test" } });
		await settle();
		expect(f.calls.downloads).toBe(0);
		expect(f.calls.queue).toBe(0);
		expect(f.calls.scans).toBe(0);
		expect(f.launcherStore.loadedInstances).toEqual([]);
		expect(f.calls.settings).toBe(1);
		expect(f.launcherStore.settings.language).toBe("en");
		expect(f.calls.themes).toEqual([["test", { force: true }]]);
		f.emit({ type: "STChanged" });
	} finally {
		f.destroyEventListeners();
	}
	await settle();
	expect(f.calls.settings).toBe(1);
	expect(f.calls.unlistens).toBe(1);
});

test("completed downloads refresh installed versions without mounting a downloader", async () => {
	const f = await fixture();
	try {
		f.initEventListeners();
		f.initEventListeners();
		await f.loadInstalledVersions();
		expect(f.calls.downloads).toBe(1);
		expect(f.calls.queue).toBe(1);
		f.hooks.scan = async () => ["1.20.1", "1.21"];
		f.emit({ type: "DFinish", data: { version: "1.21" } });
		await f.loadInstalledVersions();
		expect([...f.versionsState.mcVersions.vanilla]).toEqual([
			"1.20.1",
			"1.21",
		]);
		expect(f.calls.scans).toBe(2);
	} finally {
		f.destroyEventListeners();
	}
});

test("downloads finishing during a scan coalesce into one trailing scan", async () => {
	const f = await fixture();
	let release;
	f.hooks.scan = () =>
		new Promise((resolve) => {
			release = resolve;
		});
	try {
		f.initEventListeners();
		const initial = f.loadInstalledVersions();
		for (let i = 0; i < 20; i++)
			f.emit({ type: "DFinish", data: { version: `version-${i}` } });
		const shared = f.loadInstalledVersions();
		expect(f.calls.scans).toBe(1);
		f.hooks.scan = async () => ["1.20.1", "1.21"];
		release(["1.20.1"]);
		await Promise.all([initial, shared]);
		expect(f.calls.scans).toBe(2);
		expect(f.versionsState.rawVersions).toEqual(["1.20.1", "1.21"]);
		expect(f.versionsState.loaded).toBe(true);
		expect(f.versionsState.loading).toBe(false);
	} finally {
		f.destroyEventListeners();
	}
});

test("a failed version scan remains retryable", async () => {
	const f = await fixture();
	f.hooks.scan = async () => {
		throw Error("disk unavailable");
	};
	await f.loadInstalledVersions();
	expect(f.versionsState.loaded).toBe(false);
	expect(f.versionsState.loading).toBe(false);
	expect(f.versionsState.error).toBe("disk unavailable");
	f.hooks.scan = async () => ["1.21"];
	await f.loadInstalledVersions();
	expect(f.versionsState.rawVersions).toEqual(["1.21"]);
	expect(f.versionsState.error).toBeNull();
});
