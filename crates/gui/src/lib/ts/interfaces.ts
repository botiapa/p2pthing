export class GuiData {
	peers: UIPeer[] = [];
	selected_peer?: UIPeer;
	own_public_key?: NetworkedPublicKey;
	transfer_statistics: Map<String, TransferStatistics> = new Map();

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
	messages: ChatMessageUI[] = [];

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

	static equals(that: NetworkedPublicKey, other: NetworkedPublicKey): boolean {
		return that.e == other.e && that.n == other.n;
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

export interface IPreparedFile {
	file_id: string;
	file_name: string;
	file_extension: string;
	total_length: number;
}

export class ChatMessage {
	id: string;
	author: NetworkedPublicKey;
	recipient: NetworkedPublicKey;
	msg: string;
	attachments: IPreparedFile[] | undefined;
	dt: Date;

	constructor(
		id: string,
		author: INetworkedPublicKey,
		recipient: INetworkedPublicKey,
		msg: string,
		attachments: IPreparedFile[],
		dt: Date
	) {
		this.id = id;
		this.author = new NetworkedPublicKey(author);
		this.recipient = new NetworkedPublicKey(recipient);
		this.msg = msg;
		this.attachments = attachments;
		this.dt = dt;
	}
}

export class ChatMessageUI extends ChatMessage {
	received: boolean;

	constructor(msg: ChatMessage, received: boolean) {
		super(msg.id, msg.author, msg.recipient, msg.msg, msg.attachments, msg.dt);
		this.received = received;
	}
}

export class TransferStatistics {
	started: Date;
	bytes_written: number;
	bytes_read: number;
}
