<script lang="ts">
	import type { InstanceDto } from "$lib/types/types";
	import Icon from "$lib/icons/Icon.svelte";
	import { getDisplayIconSrc } from "$lib/icons/logos";

	let {
		instance,
		x,
		y,
		compact = false,
		hot = false,
	}: {
		instance: InstanceDto;
		x: number;
		y: number;
		/** Solo el icono (barra compacta). */
		compact?: boolean;
		/** Sobre la zona de anclado. */
		hot?: boolean;
	} = $props();

	const size = $derived(compact ? 32 : 24);
</script>

<div
	class="ghost"
	class:compact
	class:hot
	style:left="{x}px"
	style:top="{y}px"
	aria-hidden="true"
>
	{#if instance.icon}
		<img
			src={getDisplayIconSrc(instance.icon)}
			alt=""
			width={size}
			height={size}
			draggable="false"
		/>
	{:else}
		<Icon name="brand:cubic" {size} />
	{/if}
	{#if !compact}
		<span class="ghost-name">{instance.name}</span>
	{/if}
</div>

<style>
	.ghost {
		position: fixed;
		z-index: 2000;
		display: flex;
		align-items: center;
		gap: 10px;
		max-width: 240px;
		padding: 8px 12px;
		transform: translate(-18px, -50%) scale(1.03);
		border-radius: var(--border-radius-lg);
		border: 1px solid var(--accent);
		background: var(--bg-card, var(--bg-sidebar));
		color: var(--text-primary);
		box-shadow: var(--shadow-lg, 0 10px 30px rgba(0, 0, 0, 0.4));
		pointer-events: none;
		transition:
			transform 0.12s ease,
			opacity 0.12s ease;
	}

	.ghost.compact {
		padding: 6px;
		transform: translate(-50%, -50%) scale(1.03);
	}

	.ghost.hot {
		transform: translate(-18px, -50%) scale(0.92);
		opacity: 0.85;
	}

	.ghost.compact.hot {
		transform: translate(-50%, -50%) scale(0.92);
	}

	.ghost img {
		display: block;
		border-radius: var(--border-radius-sm);
		object-fit: cover;
		flex-shrink: 0;
	}

	.ghost-name {
		overflow: hidden;
		font-size: 0.85rem;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
