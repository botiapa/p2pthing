import { CallStatus, ChatMessage, GuiData, UIPeer } from "./interfaces";

type Handler = (data: GuiData, event_data: any) => GuiData | void;
export class EventHandler {
	handlers: Map<string, Handler> = new Map();

	add_handler(event_name: string, handler: Handler): EventHandler {
		if (!this.handlers.has(event_name)) {
			this.handlers.set(event_name, handler);
		} else console.error("A handler for this event has already been registered: ", event_name);
		return this;
	}

	handle(data: GuiData, event: any): GuiData | void {
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
		.add_handler("ConnectionStatistics", on_connection_statistics);
	return event_handler;
}

function on_debug_message(data: GuiData, debug_data: any) {
	console.log(`${debug_data[1]}: ${debug_data[0]}`);
}

function on_announce_response(data: GuiData, peers: any[]) {
	for (const p of peers) {
		if (!data.p(p.public_key)) {
			data.peers.push(new UIPeer(p));
		}
	}
	return data;
}

function on_peer_disconnected(data: GuiData, public_key: any) {
	if (data.selected_peer.public_key.equals(public_key)) data.selected_peer = null;
	data.peers = data.peers.filter((peer) => !peer.public_key.equals(public_key));
	return data;
}

function on_punchthrough_successfull(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.PunchthroughSuccessfull;
	return data;
}

function on_call_denied(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.RequestFailed;
	return data;
}

function on_call(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.WaitingForAnswer;
	return data;
}

function on_call_accepted(data: GuiData, public_key: any) {
	data.p(public_key).call_status = CallStatus.PunchthroughInProgress;
	return data;
}

function on_chat_message(data: GuiData, ev: any[]) {
	let msg_peer: UIPeer = ev[0];
	let msg: string = ev[1];

	let p = data.peers.find((p) => p.public_key.equals(new UIPeer(msg_peer).public_key));
	p.messages.push(new ChatMessage(p.public_key, msg));
	return data;
}

function on_chat_message_received(data: GuiData, custom_id: number) {
	for (const peer of data.peers) {
		for (const msg of peer.messages) {
			if (msg.custom_id === custom_id) msg.received = true;
		}
	}

	return data;
}

function on_audio_new_input_devices(data: GuiData, debug_data: any) {}

function on_audio_new_output_devices(data: GuiData, debug_data: any) {}

function on_connection_statistics(data: GuiData, debug_data: any) {}
