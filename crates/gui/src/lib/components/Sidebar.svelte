<script lang="ts">
	import type { GuiData, UIPeer } from "src/lib/ts/interfaces";
	import SidebarContact from "./SidebarContact.svelte";
	import { createEventDispatcher } from "svelte";
	import type { Writable } from "svelte/store";
	import { data } from "../ts/stores";

	const dispatch = createEventDispatcher();

	function on_contact_click(new_peer) {
		$data.selected_peer = new_peer;
	}
</script>

<template lang="pug">
	+each('$data.peers as peer (peer.public_key.n)')
		SidebarContact(peer="{peer}" selected="{peer.public_key.n == $data.selected_peer?.public_key.n}" on:click="{on_contact_click(peer)}" on:call)
</template>

<style lang="sass">

</style>
