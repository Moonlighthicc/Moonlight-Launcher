import { SvelteMap } from "svelte/reactivity";
import { untrack } from "svelte";
import { getAvatarSvg } from "$lib/api/cubicApi";
import offlineAvatarSvg from "$lib/assets/offline-avatar.svg?raw";
import type { MinecraftUser } from "$lib/types/types";

interface CacheEntry {
	// Render SVGs as <img> data URLs, never as inline HTML.
	svg: string;
	fetchedAt: number;
}

const cache = new SvelteMap<string, CacheEntry>();
const MAX = 50;
const TTL_MS = 5 * 60 * 1000; // 5 minutos
const STORAGE_KEY = "moonlight.accountAvatars.v1";
const MAX_SVG_LENGTH = 128 * 1024;
const MAX_STORAGE_LENGTH = 1024 * 1024;
const savedAvatars = new SvelteMap<string, string>(readSavedAvatars());

interface PendingAvatar {
	controller: AbortController;
	promise: Promise<string>;
	forced: boolean;
}

// Request handles are not UI state. Share native requests between consumers.
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const pendingFetches = new Map<string, PendingAvatar>();

/**
 * Versiones reactivas de avatar por UUID.
 * Se incrementan tras cambiar la skin para forzar recarga de heads.
 */
export const avatarVersions = new SvelteMap<string, number>();

function cacheKey(
	uuid: string,
	version: number,
	userType: string,
	serverUrl?: string | null,
): string {
	const server = userType === "Yggdrasil" ? serverUrl : null;
	return server ? `${uuid}@${version}:${server}` : `${uuid}@${version}`;
}

function savedKey(uuid: string, userType: string, serverUrl?: string | null): string {
	return JSON.stringify([uuid, userType, userType === "Yggdrasil" ? serverUrl ?? "" : ""]);
}

function readSavedAvatars(): [string, string][] {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw || raw.length > MAX_STORAGE_LENGTH) return [];
		const entries: unknown = JSON.parse(raw);
		if (!Array.isArray(entries)) return [];
		return entries.slice(-MAX).flatMap((entry): [string, string][] => {
			if (!Array.isArray(entry) || typeof entry[0] !== "string") return [];
			const svg = normalizeAvatarSvg(entry[1]);
			return svg ? [[entry[0], svg]] : [];
		});
	} catch {
		return [];
	}
}

function saveAvatar(key: string, svg: string): void {
	if (savedAvatars.get(key) === svg) return;
	savedAvatars.delete(key);
	savedAvatars.set(key, svg);
	let serialized = JSON.stringify([...savedAvatars]);
	while (savedAvatars.size > MAX || serialized.length > MAX_STORAGE_LENGTH) {
		const first = savedAvatars.keys().next();
		if (first.done) break;
		savedAvatars.delete(first.value);
		serialized = JSON.stringify([...savedAvatars]);
	}
	try {
		localStorage.setItem(STORAGE_KEY, serialized);
	} catch {
		// Keep the in-memory cache when local storage is unavailable.
	}
}

function getCacheEntry(key: string): CacheEntry | undefined {
	const entry = cache.get(key);
	if (entry && Date.now() - entry.fetchedAt >= TTL_MS) {
		cache.delete(key);
		return undefined;
	}
	return entry;
}

export function getAvatar(url: string): string | undefined {
	return getCacheEntry(url)?.svg;
}

export function setAvatar(url: string, svg: string): void {
	// Refreshing an existing entry must not evict an unrelated avatar.
	cache.delete(url);
	if (cache.size >= MAX) {
		const first = cache.keys().next();
		if (!first.done) cache.delete(first.value);
	}
	cache.set(url, { svg, fetchedAt: Date.now() });
}

export function invalidateAvatarCache(url: string) {
	cache.delete(url);
}

export function invalidateAvatarFor(uuid: string) {
	const prefix = `${uuid}@`;
	for (const [key, pending] of pendingFetches) {
		if (key.startsWith(prefix)) {
			pending.controller.abort();
			pendingFetches.delete(key);
		}
	}
	for (const key of cache.keys()) {
		if (key.startsWith(prefix)) cache.delete(key);
	}
}

