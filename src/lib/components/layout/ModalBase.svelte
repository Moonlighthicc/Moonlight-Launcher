<script module lang="ts">
	// Открытые окна в порядке открытия: Escape закрывает только самое верхнее.
	const openStack: symbol[] = [];
	export function hasOpenModal() {
		return openStack.length > 0;
	}
</script>

<script lang="ts">
	import { t } from "$lib/i18n";
	import { fade } from "svelte/transition";
	import type { Snippet } from "svelte";
	import CloseIcon from "$lib/icons/CloseIcon.svelte";
	import ScrollToTopButton from "$lib/components/ui/ScrollToTopButton.svelte";
	import { animDuration, windowSlide } from "$lib/utils/animations";
	import { animateHeight } from "$lib/utils/animateHeight";
	import { createMountReady } from "$lib/utils/mountReady.svelte";

	let {
		open = $bindable(),
		title,
		width,
		animateResize = false,
		closeDisabled = false,
		scrollBody = false,
		showScrollToTop = false,
		onclose,
		onclosed,
		onescape,
		focusOnOpen = false,
		children,
		footer,
		headerActions,
	}: {
		open: boolean;
		title?: string;
		width?: string;
		animateResize?: boolean;
		closeDisabled?: boolean;
		scrollBody?: boolean;
		showScrollToTop?: boolean;
		onclose?: () => void;
		onclosed?: () => void;
		onescape?: () => boolean;
		focusOnOpen?: boolean;
		children?: Snippet;
		footer?: Snippet;
		headerActions?: Snippet;
	} = $props();
	let modalScroll = $state<HTMLDivElement>();

	function close() {
		if (closeDisabled) return;
		open = false;
		onclose?.();
	}

	const mount = createMountReady();

	const modalId = Symbol("modal");
	$effect(() => {
		if (!open) return;
		openStack.push(modalId);
		return () => {
			const i = openStack.indexOf(modalId);
			if (i !== -1) openStack.splice(i, 1);
		};
	});

	function onWindowKeydown(e: KeyboardEvent) {
		if (e.key !== "Escape" || e.defaultPrevented || !open) return;
		if (openStack[openStack.length - 1] !== modalId) return;
		e.preventDefault();
		if (e.repeat || closeDisabled || onescape?.()) return;
		close();
	}

	function focusDialog(dialog: HTMLElement) {
		if (!focusOnOpen) return;
		queueMicrotask(() => {
			if (!open || !dialog.isConnected) return;
			const target = dialog.querySelector<HTMLElement>(".modal-body input, .modal-body button:not(:disabled)");
			(target ?? dialog).focus({ preventScroll: true });
		});
	}

	function portal(overlay: HTMLElement) {
		document.body.appendChild(overlay);
		return { destroy: () => overlay.remove() };
	}

	const fadeDuration = $derived(animDuration(250));
	const resizeDuration = $derived(animateResize ? animDuration(300) : 0);
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if open && mount.ready}
	<div
		class="modal-overlay"
		use:portal
		onclick={close}
		onkeydown={onWindowKeydown}
		role="presentation"
		transition:fade|global={{ duration: fadeDuration }}
	>
		<div
			class="modal"
			bind:this={modalScroll}
			class:animate-resize={animateResize}
			class:scroll-body={scrollBody}
			use:animateHeight={resizeDuration}
			use:focusDialog
			style:--resize-duration={`${resizeDuration}ms`}
			onclick={(e) => e.stopPropagation()}
			onkeydown={(e) => {
				if (e.key === "Escape") onWindowKeydown(e);
				e.stopPropagation();
			}}
			role="dialog"
			aria-modal="true"
			tabindex="-1"
			style:--modal-default-width={width ?? "400px"}
			in:windowSlide|global={{ viewport: true }}
			out:windowSlide|global={{ viewport: true }}
			onoutroend={() => { if (!open) onclosed?.(); }}
		>
			<div class="modal-content">
				<div class="modal-header">
					{#if title}
						<span class="modal-title">{title}</span>
					{/if}
					<div class="modal-header-actions">
						{#if headerActions}{@render headerActions()}{/if}
						<button
							type="button"
							class="action-btn"
							onclick={close}
							disabled={closeDisabled}
							aria-label={t("common.close")}
						>
							<CloseIcon size={20} />
						</button>
					</div>
				</div>

				<div class="modal-body">
					{@render children?.()}
				</div>

				{#if footer}
					<div class="modal-footer">
						{@render footer()}
					</div>
				{/if}
			</div>
			{#if showScrollToTop && !scrollBody}
				<ScrollToTopButton scroller={modalScroll} />
			{/if}
		</div>
	</div>
{/if}

<style>
	.modal-overlay {
		position: fixed;
		inset: 0;
		background: var(--bg-overlay, rgba(0, 0, 0, 0.75));
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
		backdrop-filter: blur(var(--backdrop-blur-modal, 4px));
	}

	.modal {
		box-sizing: border-box;
		background: var(--bg-sidebar);
		border: var(--border-width) solid var(--border);
		border-radius: var(--border-radius-lg);
		width: min(var(--modal-width, var(--modal-default-width)), 90vw);
		max-height: var(--modal-max-height, 90vh);
		overflow-y: auto;
		box-shadow: var(--shadow-lg, 0 20px 40px rgba(0, 0, 0, 0.4));
		transition: width var(--resize-duration)
			cubic-bezier(0.25, 0.8, 0.25, 1);
		/* Thinner, less intrusive scrollbars inside modals (inherited by
		   every scroll region within the modal). */
		--scrollbar-size: 4px;
		--scrollbar-track: transparent;
	}

	.modal.animate-resize {
		scrollbar-gutter: stable;
	}

	.modal-content {
		padding: var(--modal-padding, var(--space-xl));
		display: flex;
		flex-direction: column;
		gap: var(--modal-gap, 20px);
	}

	.modal.scroll-body {
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.scroll-body .modal-content {
		min-height: 0;
		max-height: inherit;
		box-sizing: border-box;
	}
	.animate-resize.scroll-body .modal-content {
		/* Measure the content at its natural height (up to the viewport limit),
		   not at the outer modal's current animated height. */
		flex-shrink: 0;
	}
	.scroll-body .modal-header,
	.scroll-body .modal-footer {
		flex-shrink: 0;
	}
	.scroll-body .modal-header {
		gap: 8px;
		flex-wrap: wrap;
	}
	.scroll-body .modal-body {
		min-height: 0;
		overflow-y: auto;
		overscroll-behavior: contain;
		/* Keep content clear of the scrollbar so it doesn't look cramped. */
		padding-right: 8px;
	}

	@media (prefers-reduced-motion: reduce) {
		.modal {
			transition: none;
		}
	}

	:global(.modal::-webkit-scrollbar) {
		width: var(--scrollbar-size);
	}

	:global(.modal::-webkit-scrollbar-thumb) {
		background: var(--scrollbar-thumb);
		border-radius: var(--scrollbar-radius);
	}

	.modal-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.modal-header-actions {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-left: auto;
	}

	.modal-title {
		font-size: var(--modal-title-size, 1rem);
		font-weight: var(--font-weight-bold);
		color: var(--text-primary);
	}

	.modal-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--modal-footer-gap, 10px);
	}
</style>
