<script lang="ts">
	import ToggleSwitch from "$lib/components/ui/ToggleSwitch.svelte";
	import { launcherStore } from "$lib/state/state.svelte";
	import { t } from "$lib/i18n";
	import Select from "$lib/components/layout/Select.svelte";
	import SegmentedControl from "$lib/components/settings/SegmentedControl.svelte";
	import RangeReset from "$lib/components/settings/RangeReset.svelte";
	import {
		resolveTileSliderEdgeInset,
		TILE_RANGE_DEFAULTS,
		TILE_SLIDER_EDGE_INSET,
	} from "$lib/state/tileDefaults";
	import {
		appearance,
		setSliderMotion,
		setQuickActionsDisabled,
		SLIDER_MOTIONS,
		type SliderMotion,
	} from "$lib/state/appearanceState.svelte";
	import {
		panelLayout,
		setPanelOrientationLocked,
	} from "$lib/state/panelLayoutState.svelte";
	import type { TileSide, TileStyle, TileTextPos } from "$lib/types/types";
	import {
		resolveTileSide,
		resolveGlassBlur,
		sliderScalePercent,
	} from "$lib/state/tileStyle";
	import "$lib/components/settings/controls.css";

	/**
	 * Общие настройки плиток. Редактор сохраняет изменения своей кнопкой;
	 * в настройках лаунчера их сохраняет onsave.
	 */
	let { onsave = () => {} }: { onsave?: () => unknown } = $props();

	function resetRange(key: keyof typeof TILE_RANGE_DEFAULTS) {
		launcherStore.settings[key] = TILE_RANGE_DEFAULTS[key];
		onsave();
	}

	const SIZE = { min: 30, max: 100 };
	const SPEED = { min: 10, max: 200 };
	const SLIDER_ROWS = { min: 1, max: 10 };
	const SLIDER_BORDER = { min: 30, max: 100 };
	const GLASS_BLUR = { min: 0, max: 80 };
	const INTERVALS = [0, 3, 5, 10, 30];
	const SIDES: TileSide[] = ["auto", "left", "right"];
	const STYLES: TileStyle[] = ["standard", "glass", "stylish"];
	const TEXT_POSITIONS: Exclude<TileTextPos, "auto">[] = [
		"top-left",
		"top-center",
		"top-right",
		"bottom-left",
		"bottom-center",
		"bottom-right",
	];
	const tileStyle = $derived(launcherStore.settings.tile_style ?? "standard");
	const tileSide = $derived(resolveTileSide(launcherStore.settings));
	const glassBlur = $derived(resolveGlassBlur(launcherStore.settings));
	const sliderEdgeInset = $derived(
		resolveTileSliderEdgeInset(
			launcherStore.settings.tile_slider_edge_inset,
		),
	);
	const tileTextPos = $derived(
		launcherStore.settings.tile_text_pos ?? "auto",
	);

	function fill(value: number, range: { min: number; max: number }) {
		return `${((value - range.min) / (range.max - range.min)) * 100}%`;
	}

	const tileSideOptions = $derived(
		SIDES.map((value) => ({
			value,
			label: t(`settings.launcher.appearance.tileSides.${value}`),
		})),
	);
	const intervalOptions = $derived(
		INTERVALS.map((sec) => ({
			value: String(sec),
			label:
				sec === 0
					? t("settings.launcher.appearance.intervalOff")
					: t("settings.launcher.appearance.intervalSeconds", {
							n: sec,
						}),
		})),
	);
	const motionOptions = $derived(
		SLIDER_MOTIONS.map((value) => ({
			value,
			label: t(`settings.launcher.appearance.motion.${value}`),
		})),
	);
	const tileStyleOptions = $derived(
		STYLES.map((value) => ({
			value,
			label: t(`settings.launcher.appearance.tileStyles.${value}`),
		})),
	);
</script>

