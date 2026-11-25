const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number) {
	if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
	let unit = 0;
	while (bytes >= 1000 && unit < UNITS.length - 1) {
		bytes /= 1000;
		unit++;
	}
	return `${bytes.toLocaleString(undefined, { maximumFractionDigits: 1 })} ${UNITS[unit]}`;
}