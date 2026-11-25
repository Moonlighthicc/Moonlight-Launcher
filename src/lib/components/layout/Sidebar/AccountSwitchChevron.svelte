<script lang="ts">
	import ChevronDownIcon from "$lib/icons/ChevronDownIcon.svelte";
	import type { SwitcherPlacement } from "./AccountSwitcher.svelte";

	let { placement, open, size = 16 }: {
		placement: SwitcherPlacement;
		open: boolean;
		size?: number;
	} = $props();
	const rotations = { down: 0, up: 180, left: 90, right: -90 };
</script>

<span class="chevron" style:--account-chevron-rotation={`${rotations[placement] + (open ? 180 : 0)}deg`}>
	<ChevronDownIcon {size} class="account-switch-chevron" />
</span>

<style>
	.chevron { display: inline-flex; flex-shrink: 0; }
</style>