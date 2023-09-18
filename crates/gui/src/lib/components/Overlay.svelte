<script lang="ts">
	import { data } from "../ts/stores";
	import { fade } from "svelte/transition";

	export let dropping: boolean;
</script>

<template lang="pug">
    
    +if('dropping')
	    .overlay(transition:fade="{{duration: 100}}")
            h2 Sending files to: {$data.selected_peer?.public_key.n.slice(0, 10)}

</template>

<style lang="sass">
	.overlay
		background-color: #0000005d
		width: 100%
		height: 100%
		position: absolute
		left: 0
		top: 0
		display: flex
		align-items: center
		justify-content: center
</style>
