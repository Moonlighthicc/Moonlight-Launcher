<script lang="ts">
	import { untrack } from "svelte";
	import { fade } from "svelte/transition";
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import { animDuration, shouldReduceAnimations } from "$lib/utils/animations";

	let { scroller }: { scroller?: HTMLElement } = $props();
	let visible = $state(false);
	let button = $state<HTMLButtonElement>();

	$effect(() => {
		const target = scroller;
		if (!target) {
			visible = false;
			return;
		}
		const syncVisibility = () => {
			const next = target.scrollTop > 160;
			if (!next && button && document.activeElement === button) {
				target.focus({ preventScroll: true });
			}
			visible = next;
		};
		untrack(syncVisibility);
		target.addEventListener("scroll", syncVisibility, { passive: true });
		return () => target.removeEventListener("scroll", syncVisibility);
	});

	function scrollToTop() {
		scroller?.scrollTo({ top: 0, behavior: shouldReduceAnimations() ? "instant" : "smooth" });
	}
</script>

{#if visible}
	<div class="sticky-control" transition:fade={{ duration: animDuration(220) }}>
		<button type="button" class="btn-secondary" bind:this={button} onclick={scrollToTop}>
			<Icon name="log:chevron-up" size={16} />
			{t("common.scrollToTop")}
		</button>
	</div>
{/if}

<style>
	.sticky-control {
		position: sticky;
		inset-block-end: 0;
		z-index: 5;
		block-size: 0;
		pointer-events: none;
	}
	button {
		position: absolute;
		inset-block-end: 12px;
		left: 50%;
		transform: translateX(-50%);
		white-space: nowrap;
		display: inline-flex;
		align-items: center;
		gap: 5px;
		background: var(--bg-sidebar);
		color: var(--text-primary);
		box-shadow: var(--shadow-floating);
		pointer-events: auto;
	}
</style>