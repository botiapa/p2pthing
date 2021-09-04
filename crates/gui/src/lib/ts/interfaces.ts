export class GuiData {
	peers: UIPeer[] = [];
	selected_peer?: UIPeer;
	own_public_key?: NetworkedPublicKey;
	next_msg_id: number = 0;

	/**
		Find the specified peer
	*/
	p(p: NetworkedPublicKey): UIPeer | undefined {
		return this.peers.find((elem) => elem.public_key.equals(p));
	}
}

export interface IPeer {
	public_key: NetworkedPublicKey;
}

export class UIPeer implements IPeer {
	public_key: NetworkedPublicKey;
	call_status: CallStatus = CallStatus.None;
	messages: ChatMessage[] = [];

	constructor(p: IPeer) {
		this.public_key = new NetworkedPublicKey(p.public_key);
	}

	equals(other: UIPeer): boolean {
		return this.public_key.equals(other.public_key);
	}
}

export interface INetworkedPublicKey {
	n: String;
	e: String;
}

export class NetworkedPublicKey implements INetworkedPublicKey {
	n: String;
	e: String;

	constructor(public_key: INetworkedPublicKey) {
		this.n = public_key.n;
		this.e = public_key.e;
	}

	equals(other: NetworkedPublicKey): boolean {
		return this.e == other.e && this.n == other.n;
	}
}

export enum CallStatus {
	None,
	SentRequest,
	PunchthroughInProgress,
	RequestFailed,
	PunchthroughSuccessfull,
	WaitingForAnswer,
}

export class ChatMessage {
	author: NetworkedPublicKey;
	contents: string;
	custom_id?: number;
	received?: boolean;

	constructor(
		author: NetworkedPublicKey,
		contents: string,
		custom_id?: number,
		received?: boolean
	) {
		this.author = author;
		this.contents = contents;
		this.custom_id = custom_id;
		this.received = received;
	}
}
