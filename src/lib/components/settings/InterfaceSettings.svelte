<script lang="ts">
	import { onDestroy } from "svelte";
	import { launcherStore, showError } from "$lib/state/state.svelte";
	import {
		appearance,
		setAccentColor,
	} from "$lib/state/appearanceState.svelte";
	import { t } from "$lib/i18n";
	import { applyInterfaceScale } from "$lib/api/interfaceAppearance";
	import { patchPreferences } from "$lib/utils/preferencePatch";
	import { animDuration } from "$lib/utils/animations";
	import SegmentedControl from "./SegmentedControl.svelte";
	import RangeReset from "./RangeReset.svelte";
	import Select from "$lib/components/layout/Select.svelte";
	import { FONT_THEME, PROJECT_FONTS } from "$lib/fonts/projectFonts";
	import {
		DEFAULT_INTERFACE_PREFERENCES,
		INTERFACE_SCALES,
		INTERFACE_DENSITIES,
		interfacePreferences,
		type InterfacePreferences,
	} from "$lib/utils/interfacePreferences";
	import "./controls.css";

	let { onsave }: { onsave: () => Promise<void> } = $props();
	let changing = $state(false);
	let scaleIndex = $state(1);
	let preview:
		| {
				scale: InterfacePreferences["scale"];
				applied: Promise<void>;
				controller: AbortController;
		  }
		| undefined;
	let selectedDensity = $state("theme");
	let selectedFont = $state(FONT_THEME);
	const preferences = $derived(
		interfacePreferences(launcherStore.settings.interface_preferences),
	);
	$effect(() => {
		scaleIndex = INTERFACE_SCALES.indexOf(preferences.scale);
	});
	$effect(() => {
		selectedDensity = preferences.density;
		selectedFont = preferences.font;
	});
	const densities = $derived(
		INTERFACE_DENSITIES.map((value) => ({
			value,
			label: t(`settings.interface.densities.${value}`),
		})),
	);
	const fonts = $derived([
		{ value: FONT_THEME, label: t("settings.interface.fonts.theme") },
		...PROJECT_FONTS.map((font) => ({
			value: font.family,
			label: font.family,
		})),
	]);

	function previewScale(index: number) {
		const scale = INTERFACE_SCALES[index];
		if (scale === undefined || preview?.scale === scale) return;
		scaleIndex = index;
		preview?.controller.abort();
		const controller = new AbortController();
		const request = {
			scale,
			controller,
			applied: applyInterfaceScale(scale, {
				delay: 140,
				duration: animDuration(180),
				signal: controller.signal,
			}),
		};
		preview = request;
		void request.applied.catch((error) => {
			if (preview !== request) return;
			preview = undefined;
			scaleIndex = INTERFACE_SCALES.indexOf(preferences.scale);
			// An earlier preview may have succeeded before this one failed.
			void applyInterfaceScale(preferences.scale).catch(() => {});
			showError(t("settings.interface.scaleError"), String(error));
		});
	}

	async function commitScale() {
		const request = preview;
		if (!request) return;
		try {
			await request.applied;
			if (preview !== request) return;
			preview = undefined;
			if (request.scale === preferences.scale) return;
			const current = launcherStore.settings.interface_preferences;
			if (current) patchPreferences(current, { scale: request.scale });
			else
				launcherStore.settings.interface_preferences = {
					...preferences,
					scale: request.scale,
				};
			await onsave();
		} catch {
			// Preview failures are handled above; settings saves report their own errors.
		}
	}

	onDestroy(() => {
		void commitScale();
	});

	async function update(patch: Partial<InterfacePreferences>) {
		if (changing) return;
		const replacesPreview =
			patch.scale !== undefined && preview !== undefined;
		if (replacesPreview) {
			preview?.controller.abort();
			preview = undefined;
		}
		const next = interfacePreferences({ ...preferences, ...patch });
		if (
			!replacesPreview &&
			next.scale === preferences.scale &&
			next.density === preferences.density &&
			next.font === preferences.font
		)
			return;
		try {
			changing = true;
			try {
				// Save only once native zoom succeeds; a failure keeps the old setting.
				if (replacesPreview || next.scale !== preferences.scale)
					await applyInterfaceScale(next.scale, {
						duration: animDuration(180),
					});
				const current = launcherStore.settings.interface_preferences;
				if (current) patchPreferences(current, patch);
				else launcherStore.settings.interface_preferences = { ...next };
			} finally {
				if (patch.scale !== undefined)
					scaleIndex = INTERFACE_SCALES.indexOf(preferences.scale);
				selectedDensity = preferences.density;
				selectedFont = preferences.font;
				// Disk writes do not need to block the controls or native zoom.
				changing = false;
			}
			await onsave();
		} catch (error) {
			showError(t("settings.interface.scaleError"), String(error));
		}
	}
