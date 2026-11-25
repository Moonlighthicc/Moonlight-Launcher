<script lang="ts">
	let { size = 24 }: { size?: number } = $props();
</script>

<span class="loading-ring" style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true"></span>

<style>
	.loading-ring {
		display: inline-block;
		box-sizing: border-box;
		border: 2px solid var(--border);
		border-top-color: var(--accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		flex-shrink: 0;
	}
	@keyframes spin {
		to { transform: rotate(1turn); }
	}
	@media (prefers-reduced-motion: reduce) {
		.loading-ring { animation: none; }
	}
	:global(html[data-reduce-motion]) .loading-ring { animation: none; }
</style>
