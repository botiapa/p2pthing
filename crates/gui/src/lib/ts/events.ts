import {
	CallStatus,
	ChatMessage,
	ChatMessageUI,
	GuiData,
	NetworkedPublicKey,
	TransferStatistics,
	UIPeer,
} from "./interfaces";

type Handler = (data: GuiData, event_data: any) => Promise<GuiData | void>;
export class EventHandler {
	handlers: Map<string, Handler> = new Map();

	add_handler(event_name: string, handler: Handler): EventHandler {
		if (!this.handlers.has(event_name)) {
			this.handlers.set(event_name, handler);
		} else console.error("A handler for this event has already been registered: ", event_name);
		return this;
	}

	async handle(data: GuiData, event: any): Promise<GuiData | void> {
		for (const [event_name, handler] of this.handlers) {
			if (event.payload.hasOwnProperty(event_name)) {
				return handler(data, event.payload[event_name]);
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

async function on_debug_message(data: GuiData, debug_data: any) {
	console.log(`${debug_data[1]}: ${debug_data[0]}`);
}

async function on_announce_response(data: GuiData, peers: any[]) {
	for (const public_key of peers) {
		if (!data.p(public_key)) {
			data.peers.push(new UIPeer(public_key));
		}
	}
	return data;
}

async function on_peer_disconnected(data: GuiData, public_key: any) {
	if (data.selected_peer.public_key.equals(public_key)) data.selected_peer = null;
	data.peers = data.peers.filter((peer) => !peer.public_key.equals(public_key));
	return data;
}

async function on_punchthrough_successfull(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.PunchthroughSuccessfull;
	return data;
}

async function on_call_denied(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.RequestFailed;
	return data;
}

async function on_call(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.WaitingForAnswer;
	return data;
}

async function on_call_accepted(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.PunchthroughInProgress;
	return data;
}

async function on_chat_message(data: GuiData, msg: ChatMessage) {
	let other_peer: NetworkedPublicKey;
	if (!NetworkedPublicKey.equals(msg.author, data.own_public_key)) other_peer = msg.author;
	else if (!NetworkedPublicKey.equals(msg.recipient, data.own_public_key))
		other_peer = msg.recipient;
	else console.error("Tried sending message to yourself.");
	let p = data.peers.find((p) => NetworkedPublicKey.equals(p.public_key, other_peer));
	let new_msg = new ChatMessageUI(
		msg,
		NetworkedPublicKey.equals(msg.recipient, data.own_public_key)
	);
	await new_msg.generate_absolute_paths();
	p.messages.push(new_msg);

	return data;
}

async function on_chat_message_received(data: GuiData, id: string) {
	for (const peer of data.peers) {
		for (const msg of peer.messages) {
			if (msg.id === id) msg.received = true;
		}
	}

	return data;
}

async function on_audio_new_input_devices(data: GuiData, debug_data: any) {}

async function on_audio_new_output_devices(data: GuiData, debug_data: any) {}

async function on_connection_statistics(data: GuiData, debug_data: any) {}

async function on_transfer_statistics(
	data: GuiData,
	transfer_statistics: Map<String, TransferStatistics>
) {
	data.transfer_statistics = transfer_statistics;
	return data;
}
