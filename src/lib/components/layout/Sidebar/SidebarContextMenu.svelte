<script lang="ts">
	import { t } from "$lib/i18n";
	import { invoke } from "@tauri-apps/api/core";
	import { exportInstanceZip, openInstanceDir, killInstance } from "$lib/api/cubicApi";
	import { isInstanceDownloading, playInstance } from "$lib/api/instancePlay";
	import { killInst } from "$lib/api/launcherService";
	import {
		launcherStore,
		removeNotification,
		showErrorParsed,
		showInfo,
		showSuccess,
	} from "$lib/state/state.svelte";
	import { InstState, type InstanceDto } from "$lib/types/types";
	import type { ContextMenuItem } from "$lib/components/layout/ContextMenu.svelte";
	import type ContextMenuComponent from "$lib/components/layout/ContextMenu.svelte";
	import { save } from "@tauri-apps/plugin-dialog";
	import MrpackExportModal from "./MrpackExportModal.svelte";

	type ContextMenuConstructor = typeof ContextMenuComponent;

	interface Props {
		onedit: (instance: InstanceDto) => void;
		ondelete: (instance: InstanceDto) => void;
		onpin: (instance: InstanceDto, pinned: boolean) => Promise<void>;
	}

	let { onedit, ondelete, onpin }: Props = $props();

	let ctxOpen = $state(false);
	let ctxX = $state(0);
	let ctxY = $state(0);
	let ctxItems = $state<ContextMenuItem[]>([]);
	let MenuComponent = $state<ContextMenuConstructor | null>(null);
	let loadPromise: Promise<void> | null = null;
	let openRequest = 0;
	let mrpackOpen = $state(false);
	let mrpackInstanceId = $state("");

	async function loadMenuComponent() {
		if (MenuComponent) return;
		if (loadPromise) {
			await loadPromise;
			return;
		}

		loadPromise = import("$lib/components/layout/ContextMenu.svelte").then(
			(mod) => {
				MenuComponent = mod.default;
			},
		);
		await loadPromise;
	}

	async function handleExport(instance: InstanceDto) {
		try {
			const dest = await save({
				defaultPath: `${instance.name}.zip`,
				filters: [{ name: "ZIP", extensions: ["zip"] }],
			});
			if (!dest) return;

			const notificationId = showInfo(
				t("notifications.exportingTitle"),
				t("notifications.exportingMessage", { name: instance.name }),
			);

			const path = await exportInstanceZip(instance.uuid, dest);
			removeNotification(notificationId);

			if (path) {
				showSuccess(
					t("notifications.exportTitle"),
					t("notifications.exportSuccess", { path }),
				);
			}
		} catch (err) {
			showErrorParsed(err);
		}
	}

	function buildInstanceMenu(instance: InstanceDto, processIds: string[]): ContextMenuItem[] {
		const isRunning = processIds.length > 0 || instance.status === "started";
		const cannotLaunch = instance.status === InstState.Starting || isInstanceDownloading(instance);
		const windowItems: ContextMenuItem[] = processIds.length > 0
			? processIds.map((processId, index) => ({
				label: t("instanceView.minecraftWindow", { number: index + 1 }),
				iconName: "ui:close",
				action: async () => {
					try {
						await killInstance(instance.uuid, undefined, undefined, processId);
					} catch (error) {
						showErrorParsed(error);
					}
				},
			}))
			: [{ label: t("instanceView.runningWindows", { count: 0 }), disabled: true }];
		const launchAnotherItem: ContextMenuItem = {
			label: t("instanceView.launchAnother"),
			iconName: "ui:play",
			// Те же условия, что у кнопки в карточке инстанса: не во время запуска или загрузки.
			disabled: cannotLaunch,
			action: async () => {
				try {
					await playInstance(instance);
				} catch (error) {
					showErrorParsed(error);
				}
			},
		};
		const submenuItems = isRunning ? [...windowItems, launchAnotherItem] : windowItems;
		return [
			{
				label: isRunning ? t("instanceView.close") : t("instanceView.playBtn"),
				iconName: isRunning ? "ui:close" : "ui:play",
				variant: "accent",
				prominent: true,
				split: true,
				children: submenuItems,
				badge: processIds.length > 0 ? processIds.length : undefined,
				disabled: !isRunning && cannotLaunch,
				splitLabel: t("instanceView.runningWindows", { count: processIds.length }),
				action: async () => {
					try {
						if (isRunning) await killInst(instance.uuid);
						else await playInstance(instance);
					} catch (error) {
						showErrorParsed(error);
					}
				},
			},
			{
				label: instance.pinned ? t("sidebar.unpin") : t("sidebar.pin"),
				iconName: "ui:pin",
				action: async () => {
					const previousPinned = instance.pinned;
					const newPinned = !previousPinned;
					instance.pinned = newPinned;
					try {
						await onpin(instance, newPinned);
					} catch {
						instance.pinned = previousPinned;
					}
				},
			},
			{
				label: t("sidebar.edit"),
				iconName: "nav:edit",
				action: () => onedit(instance),
			},
			{
				label: t("sidebar.openFolder"),
				iconName: "instance:folder",
				split: true,
				splitLabel: t("sidebar.openFolder"),
				action: () => openInstanceDir(instance.uuid),
				children: [
					{ label: "mods", action: () => openInstanceDir(instance.uuid, "mods") },
					{ label: "resourcepacks", action: () => openInstanceDir(instance.uuid, "resourcepacks") },
					{ label: "shaderpacks", action: () => openInstanceDir(instance.uuid, "shaderpacks") },
					{ label: "screenshots", action: () => openInstanceDir(instance.uuid, "screenshots") },
					{ label: "worlds", action: () => openInstanceDir(instance.uuid, "saves") },
				],
			},
			{
				label: t("sidebar.export"),
				iconName: "ui:download",
				action: () => void handleExport(instance),
			},
			{
				label: t("mrpackExport.title"),
				iconName: "ui:download",
				action: () => {
					mrpackInstanceId = instance.uuid;
					mrpackOpen = true;
				},
			},
			{ separator: true, label: "" },
			{
				label: t("sidebar.delete"),
				iconName: "ui:trash",
				variant: "danger",
				action: () => ondelete(instance),
			},
		];
	}

	export async function openContextMenu(e: MouseEvent, anchor?: HTMLElement) {
		const target = anchor ?? (e.target instanceof Element ? e.target : null);
		const instanceEl = target?.closest<HTMLElement>("[data-instance-uuid]");

		if (!instanceEl) return;

		const uuid = instanceEl.dataset.instanceUuid;
		const instance = launcherStore.loadedInstances.find(
			(i) => i.uuid === uuid,
		);
		if (!instance) return;

		e.preventDefault();
		const rect = (anchor ?? instanceEl).getBoundingClientRect();
		const anchored = !!anchor || (e.clientX === 0 && e.clientY === 0);
		const x = anchored ? rect.left : e.clientX;
		const y = anchored ? rect.bottom + 4 : e.clientY;
		const request = ++openRequest;
		try {
			const [processIds] = await Promise.all([
				invoke<string[]>("get_instance_processes", { uuid: instance.uuid }).catch(() => []),
				loadMenuComponent(),
			]);
			if (request !== openRequest || !instanceEl.isConnected) return;
			ctxX = x;
			ctxY = y;
			ctxItems = buildInstanceMenu(instance, processIds);
			ctxOpen = true;
		} catch (error) {
			loadPromise = null;
			showErrorParsed(error);
		}
	}
</script>

{#if MenuComponent}
	<MenuComponent bind:open={ctxOpen} x={ctxX} y={ctxY} items={ctxItems} />
{/if}

{#if mrpackOpen}
	{#key mrpackInstanceId}
		<MrpackExportModal bind:open={mrpackOpen} id={mrpackInstanceId} />
	{/key}
{/if}
