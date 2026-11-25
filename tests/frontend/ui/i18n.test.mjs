import { expect, test } from "bun:test";
import { compileModule } from "svelte/compiler";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";

async function fixture() {
	const root = resolve(import.meta.dir, "../../..");
	const source = join(root, "src/lib/i18n/index.ts");
	const bundle = await Bun.build({
		entrypoints: ["i18n-entry"],
		target: "bun",
		conditions: ["browser"],
		plugins: [
			{
				name: "i18n-fixture",
				setup(build) {
					build.onResolve(
						{ filter: /^i18n-(entry|stub)$/ },
						({ path }) => ({ path, namespace: "fixture" }),
					);
					build.onResolve(
						{
							filter: /^(@tauri-apps\/api\/core|\$lib\/state\/state.svelte)$/,
						},
						() => ({ path: "i18n-stub", namespace: "fixture" }),
					);
					build.onLoad(
						{ filter: /.*/, namespace: "fixture" },
						({ path }) => ({
							loader: "js",
							resolveDir: root,
							contents:
								path === "i18n-entry"
									? `export * from ${JSON.stringify(source)};export * from 'i18n-stub';
import {t} from ${JSON.stringify(source)};
import {effect_root,render_effect,derived,get} from 'svelte/internal/client';
export {flushSync} from 'svelte';
export const observeTranslation=onValue=>effect_root(()=>{const text=derived(()=>t('common.cancel'));render_effect(()=>onValue(get(text)))});`
									: compileModule(
											`
export const launcherStore=$state({settings:{language:'en'}});
export const calls=[];

export const invoke=(command,args)=>{calls.push({command,args});return hooks.invoke(command,args)};
`,
											{
												filename: "stub.svelte.js",
												generate: "client",
											},
										).js.code,
						}),
					);
					build.onLoad(
						{ filter: /loader\.svelte\.ts$/ },
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
	const dir = await mkdtemp(join(tmpdir(), "i18n-fixture-"));
	try {
		const entry = join(dir, "entry.mjs");
		await writeFile(entry, await bundle.outputs[0].text());
		return await import(pathToFileURL(entry).href);
	} finally {
		await rm(dir, { recursive: true, force: true });
	}
}

test("bundled locales and language selection do not request native dictionaries", async () => {
	const f = await fixture();
	expect(f.calls).toEqual([]);
	await Promise.all([f.loadAvailableLocales(), f.loadAvailableLocales(), f.downloadLocale("ru")]);
	expect(f.locales.map(({ code }) => code)).toEqual(["en", "ru"]);
	for (const { code, id } of f.locales) {
		expect(f.isBundled(code)).toBe(true);
		expect(f.isFetched(code)).toBe(true);
		expect(f.localeBadge({ id })).toBe(id);
	}
	expect(f.calls).toEqual([]);
});

test("existing translations react to changes of the bundled language", async () => {
	const f = await fixture();
	const values = [];
	const stop = f.observeTranslation((value) => values.push(value));
	try {
		f.launcherStore.settings.language = "ru";
		f.flushSync();
		f.launcherStore.settings.language = "en";
		f.flushSync();
		expect(values).toEqual(["Cancel", "Отмена", "Cancel"]);
	} finally {
		stop();
	}
	expect(f.calls).toEqual([]);
});

test("unsupported locale codes always fall back to English without retries", async () => {
	const f = await fixture();
	for (const code of ["fr", "es", "constructor", "__proto__"]) {
		f.launcherStore.settings.language = code;
		await f.downloadLocale(code);
		expect(f.isBundled(code)).toBe(false);
		expect(f.isFetched(code)).toBe(false);
		expect(f.t("common.cancel")).toBe("Cancel");
	}
	expect(f.t("constructor")).toBe("constructor");
	expect(f.calls).toEqual([]);
});

