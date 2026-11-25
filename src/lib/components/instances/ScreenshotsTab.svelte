<script lang="ts">
	import { onMount, onDestroy, untrack, tick } from "svelte";
	import { invoke, convertFileSrc } from "@tauri-apps/api/core";
	import { deleteInstanceFile, openInstanceDir } from "$lib/api/cubicApi";
	import type { InstanceDto } from "$lib/types/types";
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import ScrollToTopButton from "$lib/components/ui/ScrollToTopButton.svelte";
	import { createScreenshotThumbnails } from "$lib/state/screenshotThumbnails";
	import { SvelteMap, SvelteSet } from "svelte/reactivity";
	import ModalBase from "../layout/ModalBase.svelte";
	import { observeThemeMetrics } from "$lib/utils/themeMetrics";
	import { fade } from "svelte/transition";
	import { animDuration, windowSlide } from "$lib/utils/animations";

	let { instance }: { instance: InstanceDto } = $props();
	let screenshots = $state<string[]>([]);
	let selectedImage = $state<string | null>(null);
	let loading = $state(true);
	let error = $state("");
	let deleting = $state<string | null>(null);
	let selected = new SvelteSet<string>();
	let selectionMode = $state(false);
	let deleteTarget = $state<string | null>(null);
	let deleteModalOpen = $state(false);
	let bulkDeleteModal = $state(false);
	let zoom = $state(1);
	let panX = $state(0);
	let panY = $state(0);
	let animating = $state(false);
	let imgEl = $state<HTMLImageElement>();
	let grid = $state<HTMLDivElement>();
	let width = $state(0);
	let height = $state(0);
	let scrollTop = $state(0);
	let visible = $state(true);
	let metrics = $state({ card: 200, gap: 16 });
	let frame: number | undefined;
	let alive = true;
	let request = 0;
	let currentInstanceId = "";
	const thumbnails = createScreenshotThumbnails((path) =>
		invoke<string | null>("get_screenshot_thumbnail", {
			instanceId: currentInstanceId,
			filename: path.split(/[\\/]/).pop(),
		}),
	);
	const columns = $derived(
		Math.max(
			1,
			Math.floor((width + metrics.gap) / (metrics.card + metrics.gap)),
		),
	);
	const rowHeight = $derived(
		Math.max(
			1,
			(((width - metrics.gap * (columns - 1)) / columns) * 9) / 16 +
				metrics.gap,
		),
	);
	const rowCount = $derived(Math.ceil(screenshots.length / columns));
	const firstRow = $derived(
		Math.max(
			0,
			Math.min(rowCount - 1, Math.floor(scrollTop / rowHeight)) - 2,
		),
	);
	const lastRow = $derived(
		Math.min(rowCount, Math.ceil((scrollTop + height) / rowHeight) + 2),
	);
	const rows = $derived(
		Array.from(
			{ length: Math.max(0, lastRow - firstRow) },
			(_, index) => firstRow + index,
		),
	);

	async function loadScreenshots(id = instance.uuid, name = instance.name) {
		const token = ++request;
		loading = true;
		error = "";
		thumbnails.reset();
		try {
			const result = await invoke<string[]>(
				"get_all_instance_screenshots",
				{ instanceName: name },
			);
			if (!alive || token !== request || id !== currentInstanceId) return;
			screenshots = result;
		} catch (err) {
			if (alive && token === request) error = String(err);
		} finally {
			if (alive && token === request) loading = false;
		}
	}

	$effect(() => {
		const id = instance.uuid;
		const name = instance.name;
		untrack(() => {
			currentInstanceId = id;
			screenshots = [];
			selectedImage = null;
			deleting = null;
			selected.clear();
			selectionMode = false;
			deleteTarget = null;
			deleteModalOpen = false;
			bulkDeleteModal = false;
			scrollTop = 0;
			if (grid) grid.scrollTop = 0;
			void loadScreenshots(id, name);
		});
	});

	$effect(() => {
		const paths =
			!loading && visible && width > 0 && height > 0
				? screenshots.slice(firstRow * columns, lastRow * columns)
				: [];
		untrack(() => thumbnails.request(paths));
	});

	function handleScroll() {
		if (frame !== undefined) return;
		frame = requestAnimationFrame(() => {
			frame = undefined;
			if (grid) scrollTop = grid.scrollTop;
		});
	}

	onMount(() => {
		const viewport = grid;
		if (!viewport) return;
		const stopMetrics = observeThemeMetrics(
			viewport,
			{
				card: { variable: "--screenshot-min-width", fallback: 200 },
				gap: {
					variable: "--screenshot-gap",
					fallback: 16,
					allowZero: true,
				},
			},
			(values) => {
				metrics = values;
			},
		);
		const observer = new ResizeObserver(([entry]) => {
			width = entry.contentRect.width;
			height = entry.contentRect.height;
			scrollTop = viewport.scrollTop;
		});
		observer.observe(viewport);
		const visibility = () => {
			visible = !document.hidden;
		};
		visibility();
		document.addEventListener("visibilitychange", visibility);
		return () => {
			observer.disconnect();
			stopMetrics();
			document.removeEventListener("visibilitychange", visibility);
		};
	});
	onDestroy(() => {
		alive = false;
		request++;
		thumbnails.destroy();
		if (frame !== undefined) cancelAnimationFrame(frame);
	});

	async function navigate(event: KeyboardEvent, index: number) {
		const viewport = grid;
		if (!viewport) return;
		const targets: Record<string, number> = {
			ArrowLeft: index - 1,
			ArrowRight: index + 1,
			ArrowUp: index - columns,
			ArrowDown: index + columns,
			Home: 0,
			End: screenshots.length - 1,
		};
		if (!(event.key in targets)) return;
		event.preventDefault();
		const target = Math.max(
			0,
			Math.min(screenshots.length - 1, targets[event.key]),
		);
		const top = Math.floor(target / columns) * rowHeight;
		if (top < viewport.scrollTop) viewport.scrollTop = top;
		else if (top + rowHeight > viewport.scrollTop + height)
			viewport.scrollTop = top + rowHeight - height;
		scrollTop = viewport.scrollTop;
		await tick();
		if (alive)
			viewport.querySelector<HTMLButtonElement>(
				`[data-index="${target}"] .open-screenshot`,
			)?.focus();
	}

	/** Fecha y hora a partir del nombre estándar de Minecraft (2024-05-12_14.33.21.png). */
	function shotDate(path: string): string | null {
		const name = path.split(/[\\/]/).pop() ?? "";
		const m = name.match(/(\d{4})-(\d{2})-(\d{2})_(\d{2})\.(\d{2})\.(\d{2})/);
		return m ? `${m[3]}.${m[2]}.${m[1]} ${m[4]}:${m[5]}` : null;
	}

	const viewerIndex = $derived(
		selectedImage ? screenshots.indexOf(selectedImage) : -1,
	);
	let controlsVisible = $state(true);
	let idleTimer: ReturnType<typeof setTimeout> | undefined;

	function wake() {
		controlsVisible = true;
		clearTimeout(idleTimer);
		idleTimer = setTimeout(() => (controlsVisible = false), 2000);
	}

	function step(delta: number) {
		if (viewerIndex < 0 || screenshots.length < 2) return;
		const next =
			(viewerIndex + delta + screenshots.length) % screenshots.length;
		selectedImage = screenshots[next];
		resetViewerTransform();
		wake();
	}

	$effect(() => {
		if (selectedImage) wake();
		else clearTimeout(idleTimer);
	});

	function toggleSelected(path: string) {
		if (selected.has(path)) selected.delete(path);
		else selected.add(path);
	}

	function requestDelete(path: string) {
		deleteTarget = path;
		deleteModalOpen = true;
	}

	async function deletePaths(paths: string[]) {
		if (deleting) return;
		const id = instance.uuid;
		const token = request;
		const valid = paths.filter((path) => path.split(/[\\/]/).pop());
		if (!valid.length) return;
		deleting = valid[0];
		try {
			for (const path of valid) {
				if (!alive || token !== request) return;
				const filename = path.split(/[\\/]/).pop();
				if (filename) await deleteInstanceFile(id, "screenshots", filename, true);
			}
			if (!alive || token !== request) return;
			if (valid.includes(selectedImage ?? "")) selectedImage = null;
			selected.clear();
			selectionMode = false;
			deleteTarget = null;
			deleteModalOpen = false;
			bulkDeleteModal = false;
			deleting = null;
			await loadScreenshots();
		} catch (err) {
			if (alive && token === request) {
				error = String(err);
				deleting = null;
				await loadScreenshots();
			}
		}
	}

	const MIN_ZOOM = 1;
	const MAX_ZOOM = 6;
	const DOUBLE_TAP_ZOOM = 2.5;
	let animTimer: ReturnType<typeof setTimeout> | undefined;

	/** Ограничивает сдвиг, чтобы картинка не уезжала за края экрана. */
	function clampPan() {
		if (!imgEl) return;
		const maxX = Math.max(0, (imgEl.offsetWidth * zoom - window.innerWidth) / 2);
		const maxY = Math.max(0, (imgEl.offsetHeight * zoom - window.innerHeight) / 2);
		panX = Math.min(maxX, Math.max(-maxX, panX));
		panY = Math.min(maxY, Math.max(-maxY, panY));
	}

	/** Зум вокруг точки на экране: эта точка картинки остаётся под пальцем/курсором. */
	function zoomAt(nextZoom: number, clientX: number, clientY: number) {
		if (!imgEl) return;
		const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, nextZoom));
		if (next === zoom) return;
		if (next === MIN_ZOOM) {
			zoom = 1;
			panX = 0;
			panY = 0;
		} else {
			const rect = imgEl.getBoundingClientRect();
			const cx = rect.left + rect.width / 2;
			const cy = rect.top + rect.height / 2;
			const k = next / zoom;
			// Центр картинки без сдвига = текущий центр минус текущий сдвиг.
			panX = clientX - (cx - panX) - k * (clientX - cx);
			panY = clientY - (cy - panY) - k * (clientY - cy);
			zoom = next;
			clampPan();
		}
		wake();
	}

	/** Плавный зум (двойной тап). */
	function animateZoom(nextZoom: number, clientX: number, clientY: number) {
		animating = true;
		zoomAt(nextZoom, clientX, clientY);
		clearTimeout(animTimer);
		animTimer = setTimeout(() => (animating = false), 260);
	}

	/** Колесо мыши и тачпад (щипок = ctrl + колесо). Слушатель не пассивный, чтобы не скроллить страницу. */
	function wheelZoom(node: HTMLElement) {
		const onWheel = (event: WheelEvent) => handleWheel(event);
		node.addEventListener("wheel", onWheel, { passive: false });
		return { destroy: () => node.removeEventListener("wheel", onWheel) };
	}

	function handleWheel(event: WheelEvent) {
		event.preventDefault();
		const factor = event.ctrlKey
			? Math.exp(-event.deltaY * 0.01)
			: event.deltaY < 0
				? 1.15
				: 1 / 1.15;
		zoomAt(zoom * factor, event.clientX, event.clientY);
	}

	// Указатели мыши и пальцев: один палец = тап / перетаскивание / свайп, два = щипок.
	const pointers = new SvelteMap<number, { x: number; y: number }>();
	let pinchDist = 0;
	let gesture: { x: number; y: number; time: number; moved: boolean } | null = null;
	let swipeDx = 0;
	let lastTap: { x: number; y: number; time: number } | null = null;

	function resetGestures() {
		pointers.clear();
		gesture = null;
		pinchDist = 0;
		swipeDx = 0;
		lastTap = null;
	}

	function distance(a: { x: number; y: number }, b: { x: number; y: number }) {
		return Math.hypot(a.x - b.x, a.y - b.y);
	}

	function handlePointerDown(event: PointerEvent) {
		if (event.pointerType === "mouse" && event.button !== 0) return;
		const target = event.currentTarget as HTMLElement;
		target.setPointerCapture(event.pointerId);
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		animating = false;
		if (pointers.size === 1) {
			gesture = { x: event.clientX, y: event.clientY, time: performance.now(), moved: false };
			swipeDx = 0;
		} else if (pointers.size === 2) {
			const [a, b] = [...pointers.values()];
			pinchDist = distance(a, b);
			gesture = null;
		}
	}

	function handlePointerMove(event: PointerEvent) {
		const p = pointers.get(event.pointerId);
		if (!p) return;
		const prevX = p.x;
		const prevY = p.y;
		p.x = event.clientX;
		p.y = event.clientY;

		if (pointers.size >= 2) {
			const [a, b] = [...pointers.values()];
			const d = distance(a, b);
			if (pinchDist > 0 && d > 0) {
				zoomAt(zoom * (d / pinchDist), (a.x + b.x) / 2, (a.y + b.y) / 2);
			}
			pinchDist = d;
			return;
		}
		if (!gesture) return;
		const dx = p.x - gesture.x;
		const dy = p.y - gesture.y;
		if (Math.hypot(dx, dy) > 8) gesture.moved = true;
		if (zoom > 1) {
			panX += p.x - prevX;
			panY += p.y - prevY;
			clampPan();
		} else {
			swipeDx = dx;
		}
	}

	function handlePointerUp(event: PointerEvent) {
		const wasPinch = pointers.size >= 2;
		pointers.delete(event.pointerId);
		if (wasPinch) {
			pinchDist = 0;
			// Один палец остался: продолжаем как перетаскивание без тапа.
			const [rest] = [...pointers.values()];
			gesture = rest
				? { x: rest.x, y: rest.y, time: performance.now(), moved: true }
				: null;
			return;
		}
		if (!gesture) return;
		const g = gesture;
		gesture = null;
		const now = performance.now();
		const x = event.clientX;
		const y = event.clientY;

		if (!g.moved && now - g.time < 300) {
			if (lastTap && now - lastTap.time < 350 && distance(lastTap, { x, y }) < 30) {
				lastTap = null;
				animateZoom(zoom > 1 ? MIN_ZOOM : DOUBLE_TAP_ZOOM, x, y);
			} else {
				lastTap = { x, y, time: now };
			}
			swipeDx = 0;
			return;
		}
		lastTap = null;
		if (zoom <= 1 && Math.abs(swipeDx) > 60) {
			step(swipeDx < 0 ? 1 : -1);
		}
		swipeDx = 0;
	}

	function resetViewerTransform() {
		zoom = 1;
		panX = 0;
		panY = 0;
		animating = false;
		resetGestures();
	}

	function handlePointerCancel(event: PointerEvent) {
		pointers.delete(event.pointerId);
		if (pointers.size === 0) resetGestures();
	}

