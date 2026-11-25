import { expect, test } from "bun:test";
import { readFile, mkdtemp, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

async function fixture(saved = null) {
	const root = resolve(import.meta.dir, "../../..");
	const source = join(root, "src/lib/state/avatarCache.svelte.ts");
	const stub = `
export const hooks = {now:0, saved:${JSON.stringify(saved)}, native:async()=>'<svg/>'};
export const localStorage = {
	getItem: () => hooks.saved,
	setItem: (_, value) => {hooks.saved = value;},
};
export const calls = {native:0, fallback:0};
export const Date = {now:()=>hooks.now};
export const getAvatarSvg = (...args) => {calls.native++; return hooks.native(...args)};
export const fetch = () => {calls.fallback++; throw new Error('Unexpected WebView avatar fetch')};
`;
	const result = await Bun.build({
		entrypoints: ["avatar-entry"],
		target: "bun",
		conditions: ["browser"],
		plugins: [
			{
				name: "avatar-cache-fixture",
				setup(build) {
					build.onResolve(
						{ filter: /^avatar-(entry|stub)$/ },
						({ path }) => ({ path, namespace: "fixture" }),
					);
					build.onResolve(
						{ filter: /^\$lib\/api\/cubicApi$/ },
						() => ({ path: "avatar-stub", namespace: "fixture" }),
					);
					build.onResolve(
						{ filter: /offline-avatar\.svg\?raw$/ },
						() => ({ path: "offline", namespace: "fixture" }),
					);
					build.onLoad(
						{ filter: /.*/, namespace: "fixture" },
						({ path }) => ({
							loader: "js",
							resolveDir: root,
							contents:
								path === "avatar-entry"
									? `export * from ${JSON.stringify(source)}; export {hooks,calls} from 'avatar-stub';`
									: path === "avatar-stub"
										? stub
								: `export default '<svg width="64" height="64">offline</svg>';`,
						}),
					);
					build.onLoad(
						{ filter: /avatarCache\.svelte\.ts$/ },
						async ({ path }) => ({
							loader: "ts",
							contents:
								"import {fetch, Date, localStorage} from 'avatar-stub';\n" +
								(await readFile(path, "utf8")),
						}),
					);
				},
			},
		],
	});
	expect(result.success, result.logs.join("\n")).toBe(true);
	const dir = await mkdtemp(join(tmpdir(), "avatar-cache-"));
	const path = join(dir, "fixture.mjs");
	await writeFile(path, await result.outputs[0].text());
	return {
		...(await import(path)),
		cleanup: () => rm(dir, { recursive: true, force: true }),
	};
}

test("simultaneous avatar consumers share one native request and cache the result", async () => {
	const f = await fixture();
	try {
		let release;
		f.hooks.native = () => new Promise((resolve) => (release = resolve));
		const svg = '<svg width="64" height="64">shared</svg>';
		const requests = Array.from({ length: 20 }, () =>
			f.fetchAvatarSvg("user", "Microsoft"),
		);
		expect(f.calls.native).toBe(1);
		release(svg);
		expect(await Promise.all(requests)).toEqual(
			Array(20).fill(f.normalizeAvatarSvg(svg)),
		);
		expect(await f.fetchAvatarSvg("user", "Microsoft")).toBe(
			f.normalizeAvatarSvg(svg),
		);
		expect(f.calls.native).toBe(1);
	} finally {
		await f.cleanup();
	}
});

test("avatar expiry only checks the requested entry and refreshing a full cache preserves other entries", async () => {
	const f = await fixture();
	try {
		for (let i = 0; i < 50; i++) f.setAvatar("url-" + i, "svg-" + i);
		f.setAvatar("url-25", "updated");
		expect(f.getAvatar("url-0")).toBe("svg-0");
		f.setAvatar("url-50", "new");
		expect(f.getAvatar("url-0")).toBeUndefined();
		expect(f.getAvatar("url-25")).toBe("updated");
		f.hooks.native = async () => '<svg viewBox="0 0 8 8"><rect width="8" height="8"/></svg>';
		await f.fetchAvatarSvg("user", "Microsoft");
		await f.fetchAvatarSvg("user", "Microsoft");
		expect(f.calls.native).toBe(1);
		f.hooks.now = 5 * 60 * 1000;
		expect(f.getAvatar("url-25")).toBeUndefined();
		await f.fetchAvatarSvg("user", "Microsoft");
		expect(f.calls.native).toBe(2);
	} finally {
		await f.cleanup();
	}
});

test("skin invalidation rejects old native results without deleting a replacement request", async () => {
	const f = await fixture();
	try {
		const releases = [];
		f.hooks.native = () => new Promise((resolve) => releases.push(resolve));
		const old = f.fetchAvatarSvg("user", "Microsoft");
		f.invalidateAvatarFor("user");
		const current = f.fetchAvatarSvg("user", "Microsoft");
		releases[0]("<svg>old</svg>");
		expect(await old).toBe("");
		const shared = f.fetchAvatarSvg("user", "Microsoft");
		expect(f.calls.native).toBe(2);
		const currentSvg = '<svg width="64" height="64">current</svg>';
		releases[1](currentSvg);
		expect(await current).toBe(f.normalizeAvatarSvg(currentSvg));
		expect(await shared).toBe(f.normalizeAvatarSvg(currentSvg));
		f.bumpAvatarVersion("user");
		const next = f.fetchAvatarSvg("user", "Microsoft");
		expect(f.calls.native).toBe(3);
		const nextSvg = '<svg width="64" height="64">new skin</svg>';
		releases[2](nextSvg);
		expect(await next).toBe(f.normalizeAvatarSvg(nextSvg));
	} finally {
		await f.cleanup();
	}
});

test("failed requests can retry and offline users need no native or network requests", async () => {
	const f = await fixture();
	try {
		f.hooks.native = async () => {
			throw new Error("expected native failure");
		};
		expect(await f.fetchAvatarSvg("user", "Microsoft")).toBe("");
		const recoveredSvg = '<svg width="64" height="64">recovered</svg>';
		f.hooks.native = async () => recoveredSvg;
		expect(await f.fetchAvatarSvg("user", "Microsoft")).toBe(
			f.normalizeAvatarSvg(recoveredSvg),
		);
		expect(await f.fetchAvatarSvg("offline", "Cracked")).toBe(
			f.normalizeAvatarSvg('<svg width="64" height="64">offline</svg>'),
		);
		expect(await f.fetchAvatarSvg("", "Microsoft")).toBe("");
		expect(f.calls).toEqual({ native: 2, fallback: 0 });
	} finally {
		await f.cleanup();
	}
});

test("forced refresh updates the shared image, deduplicates and wins over a stale request", async () => {
	const f = await fixture();
	try {
		const account = { uuid: "user", username: "Steve", user_type: "Microsoft" };
		const svg = (name) => `<svg width="64" height="64">${name}</svg>`;
		f.hooks.native = async () => svg("old");
		await f.fetchAvatarSvg(account.uuid, account.user_type);
		f.hooks.now = 5 * 60 * 1000;
		const releases = [];
		f.hooks.native = () => new Promise((resolve) => releases.push(resolve));
		const stale = f.fetchAvatarSvg(account.uuid, account.user_type);
		const refresh = f.refreshAvatarSvg(account);
		const duplicate = f.refreshAvatarSvg(account);
		expect(f.calls.native).toBe(3);
		expect(f.accountAvatar(account)).toBe(f.normalizeAvatarSvg(svg("old")));
		releases[1](svg("fresh"));
		expect(await Promise.all([refresh, duplicate])).toEqual([true, true]);
		releases[0](svg("stale"));
		expect(await stale).toBe("");
		expect(f.accountAvatar(account)).toBe(f.normalizeAvatarSvg(svg("fresh")));
	} finally {
		await f.cleanup();
	}
});

test("a failed Yggdrasil refresh keeps the previous avatar without a WebView request", async () => {
	const f = await fixture();
	try {
		const account = { uuid: "user", username: "Steve", user_type: "Yggdrasil" };
		f.setAvatar("user@0", "previous");
		f.hooks.native = async (uuid, force) => {
			expect(uuid).toBe("user");
			expect(force).toBe(true);
			throw new Error("expected native failure");
		};
		expect(await f.refreshAvatarSvg(account)).toBe(false);
		expect(f.calls).toEqual({ native: 1, fallback: 0 });
		expect(f.accountAvatar(account)).toBe("previous");
	} finally {
		await f.cleanup();
	}
});

test("a slow Microsoft avatar is not replaced by a third-party image", async () => {
	const f = await fixture();
	try {
		const account = { uuid: "user", username: "Steve", user_type: "Microsoft" };
		let release;
		f.hooks.native = () => new Promise((resolve) => (release = resolve));
		const initial = f.fetchAvatarSvg(account.uuid, account.user_type);
		await Bun.sleep(3600);
		const reopened = f.fetchAvatarSvg(account.uuid, account.user_type);
		expect(f.calls).toEqual({ native: 1, fallback: 0 });
		expect(f.accountAvatar(account)).toBe("");
		const skin = '<svg viewBox="0 0 8 8" width="100%" height="100%"><rect width="8" height="8"/></svg>';
		release(skin);
		const avatar = f.normalizeAvatarSvg(skin);
		expect(await Promise.all([initial, reopened])).toEqual([avatar, avatar]);
		expect(f.accountAvatar(account)).toBe(avatar);
	} finally {
		await f.cleanup();
	}
}, 8000);

test("an unusable Yggdrasil avatar falls back to the default UI without a WebView request", async () => {
	const f = await fixture();
	try {
		const account = { uuid: "user", user_type: "Yggdrasil", yggdrasil_server_url: "https://provider.example" };
		expect(await f.refreshAvatarSvg(account)).toBe(false);
		expect(f.accountAvatar(account)).toBe(f.DEFAULT_AVATAR_SVG);
		expect(f.calls).toEqual({ native: 1, fallback: 0 });
	} finally {
		await f.cleanup();
	}
});

test("saved avatars are shown after restart without requests, and reopening refreshes without replacing them on bad responses", async () => {
	const account = { uuid: "user", username: "Steve", user_type: "Microsoft" };
	const good = '<svg viewBox="0 0 8 8" width="100%" height="100%"><rect width="8" height="8"/></svg>';
	const first = await fixture();
	let saved;
	try {
		first.hooks.native = async () => good;
		expect(await first.refreshAvatarSvg(account)).toBe(true);
		saved = first.hooks.saved;
	} finally {
		await first.cleanup();
	}

	const f = await fixture(saved);
	try {
		f.hooks.now = 24 * 60 * 60 * 1000;
		const refreshMenu = f.createAvatarRefreshSession();
		refreshMenu([]);
		expect(f.accountAvatar(account)).toBe(f.normalizeAvatarSvg(good));
		expect(f.calls.native).toBe(0);

		f.hooks.native = async (_uuid, force) => {
			expect(force).toBe(true);
			return '<svg viewBox="0 0 8 1"><rect width="8" height="1"/></svg>';
		};
		refreshMenu([account]);
		expect(await f.refreshAvatarSvg(account)).toBe(false);
		refreshMenu([{ ...account }]);
		expect(f.calls.native).toBe(1);
		expect(f.accountAvatar(account)).toBe(f.normalizeAvatarSvg(good));
		expect(f.hooks.saved).toBe(saved);

		const fresh = '<svg viewBox="0 0 8 8"><rect width="8" height="8" fill="red"/></svg>';
		f.hooks.native = async () => fresh;
		refreshMenu([]);
		refreshMenu([account]);
		expect(await f.refreshAvatarSvg(account)).toBe(true);
		expect(f.calls.native).toBe(2);
		expect(f.accountAvatar(account)).toBe(f.normalizeAvatarSvg(fresh));
		expect(f.hooks.saved).not.toBe(saved);
	} finally {
		await f.cleanup();
	}
});
