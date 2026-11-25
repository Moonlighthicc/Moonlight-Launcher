/**
 * Contenido que se desliza dentro de los mosaicos / entradas del menú "Datos":
 * iconos de mods y packs, mundos, servidores y capturas de la instancia.
 * Restaura la última vista guardada y actualiza su contenido en segundo plano.
 */
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getInstanceMods } from "$lib/api/cubicApi";
import { getInstanceServerIcons, getInstanceServers } from "$lib/api/servers";
import { getInstanceWorlds } from "$lib/api/worlds";
import type { ModDto } from "$lib/types/types";
import type { TileId } from "./instanceNavState.svelte";
import { untrack } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import {
	createTileContentLoader,
	tileStorage,
	type TileContent,
	type TileItem,
} from "./tileContentCache";

export type { TileItem } from "./tileContentCache";

const MAX_ITEMS = 500;
const MAX_SCREENSHOTS = 18;
const MIN_ICONS_TO_HIDE_PLACEHOLDERS = 6;
const EMPTY: TileItem[] = [];

const cache = new SvelteMap<string, TileItem[]>();
const loadTile = createTileContentLoader(cache, tileStorage);

function cacheKey(uuid: string, id: TileId) {
	return `${uuid}:${id}`;
}

/** Elementos ya cargados (reactivo). */
export function tileItems(uuid: string, id: TileId): TileItem[] {
	return cache.get(cacheKey(uuid, id)) ?? EMPTY;
}

export function requestTileContent(
	uuid: string,
	name: string,
	id: TileId,
	refresh = false,
) {
	return untrack(() =>
		loadTile(cacheKey(uuid, id), () => fetchItems(uuid, name, id), refresh),
	);
}

function fromPacks(list: ModDto[]): TileItem[] {
	const withIcon = list.filter((pack) => pack.icon);
	// Con suficientes iconos reales no se mezclan marcadores sin imagen.
	const pool =
		withIcon.length >= MIN_ICONS_TO_HIDE_PLACEHOLDERS
			? withIcon
			: [...withIcon, ...list.filter((pack) => !pack.icon)];
	const result: TileItem[] = pool.slice(0, MAX_ITEMS).map((pack, index) => ({
		key: `${pack.filename}:${index}`,
		src: pack.icon,
		label: pack.name,
	}));
	if (result[0]) result[0].totalCount = list.length;
	return result;
}

async function fetchItems(
	uuid: string,
	name: string,
	id: TileId,
): Promise<TileContent> {
	switch (id) {
		case "market":
			return { items: fromPacks(await getInstanceMods(uuid, true)) };
		case "resources":
		case "shaderpacks": {
			const command =
				id === "resources"
					? "get_instance_resourcepacks"
					: "get_instance_shaderpacks";
			const packs = await invoke<ModDto[]>(command, { id: uuid });
			return {
				items: fromPacks(packs),
				preliminary: packs.some((pack) => !pack.icon_revision),
			};
		}
		case "worlds": {
			const worlds = await getInstanceWorlds(uuid);
			const result: TileItem[] = worlds
				.slice(0, MAX_ITEMS)
				.map((world, index) => ({
					key: `${world.folder}:${index}`,
					src: world.icon
						? `${convertFileSrc(world.icon)}?v=${encodeURIComponent(world.iconRevision ?? "")}`
						: null,
					label: world.name,
				}));
			if (result[0]) result[0].totalCount = worlds.length;
			return { items: result };
		}
		case "servers": {
			const list = await getInstanceServers(uuid);
			const servers = list.servers.slice(0, MAX_ITEMS);
			const wanted = servers.filter((s) => s.hasIcon).map((s) => s.index);
			const icons = new Map<number, string | null>();
			let preliminary = false;
			try {
				if (wanted.length) {
					for (const icon of await getInstanceServerIcons(
						uuid,
						list.revision,
						wanted,
					)) {
						icons.set(icon.index, icon.icon);
					}
				}
			} catch {
				preliminary = true;
			}
			const result: TileItem[] = servers.map((server) => ({
				key: `${server.index}:${server.address}`,
				src: icons.get(server.index) ?? null,
				label: server.address,
			}));
			if (result[0]) result[0].totalCount = list.servers.length;
			return { items: result, preliminary };
		}
		case "screenshots": {
			const paths = await invoke<string[]>(
				"get_all_instance_screenshots",
				{ instanceName: name },
			);
			// Muestra repartida a lo largo de toda la lista, no solo las primeras.
			const picked: string[] = [];
			const step = Math.max(1, paths.length / MAX_SCREENSHOTS);
			for (
				let i = 0;
				i < paths.length && picked.length < MAX_SCREENSHOTS;
				i += step
			) {
				picked.push(paths[Math.floor(i)]);
			}
			const items: TileItem[] = [];
			let preliminary = false;
			for (const path of picked) {
				try {
					const thumb = await invoke<string | null>(
						"get_screenshot_thumbnail",
						{
							instanceId: uuid,
							filename: path.split(/[\\/]/).pop(),
						},
					);
					if (thumb)
						items.push({
							key: path,
							src: thumb,
							label: null,
							full: convertFileSrc(path),
						});
				} catch {
					preliminary = true;
				}
			}
			return { items, preliminary };
		}
	}
}
