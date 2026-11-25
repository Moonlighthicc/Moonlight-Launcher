/**
 * Колесо мыши над горизонтальным списком прокручивает его по горизонтали.
 * Жест по горизонтали (трекпад, Shift+колесо) и полоса прокрутки работают как раньше.
 * В RTL (зеркальный режим) направление инвертируется, чтобы колесо вниз вело к тому же
 * концу списка, что и в LTR.
 */
export function horizontalWheel(node: HTMLElement) {
	function onWheel(event: WheelEvent) {
		if (node.scrollWidth <= node.clientWidth) return;
		// Горизонтальный жест уже прокручивает сам браузер.
		if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
		event.preventDefault();
		const rtl = getComputedStyle(node).direction === "rtl";
		node.scrollLeft += rtl ? -event.deltaY : event.deltaY;
	}
	node.addEventListener("wheel", onWheel, { passive: false });
	return {
		destroy() {
			node.removeEventListener("wheel", onWheel);
		},
	};
}
