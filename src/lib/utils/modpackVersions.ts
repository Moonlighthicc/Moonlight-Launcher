import type { PackLoader } from "./modpackLoaders";

export type ReleaseChannel = "release" | "beta" | "alpha";
export type ReleaseFilter = ReleaseChannel | "all" | "latest";
export type PackVersion = {
	value: string;
	label: string;
	date: string;
	url?: string;
	channel: ReleaseChannel;
	packLoaders?: PackLoader[];
};
export type PackGalleryImage = {
	url: string;
	thumbnail?: string;
	title?: string | null;
	description?: string | null;
};
export type PackLoaders = {
	format: "html" | "markdown";
	changelog: (version: string, signal: AbortSignal) => Promise<string>;
	onVisibleVersions?: (versions: PackVersion[]) => void;
};

export function minecraftVersion(values: readonly (string | undefined)[]) {
	return values.map((value) => value?.trim()).find((value) => value &&
		/^(?:\d+\.\d+(?:\.\d+)?(?:[_ -]?(?:pre(?:-?release)?|rc|snapshot).*)?|\d{2}w\d{2}[a-z]|[abc]\d+\.\d+(?:\.\d+)?[a-z]?)$/i.test(value),
	);
}

export function packVersionLabel(name: string, gameVersion?: string) {
	return gameVersion ? `${gameVersion} — ${name}` : name;
}

export function releaseChannel(
	value: string | number | undefined,
): ReleaseChannel {
	return value === "alpha" || value === 3
		? "alpha"
		: value === "beta" || value === 2
			? "beta"
			: "release";
}

export function visiblePackVersions(
	versions: PackVersion[],
	showAlpha: boolean,
	filter: ReleaseFilter = "all",
) {
	const visible = versions
		.filter((v) => showAlpha || v.channel !== "alpha")
		.sort((a, b) => b.date.localeCompare(a.date));
	const latest = visible[0]?.value;
	return {
		latest,
		versions:
			filter === "all"
				? visible
				: filter === "latest"
					? visible.slice(0, 1)
					: visible.filter((v) => v.channel === filter),
	};
}

function searchWords(value: string, locale: string) {
	return value.toLocaleLowerCase(locale).match(/[\p{L}\p{N}]+/gu) ?? [];
}

export function searchPackVersions(versions: PackVersion[], query: string, locale: string) {
	const groups = query.trim().split(/\s+/).map((part) => searchWords(part, locale)).filter((part) => part.length);
	if (!groups.length) return versions;
	const longDate = new Intl.DateTimeFormat(locale, { dateStyle: "long" });
	const shortDate = new Intl.DateTimeFormat(locale, { day: "2-digit", month: "2-digit", year: "numeric" });
	return versions.filter(({ label, date }) => {
		const published = new Date(/^\d{4}-\d{2}-\d{2}$/.test(date) ? `${date}T00:00:00` : date);
		const fields = [label];
		if (Number.isFinite(published.getTime())) {
			fields.push(date.slice(0, 10), longDate.format(published), shortDate.format(published));
		}
		const tokens = fields.map((field) => searchWords(field, locale));
		// Match contiguous date/version tokens too, so 02/01 and 01/02 aren't interchangeable.
		return groups.every((group) => tokens.some((words) => words.some((_, i) =>
			group.every((part, j) => words[i + j]?.includes(part)),
		)));
	});
}
