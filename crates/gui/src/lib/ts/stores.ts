import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import { GuiData } from "./interfaces";

export const data: Writable<GuiData> = writable(new GuiData());
