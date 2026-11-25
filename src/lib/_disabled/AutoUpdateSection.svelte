<!--
	DESACTIVADO: sección «Actualizaciones del launcher» (casilla de actualización
	automática + canal + comprobación manual). Se retiró de los ajustes hasta que
	se apruebe el proyecto en GitHub; el código queda aquí para volver a activarlo.

	Para reactivarlo, en src/lib/components/settings/Settings.svelte:
	  1. importa:  import AutoUpdateSection from "$lib/_disabled/AutoUpdateSection.svelte";
	  2. coloca en la pestaña «launcher»:  <AutoUpdateSection onsave={handleSave} />
-->
<script lang="ts">
	import { t } from "$lib/i18n";
	import { launcherStore } from "$lib/state/state.svelte";
	import CollapsibleSection from "$lib/components/settings/CollapsibleSection.svelte";
	import UpdateSection from "./UpdateSection.svelte";

	let { onsave }: { onsave: () => void | Promise<void> } = $props();
</script>

<CollapsibleSection
	title={t("settings.launcher.updatesTitle")}
	iconName="ui:download"
	storageKey="section_updates"
>
	<div class="qm-field-checkbox">
		<input
			type="checkbox"
			id="auto-updates"
			bind:checked={launcherStore.settings.auto_updates}
			onchange={onsave}
		/>
		<label for="auto-updates">{t("settings.launcher.autoUpdates")}</label>
	</div>
	<UpdateSection />
</CollapsibleSection>
