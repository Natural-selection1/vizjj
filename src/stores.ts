import type { MutationResult } from "./messages/MutationResult";
import type { RepoConfig } from "./messages/RepoConfig";
import type { RepoStatus } from "./messages/RepoStatus";
import type { RevHeader } from "./messages/RevHeader";
import type { Operand } from "./messages/Operand";
import { writable, derived } from "svelte/store";
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

export const currentTheme = writable<"light" | "dark">("light");
export const unifiedTheme = derived(
    [currentTheme, repoConfigEvent],
    ([$currentTheme, $repoConfigEvent]) => {
        const baseTheme = $currentTheme;
        const themeOverride =
            $repoConfigEvent?.type === "Workspace" ? $repoConfigEvent.theme_override : null;
        return themeOverride || baseTheme;
    }
);

export function toggleTheme() {
    currentTheme.update((theme) => {
        const newTheme = theme === "light" ? "dark" : "light";
        applyTheme(newTheme);
        localStorage.setItem("theme", newTheme);
        return newTheme;
    });
}

function applyTheme(theme: string) {
    const html = document.documentElement;
    html.className = theme;
}

export function initTheme() {
    const savedTheme = localStorage.getItem("theme") as "light" | "dark" | null;
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    const theme = savedTheme || systemTheme;
    currentTheme.set(theme);
    applyTheme(theme);
}

// theme change listener
unifiedTheme.subscribe((theme) => {
    if (theme) {
        applyTheme(theme);
    }
});

export function dragOverWidget(event: DragEvent) {
    event.stopPropagation();
    currentTarget.set(null);
}
