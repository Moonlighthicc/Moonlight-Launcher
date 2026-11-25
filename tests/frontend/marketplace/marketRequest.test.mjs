import { expect, test } from "bun:test";
import { fileURLToPath } from "node:url";

test("native market bridge unregisters abort listeners on completion, cancellation and failure", () => {
	// Isolate the Tauri mock from suites that exercise the real API wrappers.
	const result = Bun.spawnSync([
		process.execPath,
		fileURLToPath(new URL("./fixtures/marketRequest.mjs", import.meta.url)),
	]);
	expect(result.exitCode, result.stderr.toString()).toBe(0);
});
