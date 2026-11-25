import type { McVersion } from "../types/types";

export interface VersionIntegrity {
	version_id: string;
	dependencies: string[];
	missing: string[];
	complete: boolean;
}

export interface VersionStatus {
	version_id: string;
	complete: boolean;
	missing_deps: string[];
}

export function parseInstalledVersion(raw: string): McVersion {
	if (raw.includes("-OptiFine_")) {
		return { loader: "optifine", version: raw, type: "" };
	}
	if (raw.includes("fabric")) {
		const clean = raw
			.replace(/^fabric-loader-[\d.]+-/, "")
			.replace(/-fabric$/, "");
		return { loader: "fabric", version: clean, type: "" };
	}
	if (raw.includes("-neoforge-")) {
		const idx = raw.indexOf("-neoforge-");
		const mcVersion = raw.substring(0, idx);
		const neoforgeVersion = raw.substring(idx + 10);
		return {
			loader: "neoforge",
			version: `${mcVersion}-neoforge-${neoforgeVersion}`,
			type: "",
		};
	}
	if (raw.includes("-forge-")) {
		const idx = raw.indexOf("-forge-");
		const mcVersion = raw.substring(0, idx);
		const forgeVersion = raw.substring(idx + 7);
		return {
			loader: "forge",
			version: `${mcVersion}-forge-${forgeVersion}`,
			type: "",
		};
	}
	if (raw.includes("quilt-loader-")) {
		const clean = raw.replace(/^quilt-loader-[\d.]+-/, "");
		return { loader: "quilt", version: clean, type: "" };
	}
	return { loader: "vanilla", version: raw, type: "" };
}

export function installedVersionInfo(id: string) {
	const prefixed = /^(fabric|quilt)-loader-(.+?)-((?:\d|[a-z]+\d).*)$/.exec(id.replace(/-fabric$/, ""));
	if (prefixed) {
		const [, loader, version, game] = prefixed;
		return { id, loader, version, game };
	}
	for (const [loader, separator] of [
		["neoforge", "-neoforge-"],
		["forge", "-forge-"],
		["optifine", "-OptiFine_"],
	]) {
		const index = id.indexOf(separator);
		if (index > 0) {
			return { id, loader, version: id.slice(index + separator.length), game: id.slice(0, index) };
		}
	}
	return { id, loader: "vanilla", game: id, version: id };
}

export function getInstalledMcVersions(raw: string[]): {
	vanilla: Set<string>;
	fabric: Set<string>;
	forge: Set<string>;
	neoforge: Set<string>;
	quilt: Set<string>;
	optifine: Set<string>;
} {
	const vanilla = new Set<string>();
	const fabric = new Set<string>();
	const forge = new Set<string>();
	const neoforge = new Set<string>();
	const quilt = new Set<string>();
	const optifine = new Set<string>();
	for (const v of raw) {
		const parsed = parseInstalledVersion(v);
		if (parsed.loader === "vanilla") vanilla.add(parsed.version);
		else if (parsed.loader === "fabric") fabric.add(parsed.version);
		else if (parsed.loader === "forge") forge.add(parsed.version);
		else if (parsed.loader === "neoforge") neoforge.add(parsed.version);
		else if (parsed.loader === "quilt") quilt.add(parsed.version);
		else if (parsed.loader === "optifine") optifine.add(parsed.version);
	}
	return { vanilla, fabric, forge, neoforge, quilt, optifine };
}

function addInstalledLoaderVersion(
	map: Map<string, Set<string>>,
	loader: string,
	mcVersion: string,
	loaderVersion: string,
) {
	const key = `${loader}:${mcVersion}`;
	if (!map.has(key)) {
		map.set(key, new Set<string>());
	}
	map.get(key)!.add(loaderVersion);
}

/**
 * Devuelve un mapa con las versiones de loader instaladas, agrupadas por
 * loader y versión de Minecraft. La clave es `${loader}:${mcVersion}` y el
 * valor es el conjunto de versiones del loader instaladas para esa
 * combinación.
 */
export function getInstalledLoaderVersions(
	raw: string[],
): Map<string, Set<string>> {
	const result = new Map<string, Set<string>>();

	for (const v of raw) {
		const { loader, game, version } = installedVersionInfo(v);
		if (loader !== "vanilla") addInstalledLoaderVersion(result, loader, game, version);
	}

	return result;
}
