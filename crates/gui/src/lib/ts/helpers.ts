import { event, path } from "@tauri-apps/api";

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

function convertFileSrc(filePath: string): string {
	return navigator.userAgent.includes("Windows")
		? `https://asset.localhost/${filePath}`
		: `asset:/${filePath}`;
}

export async function convert_attachment_file_name(
	relative_folder: string,
	fileName: string,
	ext: string
): Promise<string> {
	let full_path = "./" + relative_folder + fileName + "." + ext;
	console.log(full_path);
	return Promise.resolve(convertFileSrc(full_path));
}
