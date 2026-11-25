<script lang="ts">
	import { tick } from "svelte";
	import type { MinecraftUser } from "$lib/types/types";
	import { launcherStore, showError } from "$lib/state/state.svelte";
	import {
		accountAvatar,
		createAvatarRefreshSession,
		refreshAvatarSvg,
	} from "$lib/state/avatarCache.svelte";
	import { SvelteSet } from "svelte/reactivity";
	import {
		saveSettings,
		markLocalSettingsChange,
	} from "$lib/api/launcherService";
	import { t } from "$lib/i18n";
	import { logout, switchUser, removeUser } from "$lib/api/cubicApi";
	import AccountSetup from "$lib/components/layout/auth/AccountSetup.svelte";
	import { hasOpenModal } from "$lib/components/layout/ModalBase.svelte";
	import AddAccountCard from "$lib/components/layout/UserMenu/AddAccountCard.svelte";
	import AccountAvatar from "./AccountAvatar.svelte";
	import AccountListItem from "./AccountListItem.svelte";
	import SkinCapeManager from "./SkinCapeManager.svelte";
	import ElySkinManager from "./ElySkinManager.svelte";
	import AuthInjectSkinManager from "./AuthInjectSkinManager.svelte";
	import SkinStorage from "./SkinStorage.svelte";
	import {
		isElyByAccount,
		getAccountProvider,
		type AccountProvider,
	} from "$lib/utils/accountProviders";
	import CloseIcon from "$lib/icons/CloseIcon.svelte";
	import Lupa from "$lib/icons/Lupa.svelte";

	interface Props {
		open?: boolean;
		onclose: () => void;
	}

	let { open = true, onclose }: Props = $props();

	let editingIdx = $state<number | null>(null);
	let editingName = $state("");
	let accountProvider = $state<AccountProvider | null>(null);
	let accountTrigger: HTMLElement | null = null;
	let removingUserUuid = $state<string | null>(null);
	let selectedIdx = $derived(launcherStore.settings.active_user_idx ?? 0);

	async function handleSaveName(idx: number) {
		const regex = /^[a-zA-Z0-9_]{3,16}$/;
		if (!regex.test(editingName)) {
			showError(
				t("userMenu.profileName.invalidTitle"),
				t("userMenu.profileName.invalidDescription"),
			);
			return;
		}
		const user = launcherStore.settings.user[idx];
		if (user) {
			user.username = editingName;
			await saveSettings();
		}
		editingIdx = null;
	}

	async function handleLogout() {
		markLocalSettingsChange();
		await logout();
	}

	async function handleSwitchUser(idx: number) {
		if (idx === launcherStore.settings.active_user_idx) return;
		launcherStore.settings.active_user_idx = idx;
		removingUserUuid = null;
		editingIdx = null;
		markLocalSettingsChange();
		const user = await switchUser(idx);
		if (user) {
			launcherStore.settings.user[idx] = user;
		}
	}

	async function handleRemoveUser(uuid: string) {
		const idx = launcherStore.settings.user.findIndex(
			(u) => u.uuid === uuid,
		);
		if (idx === -1) return;
		launcherStore.settings.user.splice(idx, 1);
		if (launcherStore.settings.user.length === 0) {
			launcherStore.settings.user.push({
				username: "Steve",
				uuid: "",
				access_token: "",
				refresh_token: null,
				user_type: "Cracked",
			});
			launcherStore.settings.active_user_idx = 0;
		} else if (idx <= launcherStore.settings.active_user_idx) {
			launcherStore.settings.active_user_idx = Math.max(
				0,
				launcherStore.settings.active_user_idx - 1,
			);
		}
		removingUserUuid = null;
		markLocalSettingsChange();
		await removeUser(uuid);
	}

	function openAccountSetup(
		provider: AccountProvider,
		trigger: HTMLButtonElement,
	) {
		accountTrigger = trigger;
		accountProvider = provider;
	}

	function accountAdded() {
		editingIdx = null;
		removingUserUuid = null;
	}

	async function closeAccountSetup() {
		accountProvider = null;
		await tick();
		accountTrigger?.focus();
	}

	const refreshAvatars = createAvatarRefreshSession();
	$effect(() => refreshAvatars(open ? launcherStore.settings.user : []));

	const refreshingAvatars = new SvelteSet<string>();
	async function refreshSelectedAvatar() {
		const user = selectedUser;
		if (
			!user?.uuid ||
			user.user_type === "Cracked" ||
			refreshingAvatars.has(user.uuid)
		)
			return;
		const uuid = user.uuid;
		refreshingAvatars.add(uuid);
		try {
			if (!(await refreshAvatarSvg(user))) {
				showError(t("errors.title"), t("userMenu.refreshAvatarError"));
			}
		} finally {
			refreshingAvatars.delete(uuid);
		}
	}

	function getYggdrasilServer(user: {
		yggdrasil_server_url?: string | null;
	}): string {
		const server = user.yggdrasil_server_url?.trim() ?? "";
		try {
			return new URL(
				server.includes("://") ? server : `https://${server}`,
			).host;
		} catch {
			return server || t("userMenu.server");
		}
	}

	function getUserTypeLabel(
		user: Pick<MinecraftUser, "user_type" | "yggdrasil_server_url">,
	): string {
		const provider = getAccountProvider(user);
		const label = t(`userMenu.accountSetup.${provider}`);
		return provider === "authinject"
			? `${label} · ${getYggdrasilServer(user)}`
			: label;
	}

	function onKeydown(e: KeyboardEvent) {
		if (
			e.key !== "Escape" ||
			e.defaultPrevented ||
			accountProvider ||
			hasOpenModal()
		)
			return;
		e.preventDefault();
		onclose();
	}

	function startEditingName() {
		if (!selectedUser) return;
		editingName = selectedUser.username;
		editingIdx = selectedIdx;
	}

	function cancelEditingName() {
		editingIdx = null;
	}

	const selectedUser = $derived(launcherStore.settings.user[selectedIdx]);
	const selectedUserKey = $derived(
		`${selectedUser?.user_type}:${selectedUser?.uuid || selectedUser?.username}:${selectedUser?.yggdrasil_server_url ?? ""}`,
	);
	const selectedAvatarSvg = $derived(accountAvatar(selectedUser));
	const activeUserIdx = $derived(launcherStore.settings.active_user_idx ?? 0);
	const totalAccounts = $derived(launcherStore.settings.user.length);
