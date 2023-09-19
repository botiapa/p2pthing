<script lang="ts">
	import { showcased_image, selected_peer } from "../ts/stores";
	import { fade } from "svelte/transition";

	export let dropping: boolean;
</script>

<template lang="pug">
	
	+if('dropping')
		.overlay(transition:fade="{{duration: 100}}")
			h2 Sending files to: {$selected_peer?.public_key.n.slice(0, 10)}
		+elseif('$showcased_image')
			.overlay(transition:fade="{{duration: 100}}" on:click!="{() => $showcased_image = null}")
				img(src="{$showcased_image}" alt="{$showcased_image}" on:click|preventDefault!=("{e => e.stopPropagation()}"))

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
	.overlay img
		max-width: 80%
		max-height: 80%
		border-radius: 3px
</style>
