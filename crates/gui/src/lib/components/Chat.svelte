<script lang="ts">
	import { invoke } from "@tauri-apps/api/tauri";

	import { data } from "../ts/stores";
	import ChatMessage from "./ChatMessage.svelte";
	import { ChatMessage as ChatMessageClass } from "../ts/interfaces";

	let msg_input: string = "";

	$: shortname = $data.selected_peer?.public_key.n.slice(0, 10);

	async function on_key_up(event) {
		if (event.key !== "Enter" || msg_input.trim() == "") return;
		await invoke("send_event", {
			event: {
				SendChatMessage: [$data.selected_peer?.public_key, msg_input, []],
			},
		});
		msg_input = "";
		event.preventDefault(); // No need to `return false;`.
	}

	$: console.log("Messages: ", $data.selected_peer?.messages);

	let container;
</script>

<template lang="pug">
	#container(bind:this="{container}")
        #header
            div {shortname}
        #chat-box
            #chat-messages
                +each('$data.selected_peer?.messages as message, i')
                    ChatMessage(message="{message}" name_visible="{!$data.selected_peer?.messages[i-1]?.author.equals(message.author)}")
        input#chat-input(placeholder="Message {shortname}" bind:value="{msg_input}" on:keyup="{on_key_up}")
</template>

<style lang="sass">
    #container
        display: flex
        flex-direction: column
        height: 100%
        background-color: #ffffff0D

    #header
        background-color: #ffffff0D
        border-radius: 3px
        padding: 10px

    #chat-box
        overflow: hidden
        position: relative
        height: 100%
        width: 100%

        #chat-messages
            position: absolute
            bottom: 0
            border-radius: 3px
            overflow: auto
            width: 100%
            max-height: 100%
            padding: 5px

    #chat-input
        background: #ffffff0D
        border-color: transparent
        border-radius: 5px
        margin: 5px
        height: 30px
        padding: 5px
        color: white

        &:focus-visible
            outline: 0px

</style>
