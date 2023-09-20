import { get, writable, type Writable } from "svelte/store";
import {
	CallStatus,
	ChatMessage,
	ChatMessageUI,
	NetworkedPublicKey,
	TransferStatistics,
	UIPeer,
} from "./interfaces";
import {
	get_peer,
	own_public_key,
	peers,
	selected_peer,
	transfer_statistics,
	update_peer,
} from "./stores";
import { tick } from "svelte";

type HandlerReturnType = Promise<void>;
type Handler = (event_data: any) => HandlerReturnType;
export class EventHandler {
	handlers: Map<string, Handler> = new Map();

	add_handler(event_name: string, handler: Handler): EventHandler {
		if (!this.handlers.has(event_name)) {
			this.handlers.set(event_name, handler);
		} else console.error("A handler for this event has already been registered: ", event_name);
		return this;
	}

	async handle(event: any): Promise<void> {
		for (const [event_name, handler] of this.handlers) {
			if (event.payload.hasOwnProperty(event_name)) {
				return await handler(event.payload[event_name]);
			}
		}
		console.error("Failed to find a handler for the given event: ", event);
	}
}

export function build_event_handler(): EventHandler {
	let event_handler = new EventHandler()
		.add_handler("DebugMessage", on_debug_message)
		.add_handler("AnnounceResponse", on_announce_response)
		.add_handler("PeerDisconnected", on_peer_disconnected)
		.add_handler("CallDenied", on_call_denied)
		.add_handler("PunchThroughSuccessfull", on_punchthrough_successfull)
		.add_handler("Call", on_call)
		.add_handler("CallAccepted", on_call_accepted)
		.add_handler("OnChatMessage", on_chat_message)
		.add_handler("OnChatMessageReceived", on_chat_message_received)
		.add_handler("AudioNewInputDevices", on_audio_new_input_devices)
		.add_handler("AudioNewOutputDevices", on_audio_new_output_devices)
		.add_handler("ConnectionStatistics", on_connection_statistics)
		.add_handler("TransferStatistics", on_transfer_statistics);
	return event_handler;
}

async function on_debug_message(debug_data: any) {
	console.log(`${debug_data[1]}: ${debug_data[0]}`);
}

async function on_announce_response(event_peers: any[]): HandlerReturnType {
	for (const public_key of event_peers) {
		if (!get_peer(public_key)) {
			peers.update((p) => {
				let new_peer = new UIPeer(public_key);
				if (p.length == 1) new_peer.selected = true;
				return [...p, new_peer];
			});
		}
	}
}

async function on_peer_disconnected(public_key: any) {
	peers.update((p) => p.filter((peer) => !peer.public_key.equals(public_key)));
}

async function on_punchthrough_successfull(public_key: any) {
	update_peer(public_key, (p) => (p.call_status = CallStatus.PunchthroughSuccessfull));
}

async function on_call_denied(public_key: any) {
	update_peer(public_key, (p) => (p.call_status = CallStatus.RequestFailed));
}

async function on_call(public_key: any) {
	update_peer(public_key, (p) => (p.call_status = CallStatus.WaitingForAnswer));
}

async function on_call_accepted(public_key: any) {
	update_peer(public_key, (p) => (p.call_status = CallStatus.PunchthroughInProgress));
}

async function on_chat_message(msg: ChatMessage) {
	let other_peer: NetworkedPublicKey;
	let _own_public_key = get(own_public_key);
	if (!NetworkedPublicKey.equals(msg.author, _own_public_key)) other_peer = msg.author;
	else if (!NetworkedPublicKey.equals(msg.recipient, _own_public_key)) other_peer = msg.recipient;
	else console.error("Tried sending message to yourself.");
	let new_msg = new ChatMessageUI(msg, NetworkedPublicKey.equals(msg.recipient, _own_public_key));
	await new_msg.generate_absolute_paths();
	update_peer(other_peer, (p) => p.messages.push(new_msg));
}

async function on_chat_message_received(id: string) {
	peers.update((p) => {
		for (const peer of p) {
			for (const msg of peer.messages) {
				if (msg.id === id) msg.received = true;
			}
		}
		return p;
	});
}

async function on_audio_new_input_devices(debug_data: any) {}

async function on_audio_new_output_devices(debug_data: any) {}

async function on_connection_statistics(debug_data: any) {}

async function on_transfer_statistics(new_transfer_statistics: Map<String, TransferStatistics>) {
	transfer_statistics.set(new_transfer_statistics);
}