<div class="appearance-settings tile-settings settings-controls">
	<section
		class="ap-group"
		aria-label={t("settings.launcher.appearance.groupSliders")}
	>
		<h3 class="ap-group-title">
			{t("settings.launcher.appearance.groupSliders")}
		</h3>

		<div class="ap-field">
			<label class="ap-label" for="tile-slider-size">
				<span>{t("settings.launcher.appearance.sliderSize")}</span>
				<output for="tile-slider-size">
					{sliderScalePercent(
						launcherStore.settings.tile_slider_size,
					)}<span>%</span>
				</output>
			</label>
			<div class="range-control-row">
				<input
					class="qm-range-input"
					id="tile-slider-size"
					type="range"
					min={SIZE.min}
					max={SIZE.max}
					step="1"
					style:--range-fill={fill(
						launcherStore.settings.tile_slider_size,
						SIZE,
					)}
					bind:value={launcherStore.settings.tile_slider_size}
					onchange={() => onsave()}
				/>
				<RangeReset
					label={t("settings.launcher.appearance.sliderSize")}
					disabled={(launcherStore.settings.tile_slider_size ??
						TILE_RANGE_DEFAULTS.tile_slider_size) ===
						TILE_RANGE_DEFAULTS.tile_slider_size}
					onclick={() => resetRange("tile_slider_size")}
				/>
			</div>
			<span class="ap-hint">
				{t("settings.launcher.appearance.sliderSizeHint")}
			</span>
		</div>
		{#if launcherStore.settings.tile_slider_layout !== "stretch"}
			<div class="ap-field">
				<label class="ap-label" for="tile-slider-size-y">
					<span>{t("settings.launcher.appearance.sliderSizeY")}</span>
					<output for="tile-slider-size-y"
						>{sliderScalePercent(
							launcherStore.settings.tile_slider_size_y ?? 80,
						)}<span>%</span></output
					>
				</label>
				<div class="range-control-row">
					<input
						class="qm-range-input"
						id="tile-slider-size-y"
						type="range"
						min={SIZE.min}
						max={SIZE.max}
						step="1"
						style:--range-fill={fill(
							launcherStore.settings.tile_slider_size_y ?? 80,
							SIZE,
						)}
						value={launcherStore.settings.tile_slider_size_y ?? 80}
						oninput={(event) => {
							launcherStore.settings.tile_slider_size_y = Number(
								event.currentTarget.value,
							);
						}}
						onchange={() => onsave()}
					/>
					<RangeReset
						label={t("settings.launcher.appearance.sliderSizeY")}
						disabled={(launcherStore.settings.tile_slider_size_y ??
							TILE_RANGE_DEFAULTS.tile_slider_size_y) ===
							TILE_RANGE_DEFAULTS.tile_slider_size_y}
						onclick={() => resetRange("tile_slider_size_y")}
					/>
				</div>
			</div>
		{/if}

		<div class="ap-field">
			<label class="ap-label" for="tile-slider-edge-inset">
				<span>{t("settings.launcher.appearance.sliderEdgeInset")}</span>
				<output for="tile-slider-edge-inset">
					{t("settings.launcher.appearance.sliderEdgeInsetValue", {
						n: sliderEdgeInset,
					})}
				</output>
			</label>
			<div class="range-control-row">
				<input
					class="qm-range-input"
					id="tile-slider-edge-inset"
					type="range"
					min={TILE_SLIDER_EDGE_INSET.min}
					max={TILE_SLIDER_EDGE_INSET.max}
					step="1"
					style:--range-fill={fill(
						sliderEdgeInset,
						TILE_SLIDER_EDGE_INSET,
					)}
					value={sliderEdgeInset}
					oninput={(event) => {
						launcherStore.settings.tile_slider_edge_inset =
							event.currentTarget.valueAsNumber;
					}}
					onchange={() => onsave()}
				/>
				<RangeReset
					label={t("settings.launcher.appearance.sliderEdgeInset")}
					disabled={sliderEdgeInset ===
						TILE_RANGE_DEFAULTS.tile_slider_edge_inset}
					onclick={() => resetRange("tile_slider_edge_inset")}
				/>
			</div>
		</div>

		<div class="ap-field">
			<span class="ap-label">
				<span>{t("settings.launcher.appearance.sliderLayout")}</span>
			</span>
			<SegmentedControl
				id="tile-slider-layout"
				label={t("settings.launcher.appearance.sliderLayout")}
				value={launcherStore.settings.tile_slider_layout ?? "size"}
				options={[
					{
						value: "size",
						label: t(
							"settings.launcher.appearance.sliderLayoutSize",
						),
					},
					{
						value: "stretch",
						label: t(
							"settings.launcher.appearance.sliderLayoutStretch",
						),
					},
					{
						value: "custom",
						label: t(
							"settings.launcher.appearance.sliderLayoutCustom",
						),
					},
				]}
				onchange={(value) => {
					launcherStore.settings.tile_slider_layout = value as
						"size" | "stretch" | "custom";
					onsave();
				}}
			/>
			<span class="ap-hint">
				{t("settings.launcher.appearance.sliderLayoutHint")}
			</span>
		</div>

		{#if (launcherStore.settings.tile_slider_layout ?? "size") === "custom"}
			<div class="ap-field">
				<label class="ap-label" for="tile-slider-rows">
					<span>{t("settings.launcher.appearance.sliderRows")}</span>
					<output for="tile-slider-rows"
						>{launcherStore.settings.tile_slider_rows ?? 2}</output
					>
				</label>
				<div class="range-control-row">
					<input
						class="qm-range-input"
						id="tile-slider-rows"
						type="range"
						min={SLIDER_ROWS.min}
						max={SLIDER_ROWS.max}
						step="1"
						style:--range-fill={fill(
							launcherStore.settings.tile_slider_rows ?? 2,
							SLIDER_ROWS,
						)}
						value={launcherStore.settings.tile_slider_rows ?? 2}
						oninput={(event) => {
							launcherStore.settings.tile_slider_rows = Number(
								event.currentTarget.value,
							);
						}}
						onchange={() => onsave()}
					/>
					<RangeReset
						label={t("settings.launcher.appearance.sliderRows")}
						disabled={(launcherStore.settings.tile_slider_rows ??
							TILE_RANGE_DEFAULTS.tile_slider_rows) ===
							TILE_RANGE_DEFAULTS.tile_slider_rows}
						onclick={() => resetRange("tile_slider_rows")}
					/>
				</div>
			</div>
		{/if}

		<div class="ap-field">
			<span class="ap-label">
				<span>{t("settings.launcher.appearance.motionTitle")}</span>
			</span>
			<SegmentedControl
				id="slider-motion"
				label={t("settings.launcher.appearance.motionTitle")}
				value={appearance.sliderMotion}
				options={motionOptions}
				onchange={(value) => setSliderMotion(value as SliderMotion)}
			/>
			<span class="ap-hint">
				{t("settings.launcher.appearance.motionHint")}
			</span>
		</div>

		<label class="ap-option">
			<ToggleSwitch
				checked={launcherStore.settings.tile_slider_hide_text ?? false}
				onchange={(event) => {
					launcherStore.settings.tile_slider_hide_text =
						event.currentTarget.checked;
					onsave();
				}}
			/>
			<span class="ap-option-copy">
				<span class="ap-option-label"
					>{t("settings.launcher.appearance.sliderHideText")}</span
				>
				<span class="ap-hint"
					>{t(
						"settings.launcher.appearance.sliderHideTextHint",
					)}</span
				>
			</span>
		</label>

		<div class="ap-field">
			<label class="ap-label" for="tile-slider-grid-gap">
				<span>{t("settings.launcher.appearance.sliderGridSize")}</span>
				<output for="tile-slider-grid-gap"
					>{launcherStore.settings.tile_slider_grid_gap ?? 8}<span>
						px</span
					></output
				>
			</label>
			<div class="range-control-row">
				<input
					class="qm-range-input"
					id="tile-slider-grid-gap"
					type="range"
					min="0"
					max="24"
					step="1"
					style:--range-fill={fill(
						launcherStore.settings.tile_slider_grid_gap ?? 8,
						{ min: 0, max: 24 },
					)}
					value={launcherStore.settings.tile_slider_grid_gap ?? 8}
					oninput={(event) => {
						launcherStore.settings.tile_slider_grid_gap = Number(
							event.currentTarget.value,
						);
					}}
					onchange={() => onsave()}
				/>
				<RangeReset
					label={t("settings.launcher.appearance.sliderGridSize")}
					disabled={(launcherStore.settings.tile_slider_grid_gap ??
						TILE_RANGE_DEFAULTS.tile_slider_grid_gap) ===
						TILE_RANGE_DEFAULTS.tile_slider_grid_gap}
					onclick={() => resetRange("tile_slider_grid_gap")}
				/>
			</div>
			<span class="ap-hint"
				>{t("settings.launcher.appearance.sliderGridSizeHint")}</span
			>
		</div>
		{#if appearance.sliderMotion !== "off"}
			<div class="ap-field">
				<label class="ap-label" for="tile-slider-gap">
					<span>{t("settings.launcher.appearance.sliderGap")}</span>
					<output for="tile-slider-gap"
						>{launcherStore.settings.tile_slider_gap ?? 8}<span>
							px</span
						></output
					>
				</label>
				<div class="range-control-row">
					<input
						class="qm-range-input"
						id="tile-slider-gap"
						type="range"
						min="0"
						max="32"
						step="1"
						style:--range-fill={fill(
							launcherStore.settings.tile_slider_gap ?? 8,
							{ min: 0, max: 32 },
						)}
						value={launcherStore.settings.tile_slider_gap ?? 8}
						oninput={(event) => {
							launcherStore.settings.tile_slider_gap = Number(
								event.currentTarget.value,
							);
						}}
						onchange={() => onsave()}
					/>
					<RangeReset
						label={t("settings.launcher.appearance.sliderGap")}
						disabled={(launcherStore.settings.tile_slider_gap ??
							TILE_RANGE_DEFAULTS.tile_slider_gap) ===
							TILE_RANGE_DEFAULTS.tile_slider_gap}
						onclick={() => resetRange("tile_slider_gap")}
					/>
				</div>
				<span class="ap-hint"
					>{t("settings.launcher.appearance.sliderGapHint")}</span
				>
			</div>
		{/if}
		{#if appearance.sliderMotion !== "off"}
			<div class="ap-field">
				<label class="ap-label" for="tile-slider-speed">
					<span>{t("settings.launcher.appearance.sliderSpeed")}</span>
					<output for="tile-slider-speed">
						{launcherStore.settings.tile_slider_speed}<span>%</span>
					</output>
				</label>
				<div class="range-control-row">
					<input
						class="qm-range-input"
						id="tile-slider-speed"
						type="range"
						min={SPEED.min}
						max={SPEED.max}
						step="1"
						style:--range-fill={fill(
							launcherStore.settings.tile_slider_speed,
							SPEED,
						)}
						bind:value={launcherStore.settings.tile_slider_speed}
						onchange={() => onsave()}
					/>
					<RangeReset
						label={t("settings.launcher.appearance.sliderSpeed")}
						disabled={(launcherStore.settings.tile_slider_speed ??
							TILE_RANGE_DEFAULTS.tile_slider_speed) ===
							TILE_RANGE_DEFAULTS.tile_slider_speed}
						onclick={() => resetRange("tile_slider_speed")}
					/>
				</div>
				<span class="ap-hint">
					{t("settings.launcher.appearance.sliderSpeedHint")}
				</span>
			</div>
		{/if}

		<div class="ap-field">
			<Select
				id="screenshot-interval"
				compact
				label={t("settings.launcher.appearance.screenshotInterval")}
				value={String(launcherStore.settings.screenshot_interval)}
				options={intervalOptions}
				onchange={(value: string) => {
					launcherStore.settings.screenshot_interval = Number(value);
					onsave();
				}}
			/>
		</div>
	</section>

	<section
		class="ap-group"
		aria-label={t("settings.launcher.appearance.groupLayout")}
	>
		<h3 class="ap-group-title">
			{t("settings.launcher.appearance.groupLayout")}
		</h3>

		<div class="ap-field">
			<span class="ap-label"
				><span>{t("settings.launcher.appearance.tileStyle")}</span
				></span
			>
			<SegmentedControl
				id="tile-style"
				label={t("settings.launcher.appearance.tileStyle")}
				value={tileStyle}
				options={tileStyleOptions}
				onchange={(value) => {
					launcherStore.settings.tile_style = value as TileStyle;
					onsave();
				}}
			/>
			<span class="ap-hint"
				>{t(
					`settings.launcher.appearance.tileStyleHints.${tileStyle}`,
				)}</span
			>
		</div>

		{#if tileStyle === "glass"}
			<div class="ap-field">
				<label class="ap-label" for="tile-glass-blur">
					<span>{t("settings.launcher.appearance.glassBlur")}</span>
					<output for="tile-glass-blur">
						{glassBlur}<span>%</span>
					</output>
				</label>
				<div class="range-control-row">
					<input
						class="qm-range-input"
						id="tile-glass-blur"
						type="range"
						min={GLASS_BLUR.min}
						max={GLASS_BLUR.max}
						step="1"
						style:--range-fill={fill(glassBlur, GLASS_BLUR)}
						value={glassBlur}
						oninput={(event) => {
							launcherStore.settings.tile_centered_blur = Number(
								event.currentTarget.value,
							);
						}}
						onchange={() => onsave()}
					/>
					<RangeReset
						label={t("settings.launcher.appearance.glassBlur")}
						disabled={glassBlur ===
							TILE_RANGE_DEFAULTS.tile_centered_blur}
						onclick={() => resetRange("tile_centered_blur")}
					/>
				</div>
				<span class="ap-hint"
					>{t("settings.launcher.appearance.glassBlurHint")}</span
				>
			</div>
		{/if}

		{#if tileStyle === "standard" || tileStyle === "glass"}
			<div class="ap-field">
				<label class="ap-label" for="tile-slider-border">
					<span
						>{t(
							tileStyle === "glass"
								? "settings.launcher.appearance.glassFieldSize"
								: "settings.launcher.appearance.sliderBorder",
						)}</span
					>
					<output for="tile-slider-border"
						>{launcherStore.settings.tile_slider_border ?? 50}<span
							>%</span
						></output
					>
				</label>
				<div class="range-control-row">
					<input
						class="qm-range-input"
						id="tile-slider-border"
						type="range"
						min={SLIDER_BORDER.min}
						max={SLIDER_BORDER.max}
						step="1"
						style:--range-fill={fill(
							launcherStore.settings.tile_slider_border ?? 50,
							SLIDER_BORDER,
						)}
						value={launcherStore.settings.tile_slider_border ?? 50}
						oninput={(event) => {
							launcherStore.settings.tile_slider_border = Number(
								event.currentTarget.value,
							);
						}}
						onchange={() => onsave()}
					/>
					<RangeReset
						label={t(
							"settings.launcher.appearance." +
								(tileStyle === "glass"
									? "glassFieldSize"
									: "sliderBorder"),
						)}
						disabled={(launcherStore.settings.tile_slider_border ??
							TILE_RANGE_DEFAULTS.tile_slider_border) ===
							TILE_RANGE_DEFAULTS.tile_slider_border}
						onclick={() => resetRange("tile_slider_border")}
					/>
				</div>
				<span class="ap-hint"
					>{t(
						tileStyle === "glass"
							? "settings.launcher.appearance.glassFieldSizeHint"
							: "settings.launcher.appearance.sliderBorderHint",
					)}</span
				>
			</div>
		{/if}

		{#if tileStyle === "stylish"}
			<div class="ap-field">
				<span class="ap-label" id="tile-text-pos-label">
					<span>{t("settings.launcher.appearance.tileTextPos")}</span>
				</span>
				<button
					type="button"
					class="ap-auto"
					aria-pressed={tileTextPos === "auto"}
					onclick={() => {
						launcherStore.settings.tile_text_pos = "auto";
						onsave();
					}}
					>{t("settings.launcher.appearance.tileTextPosAuto")}</button
				>
				<div
					class="ap-position-picker"
					role="group"
					aria-labelledby="tile-text-pos-label"
				>
					{#each TEXT_POSITIONS as value (value)}
						<button
							type="button"
							class="ap-pos-cell"
							data-pos={value}
							aria-pressed={tileTextPos === value}
							aria-label={t(
								`settings.launcher.appearance.tileTextPositions.${value}`,
							)}
							title={t(
								`settings.launcher.appearance.tileTextPositions.${value}`,
							)}
							onclick={() => {
								launcherStore.settings.tile_text_pos = value;
								onsave();
							}}><span class="ap-pos-bar"></span></button
						>
					{/each}
				</div>
				<span class="ap-hint"
					>{t("settings.launcher.appearance.tileTextPosHint")}</span
				>
			</div>
		{:else}
			<div class="ap-field">
				<span class="ap-label"
					><span>{t("settings.launcher.appearance.tileSide")}</span
					></span
				>
				<SegmentedControl
					id="tile-side"
					label={t("settings.launcher.appearance.tileSide")}
					value={tileSide}
					options={tileSideOptions}
					onchange={(value) => {
						launcherStore.settings.tile_side = value as TileSide;
						onsave();
					}}
				/>
				<span class="ap-hint"
					>{t("settings.launcher.appearance.tileSideHint")}</span
				>
			</div>
		{/if}

		<label class="ap-option">
			<ToggleSwitch
				checked={appearance.quickActionsDisabled}
				onchange={(event) =>
					setQuickActionsDisabled(event.currentTarget.checked)}
			/>
			<span class="ap-option-copy">
				<span class="ap-option-label"
					>{t(
						"settings.launcher.appearance.disableInstanceQuickActions",
					)}</span
				>
				<span class="ap-hint"
					>{t(
						"settings.launcher.appearance.disableInstanceQuickActionsHint",
					)}</span
				>
			</span>
		</label>

		<label class="ap-option">
			<ToggleSwitch
				checked={panelLayout.locked}
				onchange={(event) =>
					setPanelOrientationLocked(event.currentTarget.checked)}
			/>
			<span class="ap-option-copy">
				<span class="ap-option-label"
					>{t("settings.launcher.appearance.lockPanels")}</span
				>
				<span class="ap-hint"
					>{t("settings.launcher.appearance.lockPanelsHint")}</span
				>
			</span>
		</label>
	</section>
</div>

<style>
	/* Стиль как у раздела «Интерфейс» в «Персонализации»: без собственной рамки и отступа. */
	.appearance-settings {
		display: flex;
		flex-direction: column;
		gap: calc(14px * var(--cubic-interface-gap-factor, 1));
		min-width: 0;
	}

	.ap-group {
		display: flex;
		flex-direction: column;
		gap: calc(12px * var(--cubic-interface-gap-factor, 1));
		min-width: 0;
	}

	.ap-group-title {
		margin: 0;
		font-size: 0.8rem;
		font-weight: 600;
		color: var(--text-secondary);
	}

	.ap-field {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
	}

	/* Nombre y valor van juntos, sin separarse a los bordes. */
	.ap-label {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.82rem;
		color: var(--text-secondary);
	}

	output {
		padding: 1px 6px;
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		background: var(--bg-input);
		font-size: 0.72rem;
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		color: var(--text-primary);
		white-space: nowrap;
	}

	output span {
		margin-left: 1px;
		color: var(--text-muted);
	}

	.ap-hint {
		display: block;
		font-size: 0.7rem;
		line-height: 1.45;
		color: var(--text-muted);
	}

	.ap-option {
		display: flex;
		align-items: flex-start;
		gap: 12px;
		cursor: pointer;
		user-select: none;
	}

	.ap-option-copy {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.ap-option-label {
		font-size: 0.82rem;
		color: var(--text-secondary);
		transition: color 0.2s;
	}

	.ap-option:hover .ap-option-label {
		color: var(--text-primary);
	}

	.ap-auto {
		height: 30px;
		padding: 0 10px;
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		background: var(--bg-input);
		color: var(--text-secondary);
		font-family: inherit;
		font-size: 0.78rem;
		cursor: pointer;
	}
	.ap-auto[aria-pressed="true"],
	.ap-pos-cell[aria-pressed="true"] {
		border-color: var(--accent);
		background: rgba(var(--accent-rgb), 0.12);
	}
	.ap-position-picker {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 6px;
	}
	.ap-pos-cell {
		display: flex;
		height: 34px;
		padding: 0;
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		background: var(--bg-input);
		cursor: pointer;
	}
	.ap-pos-cell[data-pos^="top"] {
		align-items: flex-start;
	}
	.ap-pos-cell[data-pos^="bottom"] {
		align-items: flex-end;
	}
	.ap-pos-cell[data-pos$="left"] {
		justify-content: flex-start;
	}
	.ap-pos-cell[data-pos$="center"] {
		justify-content: center;
	}
	.ap-pos-cell[data-pos$="right"] {
		justify-content: flex-end;
	}
	.ap-pos-bar {
		width: 16px;
		height: 5px;
		margin: 5px;
		border-radius: 3px;
		background: var(--text-muted);
	}
	.ap-pos-cell[aria-pressed="true"] .ap-pos-bar {
		background: var(--accent);
	}
	.ap-auto:focus-visible,
	.ap-pos-cell:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
</style>
