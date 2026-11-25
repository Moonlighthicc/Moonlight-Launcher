import {
	getAvailableVersions,
	getFabricVersions,
	getLoaderVersionCatalog,
	getQuiltVersions,
} from "$lib/api/cubicApi";
import {
	catalogToGrouped,
	compareVersions,
	type GroupedLoaderVersions,
} from "$lib/components/layout/VersionDownloader/versionCatalog";

export async function getSelectionCatalog(
	loader: string,
): Promise<GroupedLoaderVersions> {
	// Forge, NeoForge y OptiFine llegan ya agrupados y ordenados por Rust.
	if (loader === "forge" || loader === "neoforge" || loader === "optifine") {
		return catalogToGrouped(await getLoaderVersionCatalog(loader));
	}
	const gameVersions =
		loader === "vanilla"
			? (await getAvailableVersions()).map((v) => v.id)
			: (
					await (loader === "fabric"
						? getFabricVersions()
						: getQuiltVersions())
				)
					.map((v) => v.version)
					.sort(compareVersions);
	return { byGame: new Map(), gameVersions, stableGameVersions: [] };
}

export function minecraftVersionVisible(type: string | undefined, showSnapshots: boolean, showAlpha: boolean) {
	if (type === "snapshot") return showSnapshots;
	if (type === "old_alpha" || type === "old_beta") return showAlpha;
	return true;
}

export function instanceVersionId(loader: string, mc: string, version: string): string {
	if (!mc) return "";
	if (loader === "vanilla") return mc;
	if (!version) return "";
	if (loader === "fabric" || loader === "quilt") return `${loader}-loader-${version}-${mc}`;
	if (loader === "optifine") return `${mc}-OptiFine_${version}`;
	return `${mc}-${loader}-${version}`;
}
