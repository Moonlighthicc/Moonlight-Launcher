<script lang="ts">
	import { t } from "$lib/i18n";
	import TileSettingsPanel from "$lib/components/instances/InstanceView/TileSettingsPanel.svelte";
	import DockPicker from "./DockPicker.svelte";
	import {
		sidebarLayout,
		setSidebarPlacement,
		cellOf,
		placementOf,
		type SidebarCell,
	} from "$lib/state/sidebarLayoutState.svelte";
	import {
		panelLayout,
		setGamePanelDock,
		type GamePanelDock,
	} from "$lib/state/panelLayoutState.svelte";
	let { onsave }: { onsave: () => unknown } = $props();
	const SIDEBAR_CELLS: SidebarCell[] = [
		"top-start",
		"top-end",
		"bottom-start",
		"bottom-end",
		"left",
		"right",
	];
	const GAME_DOCKS: GamePanelDock[] = [
		"top-start",
		"top-end",
		"bottom-start",
		"bottom-end",
	];
	/** Сетка 4×4: края по половине, боковые панели занимают середину. */
	const GAME_AREAS: Record<GamePanelDock, string> = {
		"top-start": "1 / 1 / 2 / 3",
		"top-end": "1 / 3 / 2 / 5",
		"bottom-start": "4 / 1 / 5 / 3",
		"bottom-end": "4 / 3 / 5 / 5",
	};
	const SIDEBAR_AREAS: Record<SidebarCell, string> = {
		"top-start": "1 / 1 / 2 / 3",
		"top-end": "1 / 3 / 2 / 5",
		left: "2 / 1 / 4 / 2",
		right: "2 / 4 / 4 / 5",
		"bottom-start": "4 / 1 / 5 / 3",
		"bottom-end": "4 / 3 / 5 / 5",
	};
	const sidebarDockCells = $derived(
		SIDEBAR_CELLS.map((value) => ({
			value,
			label: t(`settings.launcher.appearance.gameDocks.${value}`),
			area: SIDEBAR_AREAS[value],
		})),
	);
	const sidebarCell = $derived(
		cellOf(sidebarLayout.dock, sidebarLayout.align),
	);
	const gameDockCells = $derived(
		GAME_DOCKS.map((value) => ({
			value,
			label: t(`settings.launcher.appearance.gameDocks.${value}`),
			area: GAME_AREAS[value],
		})),
	);
</script>

<div class="appearance-settings settings-controls">
	<TileSettingsPanel {onsave} />
	<div class="ap-field">
		<span class="ap-label">
			<span>{t("settings.launcher.appearance.dockTitle")}</span>
		</span>
		<DockPicker
			id="sidebar-dock"
			label={t("settings.launcher.appearance.dockTitle")}
			value={sidebarCell}
			options={sidebarDockCells}
			columns={4}
			rows={4}
			onchange={(value) => {
				const placement = placementOf(value as SidebarCell);
				setSidebarPlacement(placement.dock, placement.align);
			}}
		/>
		<span class="ap-hint">
			{t("settings.launcher.appearance.dockHint")}
		</span>
	</div>

	<div class="ap-field">
		<span class="ap-label"
			><span>{t("settings.launcher.appearance.gameDockTitle")}</span
			></span
		>
		<DockPicker
			id="game-panel-dock"
			label={t("settings.launcher.appearance.gameDockTitle")}
			value={panelLayout.gameDock}
			options={gameDockCells}
			columns={4}
			rows={4}
			onchange={(value) => setGamePanelDock(value)}
		/>
		<span class="ap-hint"
			>{t("settings.launcher.appearance.gameDockHint")}</span
		>
	</div>
</div>

<style>
	.appearance-settings {
		display: flex;
		flex-direction: column;
		gap: calc(14px * var(--cubic-interface-gap-factor, 1));
		padding-top: 10px;
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

	.ap-hint {
		display: block;
		font-size: 0.7rem;
		line-height: 1.45;
		color: var(--text-muted);
	}
</style>
