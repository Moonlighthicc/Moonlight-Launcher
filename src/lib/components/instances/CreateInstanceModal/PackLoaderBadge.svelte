<script lang="ts">
	import { getLoaderColorVar, getLoaderLogo } from "$lib/icons/logos";
	import { PACK_LOADERS, type PackLoader } from "$lib/utils/modpackLoaders";
	let { loader }: { loader: PackLoader } = $props();
</script>

<span style:--loader-color={getLoaderColorVar(loader)}>
	<img src={getLoaderLogo(loader)} width="14" height="14" alt="" />
	{PACK_LOADERS[loader]}
</span>

<style>
	span {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 6px;
		border-radius: var(--border-radius-sm);
		color: var(--loader-color);
		background: color-mix(in srgb, var(--loader-color) 12%, transparent);
		font-size: 0.7rem;
		white-space: nowrap;
	}
	img { object-fit: contain; }
</style>