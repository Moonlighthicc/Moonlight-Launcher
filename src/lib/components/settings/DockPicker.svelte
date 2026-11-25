<script lang="ts" generics="T extends string">
	/**
	 * Визуальный выбор края/зоны окна: сетка, в которой каждая зона стоит на своём
	 * месте (как в выборе положения текста у «Стильного»). Подписи — только в
	 * `title` и для программ, поэтому длинные названия не вылезают из ячейки.
	 */
	type Option = {
		value: T;
		label: string;
		/** CSS grid-area: "row-start / col-start / row-end / col-end". */
		area: string;
	};

	let {
		id,
		label,
		value,
		options,
		columns,
		rows,
		onchange,
	}: {
		id: string;
		label: string;
		value: T;
		options: Option[];
		columns: number;
		rows: number;
		onchange: (value: T) => void;
	} = $props();
</script>

<div
	class="dock-picker"
	role="group"
	aria-label={label}
	{id}
	style:grid-template-columns="repeat({columns}, 1fr)"
	style:grid-template-rows="repeat({rows}, 1fr)"
>
	{#each options as option (option.value)}
		<button
			type="button"
			class="dock-cell"
			data-dock={option.value}
			style:grid-area={option.area}
			aria-pressed={value === option.value}
			aria-label={option.label}
			title={option.label}
			onclick={() => onchange(option.value)}
		><span class="dock-bar"></span></button>
	{/each}
</div>

<style>
	.dock-picker {
		display: grid;
		gap: 4px;
		width: 100%;
		max-width: 260px;
		height: 96px;
		padding: 6px;
		box-sizing: border-box;
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		background: var(--bg-card);
	}

	.dock-cell {
		display: flex;
		padding: 0;
		border: 1px solid var(--border-color);
		border-radius: 4px;
		background: transparent;
		cursor: pointer;
		transition: border-color 0.15s, background 0.15s;
	}

	.dock-cell:hover {
		border-color: var(--text-muted);
	}

	.dock-cell[aria-pressed="true"] {
		border-color: var(--accent);
		background: color-mix(in srgb, var(--accent) 14%, transparent);
	}

	.dock-cell:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}

	/* Полоска внутри ячейки показывает, с какой стороны окна стоит панель. */
	.dock-bar {
		flex: 1;
		margin: 3px;
		border-radius: 3px;
		background: var(--border-color);
	}

	.dock-cell[aria-pressed="true"] .dock-bar {
		background: var(--accent);
	}
</style>
