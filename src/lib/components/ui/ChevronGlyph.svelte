<script lang="ts">
	/**
	 * Единая стрелка ❱ (U+2771). Везде используется один и тот же символ;
	 * направление задаётся только поворотом: 0° → вправо, 90° → вниз,
	 * 180° → влево, -90° → вверх.
	 */
	let {
		rotate = 0,
		size = 14,
		class: className = "",
	}: {
		rotate?: number;
		size?: number;
		class?: string;
	} = $props();
</script>

<span
	class="chevron-glyph {className}"
	style:font-size="{size}px"
	style:transform="rotate({rotate}deg)"
	aria-hidden="true">❱</span
>

<style>
	.chevron-glyph {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1em;
		height: 1em;
		line-height: 1;
		font-family: "Segoe UI Symbol", "DejaVu Sans", "Noto Sans Symbols 2",
			"Noto Sans Symbols", sans-serif;
		font-weight: 400;
		transition: transform 0.15s ease;
		pointer-events: none;
		user-select: none;
	}
</style>
