<script lang="ts">
	import Sidebar from "./lib/components/Sidebar.svelte";
	import Chat from "./lib/components/Chat.svelte";
	import Overlay from "./lib/components/Overlay.svelte";

	import { emit, listen } from "@tauri-apps/api/event";
	import { onDestroy, onMount } from "svelte";
	import { CallStatus, NetworkedPublicKey, UIPeer } from "./lib/ts/interfaces";
	import { build_event_handler } from "./lib/ts/events";
	import { get, writable } from "svelte/store";
	import { invoke } from "@tauri-apps/api/tauri";
	import { unlisten_all } from "./lib/ts/helpers";
	import { resourceDir } from "@tauri-apps/api/path";
	import { selected_peer, own_public_key } from "./lib/ts/stores";

	let sidebar;
	let dropping: boolean = false;

	const handler = build_event_handler();

	function main() {
		const unl1 = listen("client-event", async (event) => {
			await handler.handle(event);
		});

		const unl2 = listen("tauri://file-drop-hover", async (e) => {
			const validPaths = e.payload as string[];
			if (validPaths.length > 0 && $selected_peer) {
				dropping = true;
			}
		});

		const unl3 = listen("tauri://file-drop", (e) => {
			const paths = e.payload as string[];
			console.log("Dropping file(s):", paths);
			if (paths.length > 0 && $selected_peer) {
				invoke("send_event", {
					event: { SendChatMessage: [$selected_peer?.public_key, "", paths] },
				});
			}
			dropping = false;
		});

		const unl4 = listen("tauri://file-drop-cancelled", (e) => {
			dropping = false;
		});

		invoke("get_own_public_key").then((public_key: NetworkedPublicKey) => {
			own_public_key.set(new NetworkedPublicKey(public_key));

			emit("gui-started");
		});

		console.log("Started main");

		return unlisten_all([unl1, unl2, unl3, unl4]);
	}

	onMount(() => {
		return main();
	});
</script>

<template lang="pug">
	#main
		#sidebar: Sidebar(bind:this="{sidebar}")
		+if('$selected_peer?.call_status == CallStatus.PunchthroughSuccessfull')
			#chat: Chat
	Overlay(dropping="{dropping}")

</template>

<style lang="sass">
	:root
		font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif
		background-color: #121212
		color: white

	#main
		$padding: 10px
		display: flex
		flex-direction: row
		height: calc(100% - #{$padding*2})
		padding: $padding

	#sidebar
		width: 200px
		margin-right: 10px
	
	#chat
		width: 100%
		height: 100%
		border-radius: 5px

</style>
