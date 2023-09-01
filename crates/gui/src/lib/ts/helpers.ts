import type { event } from "@tauri-apps/api";

export function unlisten_all(futureUnlisteners: Promise<event.UnlistenFn>[]) {
	return async () => {
		for (let fn of futureUnlisteners) {
			await extractUnlistener(fn)();
		}
	};
}

// Thank you: https://github.com/probablykasper/mr-tagger/blob/444d69728071b7b1d55b3368fac759ef002f50b6/src/scripts/helpers.ts
function extractUnlistener(futureUnlistener: Promise<event.UnlistenFn>) {
	return async () => {
		const unlisten = await futureUnlistener;
		unlisten();
	};
}
