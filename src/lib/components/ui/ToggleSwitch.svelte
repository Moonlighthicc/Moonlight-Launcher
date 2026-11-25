<script lang="ts">
	import type { HTMLInputAttributes } from "svelte/elements";

	let {
		checked = $bindable(),
		...inputProps
	}: Pick<HTMLInputAttributes, "id" | "disabled" | "aria-describedby" | "onchange" | "checked"> = $props();
</script>

<span class="switch">
	<input {...inputProps} type="checkbox" role="switch" bind:checked />
	<span class="track" aria-hidden="true"><span class="thumb"></span></span>
</span>

<style>
	.switch {
		position: relative;
		display: inline-flex;
		flex: 0 0 36px;
		width: 36px;
		height: 20px;
		vertical-align: middle;
	}

	.switch input {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}

	.track {
		display: flex;
		flex: 1;
		align-items: center;
		box-sizing: border-box;
		padding: 2px;
		border: 1px solid var(--border-color);
		border-radius: 999px;
		background: var(--bg-input);
		pointer-events: none;
		transition: background-color 160ms ease, border-color 160ms ease;
	}

	.thumb {
		flex: 0 0 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--text-secondary);
		transition: transform 180ms ease, background-color 160ms ease;
	}

	input:hover:not(:disabled) + .track { border-color: var(--text-muted); }
	input:checked + .track { background: var(--accent); border-color: var(--accent); }
	input:checked + .track .thumb { transform: translateX(16px); background: var(--accent-text); }
	input:focus-visible + .track { outline: 2px solid var(--focus-ring, var(--accent)); outline-offset: 2px; }
	input:disabled { cursor: not-allowed; }
	input:disabled + .track { opacity: 0.5; }

	@media (prefers-reduced-motion: reduce) {
		.track, .thumb { transition: none; }
	}
</style>