<script lang="ts">
	import type { ChatMessage, UIPeer } from "../ts/interfaces";

	import { data, showcased_image } from "../ts/stores";
	import { path } from "@tauri-apps/api";

	export let message: ChatMessage;
	export let name_visible: boolean;

	$: console.log(`transfers: `, $data.transfer_statistics);
</script>

<template lang="pug">
	#container
        +if("name_visible")
            .author(class:unread="{message.received === false}") {message.author.n.slice(0,10)}
        .contents(class:unread="{message.received === false}") {message.msg}
        +if("message.attachments")
            +each('message.attachments as file (file.file_id)')
                +if('$data.transfer_statistics[file.file_id]')
                    +if('($data.transfer_statistics[file.file_id]?.state == "Complete" || message.author.equals($data.own_public_key)) && file.absolute_path')
                        +if('file.file_name.endsWith(".png") || file.file_name.endsWith(".jpg") || file.file_name.endsWith(".jpeg")')
                            img.attachment(src="{file.absolute_path}" alt="{file.file_name}" on:click!="{() => $showcased_image = file.absolute_path}")
                            +elseif('file.file_name.endsWith(".mp4") || file.file_name.endsWith(".webm")')
                                video.attachment(src="{file.absolute_path}" alt="{file.file_name}")
                            +else
                                a.attachment(href="{file.absolute_path}" download="{file.file_name}") {file.file_name}
                        +else
                            progress(value="{$data.transfer_statistics[file.file_id].bytes_written/file.total_length}")
                    

</template>

<style lang="sass">
    .unread
        opacity: 50%
        font-weight: normal !important

    .author
        font-weight: 700
    
    #container
        margin-top: 3px
    
    .attachment
        max-height: 15vw
        cursor: pointer
</style>
