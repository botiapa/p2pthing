<script lang="ts">
	import type { UIPeer } from "src/lib/ts/interfaces";
	import SidebarContact from "./SidebarContact.svelte";
	import { createEventDispatcher } from "svelte";
	import type { Writable } from "svelte/store";
	import { selected_peer, peers, select_peer } from "../ts/stores";

	const dispatch = createEventDispatcher();

	function on_contact_click(new_peer: UIPeer) {
		select_peer(new_peer.public_key);
	}
</script>

<template lang="pug">
	+each('$peers as peer (peer.public_key.n)')
		SidebarContact(peer="{peer}" selected="{peer.public_key.n == $selected_peer?.public_key.n}" on:click="{on_contact_click(peer)}" on:call)
</template>

<style lang="sass">

</style>