</script>

<svelte:window onkeydown={onKeydown} />

<div class="profile-view" inert={accountProvider !== null}>
	<header class="profile-header">
		<h2 class="profile-title">{t("userMenu.title")}</h2>
		<button
			type="button"
			class="close-btn"
			onclick={onclose}
			title={t("userMenu.close")}
			aria-label={t("userMenu.close")}
		>
			<CloseIcon size={18} />
		</button>
	</header>

	<div class="profile-layout">
		<section class="profile-hero">
			{#if selectedUser}
				<div class="hero-content">
					<div class="hero-identity">
						<div class="hero-avatar">
							<AccountAvatar
								avatarSvg={selectedAvatarSvg}
								username={selectedUser.username}
							/>
						</div>
						<div class="hero-meta">
							{#if editingIdx === selectedIdx}
								<div class="hero-edit-row">
									<!-- svelte-ignore a11y_autofocus -->
									<input
										type="text"
										bind:value={editingName}
										maxlength="16"
										autofocus
										class="hero-name-input"
										onkeydown={(e) => {
											if (e.key === "Enter")
												handleSaveName(selectedIdx);
											if (e.key === "Escape")
												cancelEditingName();
										}}
									/>
									<button
										type="button"
										class="btn-icon"
										onclick={() =>
											handleSaveName(selectedIdx)}
									>
										✓
									</button>
									<button
										type="button"
										class="btn-icon"
										onclick={cancelEditingName}
									>
										✕
									</button>
								</div>
							{:else}
								<!-- El nombre y, a su lado, si es la cuenta en uso. -->
								<div class="hero-name-row">
									<h3 class="hero-name">
										{selectedUser.username}
									</h3>
									{#if selectedIdx === activeUserIdx}
										<span class="hero-status">
											{t("userMenu.active")}
										</span>
									{/if}
								</div>
							{/if}
							<span class="hero-type">
								{getUserTypeLabel(selectedUser)}
							</span>
						</div>
					</div>
					<div class="hero-id-actions">
						{#if selectedUser.uuid && selectedUser.user_type !== "Cracked"}
							<button
								type="button"
								class="btn-secondary"
								onclick={refreshSelectedAvatar}
								disabled={refreshingAvatars.has(
									selectedUser.uuid,
								)}
							>
								{refreshingAvatars.has(selectedUser.uuid)
									? t("common.loading")
									: t("userMenu.refreshAvatar")}
							</button>
						{/if}
						{#if selectedUser.user_type === "Microsoft" && selectedIdx === activeUserIdx}
							<button
								type="button"
								class="btn-secondary"
								onclick={handleLogout}
							>
								{t("userMenu.logout")}
							</button>
						{/if}

						{#if selectedUser.user_type === "Cracked" && editingIdx !== selectedIdx}
							<button
								type="button"
								class="btn-secondary"
								onclick={startEditingName}
							>
								{t("userMenu.editName")}
							</button>
						{/if}

						{#if removingUserUuid === selectedUser.uuid}
							<button
								type="button"
								class="btn-primary confirm-yes"
								onclick={() =>
									handleRemoveUser(selectedUser.uuid)}
							>
								{t("userMenu.yes")}
							</button>
							<button
								type="button"
								class="btn-secondary confirm-no"
								onclick={() => (removingUserUuid = null)}
							>
								{t("userMenu.no")}
							</button>
						{:else}
							<button
								type="button"
								class="btn-danger"
								onclick={() =>
									(removingUserUuid = selectedUser.uuid)}
							>
								{t("userMenu.removeUser")}
							</button>
						{/if}
					</div>

					<div class="hero-stats">
						<div class="stat-card">
							<span class="stat-value">{totalAccounts}</span>
							<span class="stat-label">
								{totalAccounts === 1
									? t("userMenu.accountSingular")
									: t("userMenu.accountPlural")}
							</span>
						</div>
						<div class="stat-card">
							<span class="stat-value">
								{t(
									`userMenu.accountSetup.${getAccountProvider(selectedUser)}`,
								)}
							</span>
							<span class="stat-label">{t("userMenu.type")}</span>
						</div>
						{#if selectedUser.user_type === "Yggdrasil"}
							<div class="stat-card wide">
								<span class="stat-value">
									{getYggdrasilServer(selectedUser)}
								</span>
								<span class="stat-label"
									>{t("userMenu.server")}</span
								>
							</div>
						{/if}
					</div>

					{#key selectedUserKey}
						<SkinStorage
							uuid={selectedUser.uuid}
							provider={getAccountProvider(selectedUser)}
						>
							{#snippet server(
								onLocalSaved: () => void,
								onOpenWardrobe: (trigger: HTMLButtonElement) => void,
							)}
								{#if selectedUser.user_type === "Microsoft"}
									<SkinCapeManager
										uuid={selectedUser.uuid}
										{onLocalSaved}
										{onOpenWardrobe}
									/>
								{:else if isElyByAccount(selectedUser)}
									<ElySkinManager
										uuid={selectedUser.uuid}
										{onLocalSaved}
										{onOpenWardrobe}
									/>
								{:else if selectedUser.user_type === "Yggdrasil"}
									<AuthInjectSkinManager
										uuid={selectedUser.uuid}
										{onLocalSaved}
										{onOpenWardrobe}
									/>
								{/if}
							{/snippet}
						</SkinStorage>
					{/key}
				</div>
			{:else}
				<div class="hero-empty">
					<span class="hero-empty-icon">
						<Lupa width="48" height="48" />
					</span>
					<h3 class="hero-empty-title">
						{t("userMenu.noAccountsTitle")}
					</h3>
					<p class="hero-empty-subtitle">
						{t("userMenu.noAccountsSubtitle")}
					</p>
				</div>
			{/if}
		</section>

		<aside class="profile-list">
			{#if launcherStore.settings.user.length > 0}
				<h3 class="list-title">{t("userMenu.savedAccounts")}</h3>
				<div class="account-list">
					{#each launcherStore.settings.user as u, i (i)}
						<AccountListItem
							user={u}
							typeLabel={getUserTypeLabel(u)}
							isActive={i === activeUserIdx}
							isSelected={i === selectedIdx}
							avatarSvg={accountAvatar(u)}
							onselect={() => {
								editingIdx = null;
								// La cuenta se activa al abrirla, sin botón aparte.
								void handleSwitchUser(i);
							}}
						/>
					{/each}
				</div>
			{:else}
				<div class="list-empty">
					<p>{t("userMenu.addFirstAccount")}</p>
				</div>
			{/if}

			<AddAccountCard onselect={openAccountSetup} />
		</aside>
	</div>
</div>

{#if accountProvider}
	<AccountSetup
		provider={accountProvider}
		onsuccess={accountAdded}
		onclose={closeAccountSetup}
	/>
{/if}

<style>
	.profile-view {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.profile-header {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px 28px;
		border-bottom: 1px solid var(--border);
		background: var(--bg-card);
	}

	.profile-title {
		margin: 0;
		font-size: 1rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 1px;
		color: var(--text-primary);
	}

	.close-btn {
		background: transparent;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		border-radius: var(--border-radius-sm);
		padding: 6px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition:
			background 0.15s ease,
			color 0.15s ease,
			border-color 0.15s ease;
	}

	.close-btn:hover {
		background: var(--surface-hover);
		color: var(--text-primary);
		border-color: var(--border-hover);
	}

	.profile-layout {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.profile-hero {
		flex: 3;
		min-width: 0;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		background: var(--bg-main);
		position: relative;
	}

	.hero-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		padding: 24px 28px;
		gap: 20px;
	}

	.hero-identity {
		display: flex;
		align-items: center;
		gap: 16px;
		flex-wrap: wrap;
	}

	.hero-avatar {
		width: 64px;
		height: 64px;
		border-radius: var(--border-radius);
		border: 2px solid var(--bg-main);
		background-color: var(--bg-card);
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.hero-avatar :global(svg),
	.hero-avatar :global(img) {
		width: 100%;
		height: 100%;
		display: block;
		border-radius: inherit;
	}

	.hero-meta {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
		flex: 1;
	}

	.hero-name {
		margin: 0;
		font-size: 1.6rem;
		font-weight: 700;
		color: var(--text-primary);
		line-height: 1.2;
		word-break: break-word;
	}

	.hero-name-row {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		min-width: 0;
	}

	.hero-edit-row {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 4px;
	}

	.hero-name-input {
		font-family: inherit;
		font-size: 1.2rem;
		font-weight: 700;
		color: var(--text-primary);
		background: var(--bg-input);
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		padding: 6px 10px;
		outline: none;
		min-width: 0;
		width: 220px;
	}

	.hero-name-input:focus {
		border-color: var(--text-muted);
	}

	.hero-type {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		overflow-wrap: anywhere;
	}

	.hero-stats {
		display: flex;
		gap: 12px;
		flex-wrap: wrap;
	}

	.stat-card {
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		padding: 14px 18px;
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 110px;
		max-width: 100%;
	}

	.stat-card.wide {
		min-width: 160px;
		flex: 1;
	}

	.stat-value {
		font-size: 1.1rem;
		font-weight: 700;
		color: var(--text-primary);
		overflow-wrap: anywhere;
	}

	.stat-label {
		font-size: 0.65rem;
		font-weight: 700;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.hero-id-actions {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
		flex-shrink: 0;
	}

	.hero-status {
		font-size: 0.65rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.5px;
		background: var(--accent);
		color: var(--accent-text);
		padding: 4px 10px;
		border-radius: 999px;
	}

	.btn-icon {
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--border-radius-sm);
		font-size: 1rem;
		cursor: pointer;
		border: 1px solid transparent;
		background: transparent;
		color: var(--text-secondary);
		transition:
			background 0.15s ease,
			color 0.15s ease;
	}

	.btn-icon:hover {
		background: var(--surface-hover);
		color: var(--text-primary);
	}

	.confirm-yes:hover {
		background: var(--color-success);
		color: var(--bg-main);
		opacity: 1;
	}

	.confirm-no:hover {
		background: var(--color-error);
		color: var(--bg-main);
		opacity: 1;
	}

	.hero-empty {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		color: var(--text-secondary);
		text-align: center;
		padding: 40px;
	}

	.hero-empty-icon {
		opacity: 0.7;
	}

	.hero-empty-title {
		margin: 0;
		font-size: 1.3rem;
		font-weight: 700;
		color: var(--text-primary);
	}

	.hero-empty-subtitle {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}

	.profile-list {
		flex: 2;
		min-width: 300px;
		max-width: 440px;
		background: var(--bg-sidebar);
		border-left: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		padding: 20px;
		gap: 16px;
	}

	.list-title {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 700;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.6px;
	}

	.account-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		flex-shrink: 0;
	}

	.list-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 20px;
		color: var(--text-muted);
		font-size: 0.85rem;
		text-align: center;
		flex: 1;
	}

	@media (max-width: 900px) {
		.profile-layout {
			flex-direction: column;
			overflow-y: auto;
		}

		.profile-hero {
			flex: none;
			min-height: auto;
		}

		.hero-content {
			padding: 20px;
			gap: 18px;
		}

		.hero-avatar {
			width: 56px;
			height: 56px;
		}

		.hero-name {
			font-size: 1.4rem;
		}

		.hero-name-input {
			font-size: 1.1rem;
			width: 180px;
		}

		.hero-identity {
			gap: 14px;
		}

		.profile-list {
			flex: none;
			max-width: none;
			min-width: 0;
			border-left: none;
			border-top: 1px solid var(--border);
			padding: 20px;
			overflow: visible;
		}

		.account-list {
			overflow: visible;
			flex: none;
		}
	}
</style>
