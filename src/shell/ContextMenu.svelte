<script lang="ts">
    import { onMount } from "svelte";
    import { emit } from "@tauri-apps/api/event";
    import type { Operand } from "../messages/Operand";
    import type { StoreRef } from "../messages/StoreRef";

    interface Props {
        operand: Operand;
        x: number;
        y: number;
        onClose: () => void;
    }

    let { operand, x, y, onClose }: Props = $props();

    let menuElement: HTMLElement;

    // Adjust position to keep menu within viewport
    let adjustedX = $state(x);
    let adjustedY = $state(y);

    onMount(() => {
        if (menuElement) {
            const rect = menuElement.getBoundingClientRect();
            if (x + rect.width > window.innerWidth) {
                adjustedX = window.innerWidth - rect.width - 5;
            }
            if (y + rect.height > window.innerHeight) {
                adjustedY = window.innerHeight - rect.height - 5;
            }
        }

        // Close on click outside
        const handleClick = (e: MouseEvent) => {
            if (menuElement && !menuElement.contains(e.target as Node)) {
                onClose();
            }
        };

        // Close on escape
        const handleKeydown = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                onClose();
            }
        };

        // We need to delay attaching the click listener slightly to avoid handling the click that opened the menu
        setTimeout(() => document.addEventListener("click", handleClick), 0);
        document.addEventListener("keydown", handleKeydown);

        return () => {
            document.removeEventListener("click", handleClick);
            document.removeEventListener("keydown", handleKeydown);
        };
    });

    function action(event: string, payload: string) {
        emit(event, payload);
        onClose();
    }

    // --- Logic derived from menu.rs handle_context ---

    // Revision Menu Items
    let isRevision = $derived(operand.type === "Revision");
    let revHeader = $derived(operand.type === "Revision" ? operand.header : null);

    // Conditions
    let revImmutable = $derived(revHeader?.is_immutable ?? false);
    let revWorkingCopy = $derived(revHeader?.is_working_copy ?? false);
    let revSingleParent = $derived((revHeader?.parent_ids.length ?? 0) === 1);

    // Tree (Change) Menu Items
    let isChange = $derived(operand.type === "Change");
    let changeHeader = $derived(operand.type === "Change" ? operand.header : null);

    // Conditions
    let changeImmutable = $derived(changeHeader?.is_immutable ?? false);
    let changeSingleParent = $derived((changeHeader?.parent_ids.length ?? 0) === 1);

    // Ref (Branch) Menu Items
    let isRef = $derived(operand.type === "Ref");
    let refData = $derived(operand.type === "Ref" ? operand.ref : null);

    // Helpers for Ref conditions (matching menu.rs logic)
    function isRemoteBookmark(
        r: StoreRef | null
    ): r is Extract<StoreRef, { type: "RemoteBookmark" }> {
        return r?.type === "RemoteBookmark";
    }

    function isLocalBookmark(
        r: StoreRef | null
    ): r is Extract<StoreRef, { type: "LocalBookmark" }> {
        return r?.type === "LocalBookmark";
    }

    let canTrack = $derived(isRef && isRemoteBookmark(refData) && !refData.is_tracked);

    let canUntrack = $derived(
        isRef &&
            ((isLocalBookmark(refData) && refData.tracking_remotes.length > 0) ||
                (isRemoteBookmark(refData) &&
                    !refData.is_synced &&
                    refData.is_tracked &&
                    !refData.is_absent))
    );

    let canPushAll = $derived(
        isRef &&
            ((isLocalBookmark(refData) && refData.tracking_remotes.length > 0) ||
                (isRemoteBookmark(refData) && refData.is_tracked && refData.is_absent))
    );

    let canPushSingle = $derived(
        isRef && isLocalBookmark(refData) && refData.potential_remotes > 0
    );

    let canFetchAll = $derived(
        isRef &&
            ((isLocalBookmark(refData) && refData.tracking_remotes.length > 0) ||
                (isRemoteBookmark(refData) && (!refData.is_tracked || !refData.is_absent)))
    );

    let canFetchSingle = $derived(
        isRef && isLocalBookmark(refData) && refData.available_remotes > 0
    );

    let canRename = $derived(isRef && isLocalBookmark(refData));

    let canDelete = $derived(
        isRef && !(isRemoteBookmark(refData) && refData.is_absent && refData.is_tracked)
    );
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
    class="context-menu"
    bind:this={menuElement}
    style="top: {adjustedY}px; left: {adjustedX}px;"
    role="menu"
    oncontextmenu={(e) => e.preventDefault()}>
    {#if isRevision}
        <button onclick={() => action("vizjj://context/revision", "new")}>New child</button>
        <button
            disabled={revImmutable || revWorkingCopy}
            onclick={() => action("vizjj://context/revision", "edit")}>Edit as working copy</button>
        <button onclick={() => action("vizjj://context/revision", "backout")}
            >Backout into working copy</button>
        <button onclick={() => action("vizjj://context/revision", "duplicate")}>Duplicate</button>
        <button
            disabled={revImmutable}
            onclick={() => action("vizjj://context/revision", "abandon")}>Abandon</button>
        <div class="separator"></div>
        <button
            disabled={revImmutable || !revSingleParent}
            onclick={() => action("vizjj://context/revision", "squash")}>Squash into parent</button>
        <button
            disabled={revImmutable || !revSingleParent}
            onclick={() => action("vizjj://context/revision", "restore")}
            >Restore from parent</button>
        <div class="separator"></div>
        <button onclick={() => action("vizjj://context/revision", "branch")}
            >Create bookmark</button>
    {:else if isChange}
        <button
            disabled={changeImmutable || !changeSingleParent}
            onclick={() => action("vizjj://context/tree", "squash")}>Squash into parent</button>
        <button
            disabled={changeImmutable || !changeSingleParent}
            onclick={() => action("vizjj://context/tree", "restore")}>Restore from parent</button>
        <div class="separator"></div>
        <button onclick={() => action("vizjj://context/file", "open-with-default-app")}
            >Open with Default App</button>
        <button onclick={() => action("vizjj://context/file", "open-in-explorer")}
            >Open in File Explorer</button>
        <button onclick={() => action("vizjj://context/file", "copy-full-path")}
            >Copy File Path</button>
        <button onclick={() => action("vizjj://context/file", "copy-relative-path")}
            >Copy Relative Path</button>
    {:else if isRef}
        <button disabled={!canTrack} onclick={() => action("vizjj://context/branch", "track")}
            >Track</button>
        <button disabled={!canUntrack} onclick={() => action("vizjj://context/branch", "untrack")}
            >Untrack</button>
        <div class="separator"></div>
        <button disabled={!canPushAll} onclick={() => action("vizjj://context/branch", "push-all")}
            >Push</button>
        <button
            disabled={!canPushSingle}
            onclick={() => action("vizjj://context/branch", "push-single")}
            >Push to remote...</button>
        <button
            disabled={!canFetchAll}
            onclick={() => action("vizjj://context/branch", "fetch-all")}>Fetch</button>
        <button
            disabled={!canFetchSingle}
            onclick={() => action("vizjj://context/branch", "fetch-single")}
            >Fetch from remote...</button>
        <div class="separator"></div>
        <button disabled={!canRename} onclick={() => action("vizjj://context/branch", "rename")}
            >Rename...</button>
        <button disabled={!canDelete} onclick={() => action("vizjj://context/branch", "delete")}
            >Delete</button>
    {/if}
</div>

<style>
    .context-menu {
        position: fixed;
        z-index: 9999;
        background: var(--color-surface-100);
        border: 1px solid var(--color-surface-300);
        box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        min-width: 200px;
        padding: 5px 0;
        display: flex;
        flex-direction: column;
        color: var(--color-surface-900);
        user-select: none;
    }

    button {
        text-align: left;
        padding: 5px 15px;
        background: transparent;
        border: none;
        cursor: pointer;
        display: block;
        width: 100%;
        color: inherit;
        font-size: 13px;
    }

    button:hover:not(:disabled) {
        background: var(--color-primary-500);
        color: white;
    }

    button:disabled {
        opacity: 0.5;
        cursor: default;
    }

    .separator {
        height: 1px;
        background: var(--color-surface-300);
        margin: 4px 0;
    }
</style>
