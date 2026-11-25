import { SvelteSet } from "svelte/reactivity";

/**
 * Экземпляры, у которых окно Minecraft уже появилось (сигнал `InstanceGameReady`).
 * Пока экземпляр запускается, а его ID здесь нет, под кнопкой «Играть» показывается индикатор.
 */
const ready = new SvelteSet<string>();
/**
 * Экземпляры, которые запущены из этого сеанса лаунчера. Игры, найденные после
 * перезапуска лаунчера, сюда не попадают: их окно уже существует, индикатор не нужен.
 */
const launching = new SvelteSet<string>();

export function markLaunching(id: string) {
	launching.add(id);
}

export function isLaunching(id: string): boolean {
	return launching.has(id);
}

export function markGameReady(id: string) {
	ready.add(id);
	launching.delete(id);
}

export function clearGameReady(id: string) {
	ready.delete(id);
}

export function isGameReady(id: string): boolean {
	return ready.has(id);
}
