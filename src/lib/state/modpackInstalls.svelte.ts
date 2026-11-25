import { SvelteMap } from "svelte/reactivity";

export type InstallTask = { archive: string; version: string; name: string; running: boolean; error: string | null };
type InstallDetails = Pick<InstallTask, "archive" | "version" | "name">;

export const modpackInstalls = new SvelteMap<string, InstallTask>();
const retries = new Map<string, () => Promise<boolean>>();

export function runModpackInstall(
	key: string,
	task: InstallDetails,
	install: () => Promise<void>,
	isCancelled: (error: unknown) => boolean = () => false,
) {
	if (modpackInstalls.has(key)) return Promise.resolve(false);
	const attempt = async () => {
		retries.delete(key);
		modpackInstalls.set(key, { ...task, running: true, error: null });
		try {
			await install();
			modpackInstalls.delete(key);
			return true;
		} catch (cause) {
			if (isCancelled(cause)) modpackInstalls.delete(key);
			else {
				retries.set(key, attempt);
				modpackInstalls.set(key, { ...task, running: false, error: String(cause) });
			}
			return false;
		}
	};
	return attempt();
}

export function retryModpackInstall(key: string) {
	return retries.get(key)?.() ?? Promise.resolve(false);
}

export function dismissModpackInstall(key: string) {
	if (retries.delete(key)) modpackInstalls.delete(key);
}