</script>

<div class="interface-settings settings-controls" aria-busy={changing}>
	<div class="interface-accent">
		<label for="launcher-accent-color"
			>{t("settings.launcher.appearance.accentColor")}</label
		>
		<input
			id="launcher-accent-color"
			type="color"
			value={appearance.accentColor ?? "#33b1ff"}
			oninput={(event) =>
				setAccentColor(event.currentTarget.value, false)}
			onchange={(event) => setAccentColor(event.currentTarget.value)}
		/>
		<button
			type="button"
			class="detect-btn"
			disabled={appearance.accentColor === null}
			onclick={() => setAccentColor(null)}
			>{t("settings.launcher.appearance.resetAccent")}</button
		>
	</div>
	<div class="interface-field">
		<label class="interface-label" for="interface-scale">
			{t("settings.interface.scale")}
			<output for="interface-scale"
				>{INTERFACE_SCALES[scaleIndex]}%</output
			>
		</label>
		<div class="range-control-row">
			<input
				id="interface-scale"
				class="qm-range-input"
				type="range"
				min="0"
				max={INTERFACE_SCALES.length - 1}
				step="1"
				value={scaleIndex}
				aria-valuetext={`${INTERFACE_SCALES[scaleIndex]}%`}
				style:--range-fill={`${(scaleIndex / (INTERFACE_SCALES.length - 1)) * 100}%`}
				disabled={changing}
				oninput={(event) =>
					previewScale(event.currentTarget.valueAsNumber)}
				onchange={commitScale}
				onpointercancel={commitScale}
			/>
			<RangeReset
				label={t("settings.interface.scale")}
				disabled={changing ||
					INTERFACE_SCALES[scaleIndex] ===
						DEFAULT_INTERFACE_PREFERENCES.scale}
				onclick={() =>
					update({ scale: DEFAULT_INTERFACE_PREFERENCES.scale })}
			/>
		</div>
		<p class="qm-ram-hint">{t("settings.interface.scaleHint")}</p>
	</div>
	<div class="interface-field">
		<span class="interface-label">{t("settings.interface.density")}</span>
		<SegmentedControl
			id="interface-density"
			label={t("settings.interface.density")}
			value={selectedDensity}
			options={densities}
			disabled={changing}
			onchange={(value) =>
				update({
					density: value as InterfacePreferences["density"],
				})}
		/>
		<p class="qm-ram-hint">{t("settings.interface.densityHint")}</p>
	</div>
	<div class="interface-field">
		<span class="interface-label">{t("settings.interface.font")}</span>
		<Select
			id="interface-font"
			label={t("settings.interface.font")}
			bind:value={selectedFont}
			options={fonts}
			disabled={changing}
			onchange={(value) => update({ font: value })}
		/>
		<p class="qm-ram-hint">{t("settings.interface.fontHint")}</p>
	</div>
	<div class="interface-actions">
		<button
			type="button"
			class="detect-btn"
			disabled={changing}
			onclick={() => update(DEFAULT_INTERFACE_PREFERENCES)}
		>
			{t("settings.interface.reset")}
		</button>
	</div>
</div>

<style>
	.interface-settings {
		display: flex;
		flex-direction: column;
		padding-top: 10px;
		gap: calc(14px * var(--cubic-interface-gap-factor, 1));
	}
	.interface-field {
		min-width: 0;
	}
	.interface-accent {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	.interface-accent label {
		font-size: 0.8rem;
		color: var(--text-secondary);
	}
	.interface-accent input {
		width: 40px;
		height: 30px;
		padding: 3px;
		background: var(--bg-input);
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		cursor: pointer;
	}
	.interface-label {
		display: block;
		font-size: 0.8rem;
		color: var(--text-secondary);
		margin-bottom: 8px;
	}
	.interface-settings :global(.qm-ram-hint) {
		margin: 6px 0 0;
		padding: 0;
	}
	.interface-actions {
		padding-top: 12px;
		border-top: 1px solid var(--border-color);
	}
</style>
