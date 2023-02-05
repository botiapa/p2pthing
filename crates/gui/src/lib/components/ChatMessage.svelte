<script lang="ts">
	import type { ChatMessage, UIPeer } from "../ts/interfaces";

	import { data } from "../ts/stores";
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
                p {file.file_name}
                +if('$data.transfer_statistics[file.file_id]')
                    +if('$data.transfer_statistics[file.file_id]?.state == "Complete" && file.absolute_path')
                        img.attachment(src="{file.absolute_path}")
                        p Bytes read {$data.transfer_statistics[file.file_id]?.bytes_read}
                        p Bytes written {$data.transfer_statistics[file.file_id]?.bytes_written}
                        p Started {$data.transfer_statistics[file.file_id]?.started.toString()}
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
</style>
