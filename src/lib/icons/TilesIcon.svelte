<script lang="ts">
	let {
		class: className = "",
		size = 16,
	}: {
		class?: string;
		size?: number;
	} = $props();
</script>

<svg
	class={className}
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.8"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	<rect x="3" y="4" width="18" height="16" rx="2.5" />
	<path d="M3 9h18" />
	<path d="M10 9v11" />
</svg>