export function getAvatarVersion(uuid: string): number {
	return avatarVersions.get(uuid) ?? 0;
}

export function bumpAvatarVersion(uuid: string): number {
	const next = (avatarVersions.get(uuid) ?? 0) + 1;
	avatarVersions.set(uuid, next);
	// Invalidate requests, but keep the last good image until the new skin loads.
	invalidateAvatarFor(uuid);
	return next;
}

export const DEFAULT_AVATAR_SVG = "";

type AvatarAccount = Pick<MinecraftUser, "uuid" | "user_type" | "yggdrasil_server_url">;
const offlineAvatar = normalizeAvatarSvg(offlineAvatarSvg);

/** Read the shared image reactively without dropping a stale image during revalidation. */
export function accountAvatar(account: AvatarAccount | null | undefined): string {
	if (!account) return DEFAULT_AVATAR_SVG;
	if (account.user_type === "Cracked") return offlineAvatar;
	const { uuid, user_type, yggdrasil_server_url } = account;
	const key = cacheKey(uuid, getAvatarVersion(uuid), user_type, yggdrasil_server_url);
	return (
		cache.get(key)?.svg ??
		savedAvatars.get(savedKey(uuid, user_type, yggdrasil_server_url)) ??
		DEFAULT_AVATAR_SVG
	);
}

/** Call from a menu effect, passing an empty list while closed. */
export function createAvatarRefreshSession() {
	const refreshed = new Map<string, number>();
	return (accounts: readonly AvatarAccount[]) => {
		const visible = new Set<string>();
		for (const account of accounts) {
			const { uuid, user_type, yggdrasil_server_url } = account;
			if (!uuid || user_type === "Cracked") continue;
			const key = savedKey(uuid, user_type, yggdrasil_server_url);
			const version = getAvatarVersion(uuid);
			visible.add(key);
			if (refreshed.get(key) === version) continue;
			refreshed.set(key, version);
			untrack(() => void refreshAvatarSvg(account));
		}
		for (const key of refreshed.keys()) {
			if (!visible.has(key)) refreshed.delete(key);
		}
	};
}

/**
 * Аватар рисуется как `<img src="data:image/svg+xml,…">`. Корень SVG с
 * `width="100%" height="100%"` в таком режиме иногда разрешается неверно
 * (картинка превращается в пару растянутых пикселей), а оборванный ответ
 * сервера вообще не SVG. Поэтому приводим корень к явным размерам из viewBox
 * и отбрасываем невалидные ответы (UI тогда покажет аватар по умолчанию).
 */
export function normalizeAvatarSvg(raw: string): string {
	if (typeof raw !== "string" || raw.length > MAX_SVG_LENGTH) return DEFAULT_AVATAR_SVG;
	const svg = raw.trim();
	const open = /^(?:(?:<\?xml\s+[^?]*\?>|<!--[\s\S]*?-->)\s*)*(<svg(?:\s[^>]*)?>)/i.exec(svg);
	if (!open || !/<\/svg\s*>\s*$/i.test(svg)) return DEFAULT_AVATAR_SVG;

	let tag = open[1];
	const attr = (name: string) =>
		new RegExp(`\\s${name}\\s*=\\s*["']([^"']*)["']`, "i").exec(tag)?.[1];
	const number = (value: string | undefined) => {
		const n = value && /^\s*[\d.]+(px)?\s*$/.test(value) ? parseFloat(value) : NaN;
		return Number.isFinite(n) && n > 0 ? n : NaN;
	};

	let width = NaN;
	let height = NaN;
	const rawViewBox = attr("viewBox");
	const viewBox = rawViewBox
		?.trim()
		.split(/[\s,]+/)
		.map(Number);
	if (viewBox && viewBox.length === 4 && viewBox.every(Number.isFinite)) {
		width = viewBox[2];
		height = viewBox[3];
	} else {
		if (rawViewBox !== undefined) return DEFAULT_AVATAR_SVG;
		width = number(attr("width"));
		height = number(attr("height"));
		if (!Number.isFinite(width) || !Number.isFinite(height)) {
			return DEFAULT_AVATAR_SVG;
		}
		tag = tag.replace(/<svg\b/i, `<svg viewBox="0 0 ${width} ${height}"`);
	}
	if (!(width > 0 && width === height)) return DEFAULT_AVATAR_SVG;
	if (attr("xmlns") === undefined) {
		tag = tag.replace(/<svg\b/i, '<svg xmlns="http://www.w3.org/2000/svg"');
	}

	tag = tag
		.replace(/\s(width|height|preserveAspectRatio)\s*=\s*["'][^"']*["']/gi, "")
		.replace(
			/<svg\b/i,
			`<svg width="${width}" height="${height}" preserveAspectRatio="xMidYMid meet"`,
		);
	return tag + svg.slice(open[0].length);
}

