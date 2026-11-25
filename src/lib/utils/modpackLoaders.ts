export const PACK_LOADERS = {
	fabric: "Fabric",
	forge: "Forge",
	neoforge: "NeoForge",
	quilt: "Quilt",
} as const;

export type PackLoader = keyof typeof PACK_LOADERS;

const curseForgeIds: Partial<Record<number, PackLoader>> = {
	1: "forge",
	4: "fabric",
	5: "quilt",
	6: "neoforge",
};

export function packLoaders(values: readonly (string | number | null | undefined)[]): PackLoader[] {
	const found = new Set(values.map((value) => {
		if (typeof value === "number") return curseForgeIds[value];
		const name = value?.trim().toLowerCase();
		if (!name) return undefined;
		if (/^\d+$/.test(name)) return curseForgeIds[Number(name)];
		return name.replace(/^modloader[\s_-]*/, "").replace(/[\s_-]*loader$/, "").replace(/[\s_-]/g, "");
	}));
	return (Object.keys(PACK_LOADERS) as PackLoader[]).filter((loader) => found.has(loader));
}

export function versionPackLoaders(
	values: readonly (string | number | null | undefined)[],
	...fileNames: (string | null | undefined)[]
): PackLoader[] {
	const explicit = packLoaders(values);
	if (explicit.length) return explicit;
	// Only standalone loader names from this release; do not inherit another file's loader.
	return packLoaders(fileNames.flatMap((name) => name?.match(
		/(?<![\p{L}\p{N}])(?:neo[ _-]?forge|forge|fabric|quilt)(?![\p{L}\p{N}])/giu,
	) ?? []));
}

type CurseForgeLoaderFields = {
	gameVersions: string[];
	sortableGameVersions?: readonly { gameVersion: string; gameVersionName?: string }[] | null;
	modLoaders?: readonly (string | number)[] | null;
	fileName?: string;
	displayName?: string;
	isServerPack?: boolean;
};

function curseForgeFileLoaderFields(file: CurseForgeLoaderFields) {
	return [
		...file.gameVersions,
		...(file.modLoaders ?? []),
		...(file.sortableGameVersions ?? []).flatMap((entry) => [entry.gameVersion, entry.gameVersionName]),
	];
}

export function curseForgeVersionLoaders(
	file: CurseForgeLoaderFields & { id: number },
	indexes: readonly { fileId: number; modLoader?: number | null }[] = [],
	latestFiles: readonly (CurseForgeLoaderFields & { id: number })[] = [],
): PackLoader[] {
	return versionPackLoaders([
		...curseForgeFileLoaderFields(file),
		...latestFiles.filter((entry) => entry.id === file.id).flatMap(curseForgeFileLoaderFields),
		...indexes.filter((index) => index.fileId === file.id).map((index) => index.modLoader),
	], file.fileName, file.displayName);
}

export function matchesCurseForgePack(
	pack: {
		latestFilesIndexes?: { gameVersion: string; modLoader?: number | null }[];
		latestFiles?: CurseForgeLoaderFields[];
	},
	loader: string | null,
	gameVersion: string | null,
) {
	if (!loader && !gameVersion) return true;
	return (pack.latestFilesIndexes ?? []).some((file) =>
		(!gameVersion || file.gameVersion === gameVersion) && (!loader || packLoaders([file.modLoader]).some((value) => value === loader)),
	) || (pack.latestFiles ?? []).some((file) =>
		!file.isServerPack && (!gameVersion || file.gameVersions.includes(gameVersion)) &&
		(!loader || versionPackLoaders(curseForgeFileLoaderFields(file), file.fileName, file.displayName).some((value) => value === loader)),
	);
}