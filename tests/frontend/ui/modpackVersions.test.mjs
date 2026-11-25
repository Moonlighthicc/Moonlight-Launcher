import { expect, test } from "bun:test";
import en from "../../../src/lib/i18n/en-US.json";
import ru from "../../../src/lib/i18n/ru-RU.json";
import { curseForgeVersionLoaders, matchesCurseForgePack, packLoaders, versionPackLoaders } from "../../../src/lib/utils/modpackLoaders.ts";
import {
	releaseChannel,
	minecraftVersion,
	packVersionLabel,
	searchPackVersions,
	visiblePackVersions,
} from "../../../src/lib/utils/modpackVersions.ts";

const version = (value, date, channel) => ({
	value,
	label: value,
	date,
	channel,
});
const files = [
	version("release", "2026-01-01", "release"),
	version("alpha", "2026-03-01", "alpha"),
	version("beta", "2026-02-01", "beta"),
];

test("Minecraft version precedes the release name, not a loader or version bucket", () => {
	const gameVersion = minecraftVersion(["Forge", "Minecraft 1.12", "1.12.2"]);
	expect(packVersionLabel("SkyFactory 4 - 4.2.4", gameVersion)).toBe("1.12.2 — SkyFactory 4 - 4.2.4");
	expect(packVersionLabel("Pack Alpha", "25w04a")).toBe("25w04a — Pack Alpha");
	expect(packVersionLabel("Pack", minecraftVersion(["Forge", "Unknown"]))).toBe("Pack");
});

test("loader badges normalize and deduplicate provider metadata", () => {
	expect(packLoaders(["Fabric", 4, 1, "1.20.1", " NeoForge ", null, 5, "adventure"])).toEqual([
		"fabric", "forge", "neoforge", "quilt",
	]);
});

test("CurseForge version badges only use metadata for the selected file", () => {
	const indexes = [{ fileId: 1, modLoader: 4 }, { fileId: 2, modLoader: 1 }];
	expect(curseForgeVersionLoaders({ id: 1, gameVersions: ["1.21.1"] }, indexes)).toEqual(["fabric"]);
	expect(curseForgeVersionLoaders({ id: 3, gameVersions: ["NeoForge"], modLoaders: ["neoforge", "Quilt"] }, indexes)).toEqual(["neoforge", "quilt"]);
	expect(curseForgeVersionLoaders({ id: 3, gameVersions: ["1.21.1"] }, indexes)).toEqual([]);
});

test("CurseForge fills missing per-file badges from alternate fields and matching latest files", () => {
	const files = [
		{ id: 1, gameVersions: [], sortableGameVersions: [{ gameVersion: "1.21.1", gameVersionName: "Neo Forge" }] },
		{ id: 2, gameVersions: ["Fabric"] },
	];
	expect(curseForgeVersionLoaders({ id: 1, gameVersions: ["1.21.1"], modLoaders: ["modloader-quilt"] }, [], files)).toEqual(["neoforge", "quilt"]);
	expect(curseForgeVersionLoaders({ id: 3, gameVersions: ["1.21.1"], modLoaders: [4] }, [], files)).toEqual(["fabric"]);
	expect(curseForgeVersionLoaders({ id: 4, gameVersions: ["1.21.1"], fileName: "pack-neoforge-1.21.1.zip" }, [], files)).toEqual(["neoforge"]);
});

test("filename fallback does not override explicit version metadata or match parts of words", () => {
	expect(versionPackLoaders(["Fabric"], "pack-forge-1.20.mrpack")).toEqual(["fabric"]);
	expect(versionPackLoaders([], "pack-[Quilt].mrpack")).toEqual(["quilt"]);
	expect(versionPackLoaders([], "Fabrication-1.0.mrpack", "Reforged.zip")).toEqual([]);
});

test("CurseForge combines loader and Minecraft version on the same client file", () => {
	const pack = {
		latestFilesIndexes: [
			{ gameVersion: "1.21.1", modLoader: 6 },
			{ gameVersion: "1.20.1", modLoader: 1 },
		],
		latestFiles: [
			{ gameVersions: ["1.21.1", "Fabric"], isServerPack: true },
			{ gameVersions: ["1.20.1", "Fabric"] },
		],
	};
	expect(matchesCurseForgePack(pack, "fabric", null)).toBe(true);
	expect(matchesCurseForgePack(pack, "fabric", "1.20.1")).toBe(true);
	expect(matchesCurseForgePack(pack, "fabric", "1.21.1")).toBe(false);
	expect(matchesCurseForgePack(pack, "neoforge", "1.20.1")).toBe(false);
	expect(matchesCurseForgePack(pack, "neoforge", "1.21.1")).toBe(true);
});

test("Modrinth and CurseForge map to the same release channels", () => {
	for (const [modrinth, curseforge] of [
		["release", 1],
		["beta", 2],
		["alpha", 3],
	]) {
		expect(releaseChannel(modrinth)).toBe(modrinth);
		expect(releaseChannel(curseforge)).toBe(modrinth);
	}
});

test("alpha files are hidden and cannot be marked latest without opting in", () => {
	expect(visiblePackVersions(files, false)).toEqual({
		latest: "beta",
		versions: [files[2], files[0]],
	});
	expect(visiblePackVersions(files, false, "alpha").versions).toEqual([]);
	expect(visiblePackVersions(files, true, "latest")).toEqual({
		latest: "alpha",
		versions: [files[1]],
	});
});

test("channel filters do not change which file gets the latest badge", () => {
	expect(visiblePackVersions(files, true, "release")).toEqual({
		latest: "alpha",
		versions: [files[0]],
	});
	expect(visiblePackVersions(files, true, "beta").versions).toEqual([
		files[2],
	]);
});

test("version search matches names, ISO dates and localized dates", () => {
	const entries = files.map((file) => ({ ...file, label: `Sky Pack ${file.label}` }));
	expect(searchPackVersions(entries, "sKY beta", "en-US")).toEqual([entries[2]]);
	expect(searchPackVersions(entries, "2026-02-01", "en-US")).toEqual([entries[2]]);
	expect(searchPackVersions(entries, "02/01/2026", "en-US")).toEqual([entries[2]]);
	expect(searchPackVersions(entries, "01.02.2026", "ru-RU")).toEqual([entries[2]]);
	expect(searchPackVersions(entries, "февраля 2026", "ru-RU")).toEqual([entries[2]]);
	expect(searchPackVersions(entries, "01/02/2026", "en-US")).toEqual([]);
	expect(searchPackVersions(entries, "   ", "en-US")).toBe(entries);
});

test("search respects the release filters without changing the latest version", () => {
	const list = visiblePackVersions(files, false, "release");
	expect(searchPackVersions(list.versions, "beta", "en-US")).toEqual([]);
	expect(searchPackVersions(list.versions, "2026-01", "en-US")).toEqual([files[0]]);
	expect(list.latest).toBe("beta");
});

test("the version search, name editor and download collapse have English and Russian labels", () => {
	for (const [dict, versions, saved, cancel] of [
		[en, "Versions", "Saved", "Cancel"],
		[ru, "Версии", "Сохранено", "Отменить"],
	]) {
		const pack = dict.createInstance.pack;
		expect([pack.versions, pack.nameSaved, pack.cancelName]).toEqual([versions, saved, cancel]);
		expect(pack.searchVersions.trim()).not.toBeEmpty();
		expect(pack.noFiles.trim()).not.toBeEmpty();
		expect(dict.sidebar.collapseDownloads.trim()).not.toBeEmpty();
	}
});
