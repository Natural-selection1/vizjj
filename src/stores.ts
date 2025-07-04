import type { MutationResult } from "./messages/MutationResult";
import type { RepoConfig } from "./messages/RepoConfig";
import type { RepoStatus } from "./messages/RepoStatus";
import type { RevHeader } from "./messages/RevHeader";
import type { Operand } from "./messages/Operand";
import { writable } from "svelte/store";
import { event, type Query } from "./ipc";
import type { InputRequest } from "./messages/InputRequest";
import type { InputResponse } from "./messages/InputResponse";
import type { RevChange } from "./messages/RevChange";

export const repoConfigEvent = await event<RepoConfig>("vizjj://repo/config", { type: "Initial" });
export const repoStatusEvent = await event<RepoStatus | undefined>(
    "vizjj://repo/status",
    undefined
);
export const revisionSelectEvent = await event<RevHeader | undefined>(
    "vizjj://revision/select",
    undefined
);
export const changeSelectEvent = await event<RevChange | undefined>(
    "vizjj://change/select",
    undefined
);

export const currentMutation = writable<Query<MutationResult> | null>(null);
export const currentContext = writable<Operand | null>();
export const currentSource = writable<Operand | null>();
export const currentTarget = writable<Operand | null>();
export const currentInput = writable<
    (InputRequest & { callback: (response: InputResponse) => void }) | null
>();

export const hasModal = writable<boolean>(false);

export function dragOverWidget(event: DragEvent) {
    event.stopPropagation();
    currentTarget.set(null);
}
