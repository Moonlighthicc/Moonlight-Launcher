import { launcherStore } from "$lib/state/state.svelte";
import en from "./en-US.json";
import ru from "./ru-RU.json";
import { i18nLoader, locales } from "./loader.svelte";

type NestedKeys<T, Prefix extends string = ""> = {
	[K in keyof T & string]: T[K] extends string
		? `${Prefix}${K}`
		: T[K] extends object
			? NestedKeys<T[K], `${Prefix}${K}.`>
			: never;
}[keyof T & string];

export type TranslationKey = Exclude<NestedKeys<typeof en>, "id">;

type DictValue = string | { [key: string]: DictValue };
type LocaleDict = Record<string, DictValue>;

function flatten(
	obj: Record<string, DictValue>,
	prefix = "",
): Record<string, string> {
	const result: Record<string, string> = Object.create(null);
	for (const key in obj) {
		if (key === "id" || (!prefix && key === "version")) continue;
		const val = obj[key];
		if (typeof val === "string") {
			result[prefix + key] = val;
		} else if (val && typeof val === "object") {
			Object.assign(result, flatten(val, prefix + key + "."));
		}
	}
	return result;
}

// Todos los idiomas van incluidos en la aplicación: no hay descargas ni red.
const enFlat = flatten(en as LocaleDict);
const bundled: Record<string, Record<string, string>> = {
	en: enFlat,
	ru: flatten(ru as LocaleDict),
};
for (const code of Object.keys(bundled)) i18nLoader.fetched.add(code);

export function isBundled(lang: string): boolean {
	return Object.hasOwn(bundled, lang);
}

export function isFetched(lang: string): boolean {
	return isBundled(lang);
}

export function localeBadge(entry: { id: string }): string {
	return entry.id;
}

/** Se mantiene por compatibilidad: los idiomas ya están incluidos, no hay nada que descargar. */
export async function downloadLocale(_lang: string): Promise<void> {}

/** Se mantiene por compatibilidad: la lista de idiomas es fija y está incluida. */
export function loadAvailableLocales(): Promise<void> {
	return Promise.resolve();
}

export function t(
	key: TranslationKey,
	params?: Record<string, string | number>,
): string;
export function t(
	key: string,
	params?: Record<string, string | number>,
): string;
export function t(
	key: string,
	params?: Record<string, string | number>,
): string {
	const lang = launcherStore.settings?.language || "en";
	const flat = isBundled(lang) ? bundled[lang] : enFlat;

	// Si falta la cadena en el idioma elegido, se usa el inglés.
	const result = flat[key] ?? enFlat[key];
	if (result === undefined) return key;
	if (!params) return result;
	return result.replace(/\{(\w+)\}/g, (_, name) =>
		String(params[name] ?? `{${name}}`),
	);
}

export { locales };
