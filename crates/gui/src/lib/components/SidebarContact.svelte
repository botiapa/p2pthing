<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import { CallStatus, UIPeer } from "../ts/interfaces";
	import avatar from "../../assets/user-avatar.svg";
	import call_icon from "../../assets/call.svg?raw";
	import accept_icon from "../../assets/checkmark.svg?raw";
	import deny_icon from "../../assets/close.svg?raw";
	import { invoke } from "@tauri-apps/api/tauri";
	import { get_peer, peers, update_peer } from "../ts/stores";

	export let peer: UIPeer;
	export let selected: boolean;

	const dispatch = createEventDispatcher();

	$: shortname = peer.public_key.n.slice(0, 10);

	let status_class = ".status-none";
	$: {
		switch (peer.call_status) {
			case CallStatus.None:
				status_class = "status-none";
				break;
			case CallStatus.PunchthroughInProgress:
				status_class = "status-progress";
				break;
			case CallStatus.PunchthroughSuccessfull:
				status_class = "status-successfull";
				break;
			case CallStatus.RequestFailed:
				status_class = "status-failed";
				break;
			case CallStatus.SentRequest:
				status_class = "status-sent";
				break;
			case CallStatus.WaitingForAnswer:
				status_class = "status-sent";
				break;
		}
	}

	function on_call() {
		invoke("send_event", {
			event: { Call: peer.public_key },
		});
		update_peer(peer.public_key, (p) => (p.call_status = CallStatus.SentRequest));
	}

	function on_accept() {
		invoke("send_event", {
			event: { CallAccepted: peer.public_key },
		});
		update_peer(peer.public_key, (p) => (p.call_status = CallStatus.PunchthroughInProgress));
	}

	function on_deny() {
		invoke("send_event", {
			event: { CallDenied: peer.public_key },
		});
		update_peer(peer.public_key, (p) => (p.call_status = CallStatus.RequestFailed));
	}
</script>

<template lang="pug">
	.contact(on:click class:selected="{selected}" class="{status_class}")
		.avatar
			img(src="{avatar}" alt="Avatar")
			.status
		.name {shortname}
		.controls
			+if('peer.call_status == CallStatus.WaitingForAnswer')
				.icon(on:click="{on_accept}") {@html accept_icon}
				.icon(on:click="{on_deny}") {@html deny_icon}
				+elseif('peer.call_status == CallStatus.None')
					.icon(on:click="{on_call}") {@html call_icon}

</template>

<style lang="sass">
	@use "../sass/_globals" as *
	@use "sass:math"

	$status-color-base: gray
	$status-color-sent: yellow
	$status-color-inprogress: aqua
	$status-color-successfull: $neon-green
	$status-color-failed: red

	.contact
		background-color: #ffffff0D
		margin: 3px
		padding: 5px
		border-radius: 3px
		display: flex
		width: 180px
		transition: background 0.1s ease-in

		&:hover
			background: #ffffff12
		
		&.selected
			background: #ffffff1A
	
		.avatar
			$avatar-size: 30px
			$status-size : 10px
			display: flex
			position: relative
			align-items: center

			img
				width: $avatar-size
				height: $avatar-size
				margin-right: 10px
				border-radius: 50%
		
			.status
				width: $status-size
				height: $status-size
				border-radius: 50%
				position: absolute
				top: math.div($avatar-size, 2) + math.div($avatar-size, 8)
				left: math.div($avatar-size, 2) + math.div($avatar-size, 8)
				display: block
		
		&.status-none
			.avatar
				.status
					background-color: $status-color-base
		
		&.status-progress
			.avatar
				.status
					background-color: $status-color-inprogress
		
		&.status-successfull
			.avatar
				.status
					background-color: $status-color-successfull
		
		&.status-failed
			.avatar
				.status
					background-color: $status-color-failed
		
		&.status-sent
			.avatar
				.status
					background-color: $status-color-sent

		.name
			text-align: center
			display: flex
			align-items: center
	
	.controls
		display: flex
		align-items: center
		justify-content: flex-end
		width: 100%

		.icon
			fill: white
			width: 15px !important
			height: 15px !important
			transition: fill 0.1s ease-in

		.icon:hover
			fill: $neon_green
</style>
