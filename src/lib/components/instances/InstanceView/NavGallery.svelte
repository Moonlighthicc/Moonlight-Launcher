<script lang="ts">
	import {
		requestTileContent,
		tileItems,
	} from "$lib/state/tileContentState.svelte";
	import type { TileId } from "$lib/state/instanceNavState.svelte";
	import { SvelteSet } from "svelte/reactivity";

	let {
		uuid,
		name,
		ids,
		width,
		size = 22,
	}: {
		uuid: string;
		name: string;
		/** Secciones de las que se toman las imágenes (se mezclan una a una). */
		ids: TileId[];
		/** Ancho máximo disponible en la cabecera, en px. */
		width: number;
		size?: number;
	} = $props();

	const GAP = 6;
	const STEP_MS = 2600;
	const SLIDE_MS = 520;
	const MAX_IMAGES = 30;
	const reducedMotion =
		typeof matchMedia === "function" &&
		matchMedia("(prefers-reduced-motion: reduce)").matches;

	$effect(() => {
		for (const id of ids) requestTileContent(uuid, name, id);
	});

	/** Imágenes de todas las secciones, intercaladas para que haya variedad. */
	const images = $derived.by(() => {
		const lists = ids.map((id) =>
			tileItems(uuid, id).filter((item) => item.src),
		);
		const out: string[] = [];
		const seen = new SvelteSet<string>();
		for (let k = 0; ; k++) {
			let any = false;
			for (const list of lists) {
				const item = list[k];
				if (!item?.src) continue;
				any = true;
				if (!seen.has(item.src)) {
					seen.add(item.src);
					out.push(item.src);
				}
			}
			if (!any || out.length >= MAX_IMAGES) break;
		}
		return out.slice(0, MAX_IMAGES);
	});

	const visible = $derived(
		Math.max(1, Math.min(6, Math.floor((width + GAP) / (size + GAP)))),
	);
	const stepping = $derived(images.length > visible && !reducedMotion);
	// Con copia de los primeros elementos al final, el bucle vuelve a 0 sin saltos.
	const shown = $derived(
		images.length > visible
			? [...images, ...images.slice(0, visible)]
			: images,
	);

	let pos = $state(0);
	let animate = $state(true);

	$effect(() => {
		if (!stepping) return;
		const count = images.length;
		let resetTimer: ReturnType<typeof setTimeout> | undefined;
		const timer = setInterval(() => {
			animate = true;
			pos += 1;
			if (pos >= count) {
				resetTimer = setTimeout(() => {
					animate = false;
					pos = 0;
					requestAnimationFrame(() =>
						requestAnimationFrame(() => {
							animate = true;
						}),
					);
				}, SLIDE_MS + 40);
			}
		}, STEP_MS);
		return () => {
			clearInterval(timer);
			clearTimeout(resetTimer);
			pos = 0;
			animate = true;
		};
	});
</script>

{#if images.length > 0 && width >= size}
	<span
		class="gallery"
		style:width="{Math.min(visible, images.length) * (size + GAP) - GAP}px"
		style:height="{size}px"
		aria-hidden="true"
	>
		<span
			class="strip"
			class:animate
			style:gap="{GAP}px"
			style:transform="translate3d({-pos * (size + GAP)}px, 0, 0)"
		>
			{#each shown as src, i (i)}
				<img
					class="g-img"
					{src}
					alt=""
					draggable="false"
					loading="lazy"
					decoding="async"
					style:width="{size}px"
					style:height="{size}px"
				/>
			{/each}
		</span>
	</span>
{/if}

<style>
	.gallery {
		display: block;
		flex-shrink: 0;
		overflow: hidden;
		pointer-events: none;
		user-select: none;
	}

	.strip {
		display: flex;
		will-change: transform;
	}

	.strip.animate {
		transition: transform 520ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.g-img {
		display: block;
		flex-shrink: 0;
		border-radius: 24%;
		object-fit: cover;
		background: color-mix(in srgb, var(--text-primary) 8%, transparent);
	}
</style>
