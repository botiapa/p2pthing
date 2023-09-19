import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { NetworkedPublicKey, UIPeer, TransferStatistics } from "./interfaces";

/**
    Find the specified peer by NetworkedPublicKey
*/
export function get_peer(p: NetworkedPublicKey): UIPeer | undefined {
	return get(peers).find((elem) => elem.public_key.equals(p));
}

/**
    Find the specified peer by NetworkedPublicKey and update it, notifying store subscribers
*/
export function update_peer(pkey: NetworkedPublicKey, cb: (p: UIPeer | null) => void) {
	peers.update((p) => {
		cb(p.find((peer) => peer.public_key.equals(pkey)));
		return p;
	});
}

export const peers: Writable<UIPeer[]> = writable([]);
export const selected_peer: Writable<UIPeer | null> = writable(null);
export const own_public_key: Writable<NetworkedPublicKey | null> = writable(null);
export const transfer_statistics: Writable<Map<String, TransferStatistics>> = writable(new Map());

export const showcased_image: Writable<string> = writable();
