import { onMount } from "svelte";

/**
 * Для окон, которые монтируются сразу в открытом состоянии (лениво подгруженные
 * компоненты, скелетоны). Svelte не проигрывает intro-переход для блока, созданного
 * в момент монтирования, поэтому возвращаем `ready = true` на следующем кадре:
 * блок с переходом создаётся уже после монтирования и анимация играет.
 */
export function createMountReady() {
	let ready = $state(false);
	onMount(() => {
		const id = requestAnimationFrame(() => {
			ready = true;
		});
		return () => cancelAnimationFrame(id);
	});
	return {
		get ready() {
			return ready;
		},
	};
}
