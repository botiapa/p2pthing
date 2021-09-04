<script lang="ts">
	import type { ChatMessage, UIPeer } from "../ts/interfaces";

	import { data } from "../ts/stores";

	export let message: ChatMessage;
	export let name_visible: boolean;
</script>

<template lang="pug">
	#container
        +if("name_visible")
            .author(class:unread="{message.received === false}") {message.author.n.slice(0,10)}
        .contents(class:unread="{message.received === false}") {message.contents}
</template>

<style lang="sass">
    .unread
        opacity: 50%
        font-weight: normal !important

    .author
        font-weight: 700
    
    #container
        margin-top: 3px
</style>
