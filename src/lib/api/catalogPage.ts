const ATTEMPT_TIMEOUT = 8_000;
const RETRY_DELAYS = [400, 1_200];

function isTransient(error: unknown) {
	const message = error instanceof Error ? error.message : String(error);
	return (
		/^(?:Market request timed out|Too many pending market requests|HTTP request failed:|Failed to read response:)/i.test(
			message,
		) || /^API error (?:408|429|5\d{2})\b/i.test(message)
	);
}

function wait(delay: number, signal: AbortSignal) {
	signal.throwIfAborted();
	return new Promise<void>((resolve, reject) => {
		const timer = setTimeout(() => {
			signal.removeEventListener("abort", abort);
			resolve();
		}, delay);
		function abort() {
			clearTimeout(timer);
			reject(signal.reason);
		}
		signal.addEventListener("abort", abort, { once: true });
	});
}

/** A failed page is retried in place, never concurrently with another attempt. */
export async function loadCatalogPage<T>(
	request: (signal: AbortSignal) => Promise<T>,
	signal: AbortSignal,
): Promise<T> {
	for (let attempt = 0; ; attempt++) {
		signal.throwIfAborted();
		const timeout = new AbortController();
		const attemptSignal = AbortSignal.any([signal, timeout.signal]);
		const timer = setTimeout(
			() => timeout.abort(new Error("Market request timed out")),
			ATTEMPT_TIMEOUT,
		);
		try {
			const result = await request(attemptSignal);
			attemptSignal.throwIfAborted();
			return result;
		} catch (error) {
			signal.throwIfAborted();
			const failure = timeout.signal.aborted
				? timeout.signal.reason
				: error;
			if (attempt >= RETRY_DELAYS.length || !isTransient(failure))
				throw failure;
		} finally {
			clearTimeout(timer);
		}
		await wait(RETRY_DELAYS[attempt], signal);
	}
}