async function validateAvatarSvg(raw: string): Promise<string> {
	const svg = normalizeAvatarSvg(raw);
	if (!svg || typeof Image === "undefined") return svg;
	try {
		const image = new Image();
		image.src = `data:image/svg+xml,${encodeURIComponent(svg)}`;
		await image.decode();
		return image.naturalWidth > 0 && image.naturalWidth === image.naturalHeight
			? svg
			: DEFAULT_AVATAR_SVG;
	} catch {
		return DEFAULT_AVATAR_SVG;
	}
}

export async function fetchAvatarSvg(
	uuid: string,
	userType: string,
	serverUrl?: string | null,
): Promise<string> {
	return requestAvatarSvg(uuid, userType, serverUrl, false);
}

export async function refreshAvatarSvg(account: AvatarAccount): Promise<boolean> {
	const { uuid, user_type, yggdrasil_server_url } = account;
	return Boolean(await requestAvatarSvg(uuid, user_type, yggdrasil_server_url, true));
}

async function requestAvatarSvg(
	uuid: string,
	userType: string,
	serverUrl: string | null | undefined,
	force: boolean,
): Promise<string> {
	if (userType === "Cracked") {
		return offlineAvatar;
	}

	if (!uuid) {
		return DEFAULT_AVATAR_SVG;
	}

	const version = getAvatarVersion(uuid);
	const key = cacheKey(uuid, version, userType, serverUrl);
	const pending = pendingFetches.get(key);
	if (pending && (!force || pending.forced)) return pending.promise;
	const cached = cache.get(key);
	if (!force && cached && Date.now() - cached.fetchedAt < TTL_MS) return cached.svg;
	// A forced request supersedes an older regular request, but leaves the last
	// rendered avatar in place until a valid replacement arrives.
	pending?.controller.abort();

	const controller = new AbortController();
	const promise = loadAvatar(uuid, controller.signal, force || version > 0)
		.then((svg) => {
			// Native requests cannot be aborted; reject results invalidated in flight.
			if (
				controller.signal.aborted ||
				getAvatarVersion(uuid) !== version
			) {
				return DEFAULT_AVATAR_SVG;
			}
			if (svg) {
				setAvatar(key, svg);
				saveAvatar(savedKey(uuid, userType, serverUrl), svg);
			}
			return svg;
		})
		.finally(() => {
			// An invalidation may already have started a replacement request.
			if (pendingFetches.get(key)?.controller === controller) {
				pendingFetches.delete(key);
			}
		});
	pendingFetches.set(key, { controller, promise, forced: force });
	return promise;
}

async function loadAvatar(
	uuid: string,
	signal: AbortSignal,
	fresh: boolean,
): Promise<string> {
	try {
		return await validateAvatarSvg(await getAvatarSvg(uuid, fresh));
	} catch (error) {
		if (!signal.aborted) console.error("Error loading avatar:", error);
		return DEFAULT_AVATAR_SVG;
	}
}
