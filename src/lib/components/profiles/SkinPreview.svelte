<script lang="ts">
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import Skin3dViewer from "./Skin3dViewer.svelte";
	import { launcherStore } from "$lib/state/state.svelte";

	interface Props {
		skinUrl: string;
		capeUrl: string | null;
		model: "default" | "slim";
		onOpenWardrobe: (trigger: HTMLButtonElement) => void;
		onSaveLocal?: () => void;
		savingLocal?: boolean;
		emptyText?: string;
		draggingPng?: boolean;
		dropTargetActive?: boolean;
		onDragEnter?: (e: DragEvent) => void;
		onDragLeave?: (e: DragEvent) => void;
		onDragOver?: (e: DragEvent) => void;
		onDrop?: (e: DragEvent) => void;
	}

	let {
		skinUrl,
		capeUrl,
		model,
		onOpenWardrobe,
		onSaveLocal,
		savingLocal = false,
		emptyText,
		draggingPng = false,
		dropTargetActive = false,
		onDragEnter,
		onDragLeave,
		onDragOver,
		onDrop,
	}: Props = $props();

	// Girar y mover los ojos, a gusto de quien mira: se recuerda entre aperturas.
	const MOTION_KEY = "skin-preview-motion";
	function storedMotion(): { rotate: boolean; eyes: boolean } {
		try {
			const kept = JSON.parse(localStorage.getItem(MOTION_KEY) ?? "{}");
			return {
				rotate: kept.rotate !== false,
				eyes: kept.eyes !== false,
			};
		} catch {
			return { rotate: true, eyes: true };
		}
	}
	const motion = $state(storedMotion());
	/** Si la skin lleva ojos de Fresh Moves: solo entonces hay botón de ojos. */
	let hasEyes = $state(false);
	function toggle(which: "rotate" | "eyes") {
		motion[which] = !motion[which];
		try {
			localStorage.setItem(MOTION_KEY, JSON.stringify(motion));
		} catch {
			// Sin almacenamiento la elección dura lo que la ventana.
		}
	}
	// Con las animaciones 3D desactivadas en Ajustes no hay nada que encender.
	const motionAllowed = $derived(
		!launcherStore.settings.disable_skin3d_animations,
	);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (optional drag-and-drop augments the separate keyboard-accessible upload button) -->
<div
	class="preview-zone"
	class:drop-ready={draggingPng && dropTargetActive}
	ondragenter={onDragEnter}
	ondragleave={onDragLeave}
	ondragover={onDragOver}
	ondrop={onDrop}
>
	{#if skinUrl}
		<Skin3dViewer
			{skinUrl}
			{capeUrl}
			{model}
			quality={launcherStore.settings.reduce_skin3d_quality
				? "low"
				: "high"}
			rotate={motion.rotate}
			eyes={motion.eyes}
			onEyesFound={(found) => (hasEyes = found)}
		/>
	{/if}

	<div class="motion-controls">
		{#if skinUrl && motionAllowed}
			<button
				type="button"
				class="motion-btn"
				class:active={motion.rotate}
				onclick={() => toggle("rotate")}
				title={t(
					motion.rotate
						? "userMenu.skinCape.stopRotation"
						: "userMenu.skinCape.startRotation",
				)}
				aria-label={t(
					motion.rotate
						? "userMenu.skinCape.stopRotation"
						: "userMenu.skinCape.startRotation",
				)}
			>
				<Icon name="ui:refresh" size={14} />
			</button>
		{/if}
		<button
			type="button"
			class="motion-btn"
			aria-haspopup="dialog"
			onclick={(e) => onOpenWardrobe(e.currentTarget)}
			title={t("userMenu.skinCape.skinCloset.title")}
			aria-label={t("userMenu.skinCape.skinCloset.title")}
		>
			<Icon name="ui:resources" size={14} />
		</button>
		{#if onSaveLocal}
			<button
				type="button"
				class="motion-btn"
				onclick={onSaveLocal}
				disabled={savingLocal}
				title={t("userMenu.skinCape.skinCloset.copyLocal")}
				aria-label={t("userMenu.skinCape.skinCloset.copyLocal")}
			>
				<Icon name="ui:download" size={14} />
			</button>
		{/if}
		{#if skinUrl && motionAllowed && hasEyes}
			<!-- En el juego los ojos solo se mueven con el paquete: lo dice la burbuja. -->
			<button
				type="button"
				class="motion-btn"
				class:active={motion.eyes}
				aria-pressed={motion.eyes}
				onclick={() => toggle("eyes")}
				title={t("userMenu.skinCape.eyesMove")}
				aria-label={t("userMenu.skinCape.eyesMove")}
			>
				<Icon name="ui:eye" size={14} />
			</button>
		{/if}
	</div>

	{#if skinUrl}
		{#if draggingPng && dropTargetActive}
			<div class="drop-overlay">
				<Icon name="ui:upload" size={32} />
				<span>{t("userMenu.skinCape.dropSkinHere")}</span>
			</div>
		{/if}
	{:else}
		<div class="empty-preview">
			<span role="status"
				>{emptyText ?? t("userMenu.skinCape.noActiveSkin")}</span
			>
		</div>
	{/if}
</div>

<style>
	.preview-zone {
		position: relative;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		overflow: hidden;
		min-height: 260px;
		height: 300px;
		transition:
			border-color 0.15s ease,
			background 0.15s ease;
	}

	.preview-zone.drop-ready {
		border-color: var(--accent);
		background: rgba(var(--accent-rgb), 0.04);
	}

	.drop-overlay {
		position: absolute;
		inset: 0;
		z-index: 4;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		background: var(--bg-overlay);
		backdrop-filter: blur(var(--backdrop-blur-modal));
		color: var(--text-primary);
		font-size: 0.85rem;
		font-weight: 600;
	}

	.motion-controls {
		position: absolute;
		top: 8px;
		left: 8px;
		z-index: 3;
		display: flex;
		gap: 6px;
	}

	.motion-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		background: var(--media-overlay);
		border: var(--border-width) solid var(--media-overlay-border);
		color: var(--media-overlay-text);
		border-radius: var(--border-radius-sm);
		cursor: pointer;
		transition:
			background 0.15s ease,
			color 0.15s ease;
	}

	.motion-btn:hover:not(:disabled) {
		background: var(--surface-hover);
		color: var(--text-primary);
	}

	.motion-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.motion-btn.active {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--accent-text);
	}

	.empty-preview {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
</style>
