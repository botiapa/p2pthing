<script lang="ts">
	import type { ChatMessage, UIPeer } from "../ts/interfaces";

	import { data } from "../ts/stores";
	import { path } from "@tauri-apps/api";

	export let message: ChatMessage;
	export let name_visible: boolean;

	function convertFileSrc(filePath: string): string {
		return navigator.userAgent.includes("Windows")
			? `https://asset.localhost/${filePath}`
			: `asset://${filePath}`;
	}

	async function convert_attachment_file_name(fileName: string, ext: string): Promise<string> {
		let curr_dir = await path.currentDir();
		let full_path = curr_dir + "downloads\\" + fileName + "." + ext;
		console.log(full_path);
		return Promise.resolve(convertFileSrc(full_path));
	}

	$: console.log(`asd: `, $data.transfer_statistics);
</script>

<template lang="pug">
	#container
        +if("name_visible")
            .author(class:unread="{message.received === false}") {message.author.n.slice(0,10)}
        .contents(class:unread="{message.received === false}") {message.msg}
        +if("message.attachments")
            +each('message.attachments as file (file.file_id)')
                p {file.file_name}
                +await('convert_attachment_file_name(file.file_id, file.file_extension) then path')
                    img.attachment(src="{path}")
                    p Bytes read {$data.transfer_statistics[file.file_id]?.bytes_read}
                    p Bytes written {$data.transfer_statistics[file.file_id]?.bytes_written}
                    p Started {$data.transfer_statistics[file.file_id]?.started}
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
