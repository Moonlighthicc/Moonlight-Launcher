import { addToQueue, checkVersionIntegrity, launchInstance } from "$lib/api/cubicApi";
import { isVersionDownloading } from "$lib/state/downloadState.svelte";
import { hasDownloadFailed, trackConnecting } from "$lib/state/downloadQueueState.svelte";
import { clearGameReady } from "$lib/state/gameReadyState.svelte";
import { InstState, type InstanceDto } from "$lib/types/types";

export function isInstanceDownloading(instance: InstanceDto) {
	return isVersionDownloading(instance.version) || isVersionDownloading(`modpack-${instance.uuid}`);
}

/** Cards check install integrity on demand, rather than scanning every instance on render. */
export async function playInstance(instance: InstanceDto, needsDownload?: boolean) {
	if (instance.status === InstState.Starting || isInstanceDownloading(instance)) return;
	if (instance.status !== InstState.Started) {
		const integrity = needsDownload === undefined ? await checkVersionIntegrity(instance.version) : null;
		const missing = needsDownload ?? (integrity ? !integrity.complete : hasDownloadFailed(instance.version));
		if (isInstanceDownloading(instance)) return;
		if (missing) {
			await trackConnecting(instance.version, () => addToQueue(instance.version));
			return;
		}
	}
	clearGameReady(instance.uuid);
	await launchInstance(instance);
}