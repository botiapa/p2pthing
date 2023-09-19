import { fs, path } from "@tauri-apps/api";
import { BaseDirectory, appDataDir } from "@tauri-apps/api/path";
import { convertFileSrc } from "@tauri-apps/api/tauri";

export interface IPeer {
	public_key: NetworkedPublicKey;
}

export class UIPeer implements IPeer {
	public_key: NetworkedPublicKey;
	call_status: CallStatus = CallStatus.None;
	messages: ChatMessageUI[] = [];

	constructor(public_key: INetworkedPublicKey) {
		this.public_key = new NetworkedPublicKey(public_key);
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

const downloads_folder: string = "downloads";
export class GuiFile implements IPreparedFile {
	file_id: string;
	file_name: string;
	file_extension: string;
	total_length: number;
	absolute_path?: string;

	private file_contents?: string;

	get_converted_file_name(): string {
		return this.file_id + "." + this.file_extension;
	}

	constructor(f: IPreparedFile) {
		this.file_id = f.file_id;
		this.file_name = f.file_name;
		this.file_extension = f.file_extension;
		this.total_length = f.total_length;
	}

	public async generate_absolute_path() {
		this.absolute_path = convertFileSrc(
			await path.join(await appDataDir(), downloads_folder, this.get_converted_file_name())
		);
	}

	public human_readable_size(): string {
		let size = this.total_length;
		let unit = "B";
		if (size > 1024) {
			size /= 1024;
			unit = "KB";
		}
		if (size > 1024) {
			size /= 1024;
			unit = "MB";
		}
		if (size > 1024) {
			size /= 1024;
			unit = "GB";
		}
		return size.toFixed(2) + " " + unit;
	}

	public async get_file_contents(): Promise<string> {
		if (!this.file_contents) {
			this.file_contents = await fs.readTextFile(
				await path.join(downloads_folder, this.get_converted_file_name()),
				{
					dir: BaseDirectory.AppData,
				}
			);
		}
		return this.file_contents;
	}
}

export class ChatMessage {
	id: string;
	author: NetworkedPublicKey;
	recipient: NetworkedPublicKey;
	msg: string;
	attachments: GuiFile[] | undefined;
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
		this.attachments = attachments?.map((f) => new GuiFile(f));
		this.dt = dt;
	}

	async generate_absolute_paths() {
		if (this.attachments) {
			for (const i in this.attachments) {
				await this.attachments[i].generate_absolute_path();
			}
		}
	}
}

export class ChatMessageUI extends ChatMessage {
	received: boolean;

	constructor(msg: ChatMessage, received: boolean) {
		super(msg.id, msg.author, msg.recipient, msg.msg, msg.attachments, msg.dt);
		this.received = received;
	}
}

export enum TransferState {
	Transfering = "Transfering",
	Complete = "Complete",
}

export class TransferStatistics {
	started: Date;
	bytes_written: number;
	bytes_read: number;
	state: TransferState;
}
