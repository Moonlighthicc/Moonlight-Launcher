<script lang="ts">
	import { t } from "$lib/i18n";
	import ModalBase from "$lib/components/layout/ModalBase.svelte";
	import type { PackGalleryImage } from "$lib/utils/modpackVersions";
	let {
		images,
		index = $bindable(),
		onclose,
	}: {
		images: PackGalleryImage[];
		index: number;
		onclose: () => void;
	} = $props();
	let open = $state(true);
	let zoom = $state(1),
		x = $state(0),
		y = $state(0);
	let stage = $state<HTMLDivElement>();
	let picture = $state<HTMLImageElement>();
	let drag: { id: number; start: number; x: number; y: number } | null = null;
	const image = $derived(images[index]);
	function step(delta: number) {
		index = (index + delta + images.length) % images.length;
		setZoom(1);
		drag = null;
	}
	function bound() {
		if (!stage) return;
		const maxX = Math.max(
			0,
			((picture?.offsetWidth ?? 0) * zoom - stage.clientWidth) / 2,
		);
		const maxY = Math.max(
			0,
			((picture?.offsetHeight ?? 0) * zoom - stage.clientHeight) / 2,
		);
		x = Math.min(maxX, Math.max(-maxX, x));
		y = Math.min(maxY, Math.max(-maxY, y));
	}
	function setZoom(value: number, clientX?: number, clientY?: number) {
		if (!stage) return;
		const next = Math.min(6, Math.max(1, value));
		if (next === 1) {
			zoom = 1;
			x = y = 0;
			return;
		}
		const rect = stage.getBoundingClientRect();
		const px =
			clientX === undefined ? 0 : clientX - rect.left - rect.width / 2;
		const py =
			clientY === undefined ? 0 : clientY - rect.top - rect.height / 2;
		x = px - ((px - x) * next) / zoom;
		y = py - ((py - y) * next) / zoom;
		zoom = next;
		bound();
	}
	function wheel(node: HTMLElement) {
		const handle = (event: WheelEvent) => {
			event.preventDefault();
			setZoom(
				zoom * Math.exp(-event.deltaY * 0.002),
				event.clientX,
				event.clientY,
			);
		};
		node.addEventListener("wheel", handle, { passive: false });
		return { destroy: () => node.removeEventListener("wheel", handle) };
	}
	function keydown(event: KeyboardEvent) {
		if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
		event.preventDefault();
		step(event.key === "ArrowLeft" ? -1 : 1);
	}
</script>

<ModalBase
	bind:open
	width="min(1100px, 95vw)"
	title={t("createInstance.pack.gallery")}
	focusOnOpen
	{onclose}
>
	{#if image}
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<div
			class="viewer"
			role="group"
			aria-label={t("createInstance.pack.gallery")}
			onkeydown={keydown}
		>
			<!-- Pointer gestures have equivalent buttons in the toolbar. -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="stage"
				class:zoomed={zoom > 1}
				bind:this={stage}
				use:wheel
				ondblclick={(e) =>
					setZoom(zoom === 1 ? 2.5 : 1, e.clientX, e.clientY)}
				onpointerdown={(e) => {
					if (e.button !== 0 || drag) return;
					drag = {
						id: e.pointerId,
						start: e.clientX,
						x: e.clientX,
						y: e.clientY,
					};
					e.currentTarget.setPointerCapture(e.pointerId);
				}}
				onpointermove={(e) => {
					if (drag?.id !== e.pointerId || zoom === 1) return;
					x += e.clientX - drag.x;
					y += e.clientY - drag.y;
					drag.x = e.clientX;
					drag.y = e.clientY;
					bound();
				}}
				onpointerup={(e) => {
					if (drag?.id !== e.pointerId) return;
					const delta = e.clientX - drag.start;
					drag = null;
					if (zoom === 1 && Math.abs(delta) > 55)
						step(delta < 0 ? 1 : -1);
				}}
				onlostpointercapture={() => (drag = null)}
			>
				{#key image.url}<img
						bind:this={picture}
						src={image.url}
						alt={image.title ||
							t("instanceView.screenshots.fullSize")}
						draggable="false"
						style:transform={`translate(${x}px, ${y}px) scale(${zoom})`}
					/>{/key}
			</div>
			<div class="tools">
				<button
					type="button"
					class="btn-secondary"
					disabled={images.length < 2}
					aria-label={t("common.previous")}
					onclick={() => step(-1)}>‹</button
				>
				<span>{index + 1} / {images.length}</span>
				<button
					type="button"
					class="btn-secondary"
					disabled={images.length < 2}
					aria-label={t("common.next")}
					onclick={() => step(1)}>›</button
				>
				<button
					type="button"
					class="btn-secondary"
					disabled={zoom === 1}
					aria-label={t("createInstance.pack.zoomOut")}
					onclick={() => setZoom(zoom / 1.25)}>−</button
				>
				<button
					type="button"
					class="btn-secondary"
					aria-label={t("createInstance.pack.resetZoom")}
					onclick={() => setZoom(1)}>{Math.round(zoom * 100)}%</button
				>
				<button
					type="button"
					class="btn-secondary"
					disabled={zoom === 6}
					aria-label={t("createInstance.pack.zoomIn")}
					onclick={() => setZoom(zoom * 1.25)}>+</button
				>
			</div>
			{#if image.title || image.description}<p>
					<strong>{image.title}</strong>
					{image.description}
				</p>{/if}
		</div>
	{/if}
</ModalBase>

<style>
	.stage {
		display: grid;
		place-items: center;
		height: min(60vh, 660px);
		overflow: hidden;
		background: var(--bg-card);
		border-radius: var(--border-radius-sm);
		touch-action: none;
		cursor: zoom-in;
	}
	.stage.zoomed {
		cursor: grab;
	}
	.stage.zoomed:active {
		cursor: grabbing;
	}
	.stage img {
		display: block;
		max-width: 100%;
		max-height: min(60vh, 660px);
		object-fit: contain;
		user-select: none;
	}
	.tools {
		display: flex;
		justify-content: center;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 8px;
	}
	p {
		text-align: center;
		overflow-wrap: anywhere;
		margin: 6px 0;
	}
</style>