</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === "Escape" && selectedImage && !event.defaultPrevented) {
			event.preventDefault();
			if (!event.repeat) selectedImage = null;
		}
		else if (selectedImage && event.key === "ArrowLeft") {
			event.preventDefault();
			step(-1);
		} else if (selectedImage && event.key === "ArrowRight") {
			event.preventDefault();
			step(1);
		}
	}}
/>

<div class="screenshots-section">
	<div class="section-header">
		<span class="section-title"
			>{t("instanceView.screenshots.title")} ({screenshots.length})</span
		>
		<div class="selection-actions">
			<button type="button" class="folder-link" onclick={() => {
				selectionMode = !selectionMode;
				selected.clear();
			}} disabled={screenshots.length === 0}>
				{selectionMode ? t("instanceView.screenshots.cancelSelect") : t("instanceView.screenshots.select")}
			</button>
			{#if selectionMode}
				<span class="selection-count">{selected.size}</span>
				<button type="button" class="delete-btn bulk-delete" disabled={selected.size === 0 || deleting !== null} onclick={() => bulkDeleteModal = true}>
					<Icon name="ui:trash" size={16} /> {t("instanceView.deleteSelected")}
				</button>
			{/if}
		</div>
		<button
			type="button"
			class="folder-link"
			title={t("instanceView.options.folder")}
			onclick={() => void openInstanceDir(instance.uuid, "screenshots")}
		>
			<span>{t("instanceView.options.folder")} screenshots</span>
			<Icon name="instance:external-link" size={13} />
		</button>
	</div>

	{#if error}<p role="alert">
			{error}
			<button type="button" onclick={() => loadScreenshots()}
				>{t("common.retry")}</button
			>
		</p>{/if}
	<div
		class="screenshots-grid"
		bind:this={grid}
		tabindex="-1"
		onscroll={handleScroll}
		style:--screenshot-columns={columns}
	>
		{#if loading && screenshots.length === 0}
			<div class="screenshots-skeleton sk-delayed" aria-hidden="true">
				{#each { length: columns * 3 } as _, index (index)}
					<span class="sk-block screenshots-skeleton-tile"></span>
				{/each}
			</div>
			<span class="sk-sr-only" role="status">{t("common.loading")}</span>
		{/if}
		<div
			class="screenshot-space"
			style:height={`${Math.max(0, rowCount * rowHeight - metrics.gap)}px`}
		>
			{#each rows as row (row)}
				<div
					class="screenshot-row"
					style:transform={`translateY(${row * rowHeight}px)`}
					style:height={`${rowHeight - metrics.gap}px`}
				>
					{#each screenshots.slice(row * columns, (row + 1) * columns) as path, offset (path)}
						{@const index = row * columns + offset}
						{@const preview = thumbnails.get(path)}
						<div class="screenshot-card" class:selecting={selectionMode} class:selected={selected.has(path)} data-index={index}>
							<button
								type="button"
								class="open-screenshot"
								aria-pressed={selectionMode ? selected.has(path) : undefined}
								onclick={() => { if (selectionMode) toggleSelected(path); else { selectedImage = path; resetViewerTransform(); } }}
								onkeydown={(event) => navigate(event, index)}
								aria-label={path.split(/[\\/]/).pop()}
							>
								{#if preview}<img
										src={preview}
										alt=""
										decoding="async"
									/>{:else}<Icon
										name="instance:image"
										size={24}
									/>{/if}
							</button>
							{#if shotDate(path)}
								<span class="shot-date">{shotDate(path)}</span>
							{/if}
							<div class="overlay">
								{#if selectionMode}
									<span class="pick" class:on={selected.has(path)} aria-hidden="true">
										{#if selected.has(path)}<Icon name="ui:check" size={14} />{/if}
									</span>
								{:else}
									<button type="button" class="delete-btn" disabled={deleting !== null || loading} onclick={(e) => { e.stopPropagation(); requestDelete(path); }} title={t("common.delete")} aria-label={t("common.delete")}>
										<Icon name="ui:trash" size={16} />
									</button>
								{/if}
							</div>
												</div>
					{/each}
				</div>
			{/each}
		</div>

		{#if !loading && !error && screenshots.length === 0}
			<div class="empty-state">
				{t("instanceView.screenshots.empty")}
			</div>
		{/if}
		<ScrollToTopButton scroller={grid} />
	</div>
</div>

{#if selectedImage}
	<div
		class="image-viewer-overlay"
		transition:fade={{ duration: animDuration(250) }}
		role="button"
		tabindex="0"
		onclick={() => (selectedImage = null)}
		onkeydown={(e) => {
			if ((e.key === "Enter" || e.key === " ") && e.target === e.currentTarget) {
				e.preventDefault();
				selectedImage = null;
			}
		}}
	>
		<div
			class="viewer-container"
			in:windowSlide={{ viewport: true }}
			out:windowSlide={{ viewport: true }}
			class:idle={!controlsVisible}
			onmousemove={wake}
			use:wheelZoom
			role="dialog"
			aria-modal="true"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
			onkeydown={(e) => {
				if (e.key === "Escape" && !e.defaultPrevented) {
					e.preventDefault();
					if (!e.repeat) selectedImage = null;
				}
				e.stopPropagation();
			}}
		>
			<img
				bind:this={imgEl}
				class:zoomed={zoom > 1}
				class:animating
				style:transform={`translate3d(${panX}px, ${panY}px, 0) scale(${zoom})`}
				src={convertFileSrc(selectedImage)}
				alt={t("instanceView.screenshots.fullSize")}
				draggable="false"
				decoding="async"
				onpointerdown={handlePointerDown}
				onpointermove={handlePointerMove}
				onpointerup={handlePointerUp}
				onpointercancel={handlePointerCancel}
			/>
			{#if shotDate(selectedImage)}
				<span class="viewer-date">{shotDate(selectedImage)}</span>
			{/if}
			{#if screenshots.length > 1}
				<button
					type="button"
					class="viewer-nav viewer-prev"
					aria-label={t("common.previous")}
					onclick={() => step(-1)}
				>
					<svg viewBox="0 0 24 24" aria-hidden="true"
						><path d="M15 5l-7 7 7 7" /></svg
					>
				</button>
				<button
					type="button"
					class="viewer-nav viewer-next"
					aria-label={t("common.next")}
					onclick={() => step(1)}
				>
					<svg viewBox="0 0 24 24" aria-hidden="true"
						><path d="M9 5l7 7-7 7" /></svg
					>
				</button>
			{/if}
			<button
				type="button"
				class="close-btn"
				onclick={() => (selectedImage = null)}>✕</button
			>
		</div>
	</div>
{/if}


<ModalBase bind:open={deleteModalOpen} title={t("sidebar.modals.deleteTitle")} onclose={() => { deleteTarget = null; deleteModalOpen = false; }}>
	<p class="confirmation-message">{t("instanceView.screenshots.confirmDelete")}</p>
	{#snippet footer()}
		<button type="button" class="btn-secondary" onclick={() => { deleteTarget = null; deleteModalOpen = false; }}>{t("sidebar.modals.cancel")}</button>
		<button type="button" class="btn-primary" style="background: var(--color-error); color: var(--color-on-error);" onclick={() => deleteTarget && void deletePaths([deleteTarget])}>{t("sidebar.modals.deleteBtn")}</button>
	{/snippet}
</ModalBase>
<ModalBase bind:open={bulkDeleteModal} title={t("sidebar.modals.deleteTitle")}>
	<p class="confirmation-message">{t("instanceView.deleteSelectedConfirm", { count: selected.size })}</p>
	{#snippet footer()}
		<button type="button" class="btn-secondary" onclick={() => bulkDeleteModal = false}>{t("sidebar.modals.cancel")}</button>
		<button type="button" class="btn-primary" style="background: var(--color-error); color: var(--color-on-error);" onclick={() => void deletePaths([...selected])}>{t("sidebar.modals.deleteBtn")}</button>
	{/snippet}
</ModalBase>

<style>
	.screenshots-section {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		flex: 1;
		min-height: 0;
		min-width: 0;
	}

	.section-title {
		font-size: 1.2rem;
		font-weight: 600;
		color: var(--text-primary);
	}

	.screenshots-grid {
		--screenshot-min-width: 200px;
		--screenshot-gap: 1rem;
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		overflow-anchor: none;
		scrollbar-gutter: stable;
		position: relative;
	}

	.screenshots-skeleton {
		display: grid;
		grid-template-columns: repeat(
			auto-fill,
			minmax(var(--screenshot-min-width), 1fr)
		);
		gap: var(--screenshot-gap);
		padding-bottom: var(--screenshot-gap);
	}

	.screenshots-skeleton-tile {
		aspect-ratio: 16 / 9;
	}
	.screenshot-space {
		position: relative;
		width: 100%;
	}
	.screenshot-row {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		display: grid;
		grid-template-columns: repeat(
			var(--screenshot-columns),
			minmax(0, 1fr)
		);
		gap: var(--screenshot-gap);
	}
	.open-screenshot {
		width: 100%;
		height: 100%;
		border: 0;
		padding: 0;
		background: transparent;
		color: var(--text-secondary);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.screenshot-card {
		position: relative;
		min-width: 0;
		height: 100%;
		background: rgba(var(--surface-rgb), 0.05);
		border-radius: var(--border-radius-sm);
		overflow: hidden;
		cursor: pointer;
		border: 2px solid transparent;
		transition:
			transform 0.2s,
			border-color 0.2s;
	}

	.screenshot-card:not(.selecting):hover {
		transform: scale(1.02);
	}

	.screenshot-card.selected {
		box-shadow: inset 0 0 0 3px var(--accent-primary);
	}

	.screenshot-card img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.overlay {
		position: absolute;
		top: 0.5rem;
		right: 0.5rem;
		display: flex;
		gap: 6px;
		opacity: 0;
		transition: opacity 0.2s;
		pointer-events: none;
	}

	.screenshot-card:hover .overlay,
	.screenshot-card:focus-within .overlay,
	.screenshot-card.selecting .overlay {
		opacity: 1;
	}

	/* Удаление: сплошная кнопка из палитры темы, одинаково видна в светлой и тёмной. */
	.delete-btn {
		pointer-events: auto;
		width: 28px;
		height: 28px;
		padding: 0;
		border: none;
		border-radius: 50%;
		background: var(--color-error);
		color: var(--color-on-error);
		box-shadow: var(--shadow-indicator);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: filter 0.15s, opacity 0.15s;
	}

	.delete-btn:hover:not(:disabled) {
		filter: brightness(1.1);
	}

	.delete-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}

	/* Кружок выбора в углу карточки: пустой кольцо → синий с галочкой. */
	.pick {
		box-sizing: border-box;
		width: 24px;
		height: 24px;
		border-radius: 50%;
		border: 2px solid var(--media-overlay-text);
		background: var(--media-overlay);
		color: var(--accent-text);
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.15s, border-color 0.15s;
	}

	.pick.on {
		background: var(--accent-primary);
		border-color: var(--accent-primary);
	}

	.empty-state {
		grid-column: 1 / -1;
		text-align: center;
		padding: 3rem;
		color: var(--text-secondary);
		background: rgba(var(--surface-rgb), 0.02);
		border-radius: var(--border-radius-sm);
		border: 1px dashed rgba(var(--surface-rgb), 0.1);
	}

	.image-viewer-overlay {
		position: fixed;
		top: 0;
		left: 0;
		width: 100vw;
		height: 100vh;
		background: var(--viewer-overlay);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 2000;
		backdrop-filter: blur(var(--backdrop-blur-viewer, 2px));
	}

	.viewer-container {
		position: relative;
		max-width: 90%;
		max-height: 90%;
	}

	.viewer-container img {
		max-width: 90vw;
		max-height: 90vh;
		border-radius: var(--border-radius-sm);
		box-shadow: var(--shadow-floating);
		transform-origin: center;
		will-change: transform;
		user-select: none;
		-webkit-user-drag: none;
		touch-action: none;
		cursor: zoom-in;
	}
	/* Плавность только для анимированного зума (двойной тап); при перетаскивании без задержки. */
	.viewer-container img.animating {
		transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
	}
	.viewer-container img.zoomed {
		cursor: grab;
	}
	.viewer-container img.zoomed:active {
		cursor: grabbing;
	}
	.selection-actions { display: flex; align-items: center; gap: 8px; margin-left: auto; }
	.selection-count { color: var(--text-secondary); font-size: .85rem; font-variant-numeric: tabular-nums; }
	.delete-btn.bulk-delete { width: auto; height: auto; border-radius: var(--border-radius-sm); padding: 5px 10px; gap: 6px; font-size: 0.8rem; font-weight: 600; }

	.viewer-container {
		transition: cursor 0s;
	}

	.viewer-container.idle {
		cursor: none;
	}

	.viewer-date,
	.viewer-nav {
		transition: opacity 0.25s ease;
	}

	.viewer-container.idle .viewer-date,
	.viewer-container.idle .viewer-nav,

	.viewer-date {
		position: absolute;
		left: 12px;
		bottom: 12px;
		padding: 4px 10px;
		border-radius: var(--border-radius-sm);
		background: var(--media-overlay);
		color: var(--media-overlay-text);
		font-size: 0.8rem;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}

	.viewer-nav {
		position: absolute;
		top: 50%;
		width: 40px;
		height: 40px;
		margin-top: -20px;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		border: none;
		border-radius: 50%;
		background: var(--media-overlay);
		color: var(--media-overlay-text);
		cursor: pointer;
	}

	.viewer-nav svg {
		width: 22px;
		height: 22px;
		fill: none;
		stroke: currentColor;
		stroke-width: 2.4;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.viewer-prev {
		left: 12px;
	}

	.viewer-next {
		right: 12px;
	}

	.section-header {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.folder-link {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 3px 8px;
		background: transparent;
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		color: var(--text-muted);
		font-family: inherit;
		font-size: 0.7rem;
		font-weight: 600;
		cursor: pointer;
		transition:
			color 0.15s,
			border-color 0.15s;
	}

	.folder-link:hover {
		color: var(--text-primary);
		border-color: var(--text-muted);
	}

	.shot-date {
		position: absolute;
		top: 0.5rem;
		left: 0.5rem;
		padding: 2px 7px;
		border-radius: var(--border-radius-sm);
		background: var(--media-overlay);
		color: var(--media-overlay-text);
		font-size: 0.68rem;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		pointer-events: none;
	}

	.close-btn {
		position: absolute;
		top: -40px;
		right: -40px;
		background: transparent;
		border: none;
		color: var(--media-overlay-text);
		font-size: 2rem;
		cursor: pointer;
	}

	@media (max-width: 700px) {
		.screenshots-grid {
			--screenshot-min-width: 160px;
		}
	}

	@media (max-width: 550px) {
		.screenshots-grid {
			--screenshot-min-width: 130px;
			--screenshot-gap: 8px;
		}
	}

	@media (max-width: 400px) {
		.screenshots-grid {
			--screenshot-min-width: 100px;
			--screenshot-gap: 6px;
		}
	}
</style>
