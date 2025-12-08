<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { emit } from "@tauri-apps/api/event";
    import type { Query } from "../ipc";
    import type { RevResult } from "../messages/RevResult";
    import { onMount } from "svelte";

    interface Props {
        selection: Query<RevResult>;
    }

    let { selection } = $props();

    const appWindow = getCurrentWindow();

    let activeMenu: string | null = $state(null);
    let titleBarElement: HTMLElement;

    // Close menu when clicking outside
    function handleWindowClick(e: MouseEvent) {
        if (activeMenu && !(e.target as Element).closest(".menu-trigger")) {
            activeMenu = null;
        }
    }

    onMount(() => {
        document.addEventListener("click", handleWindowClick);
        return () => document.removeEventListener("click", handleWindowClick);
    });

    function toggleMenu(menu: string) {
        if (activeMenu === menu) {
            activeMenu = null;
        } else {
            activeMenu = menu;
        }
    }

    // Repository Actions
    function repoOpen() {
        invoke("menu_repo_open");
        activeMenu = null;
    }
    function repoReopen() {
        invoke("menu_repo_reopen");
        activeMenu = null;
    }
    function repoClose() {
        appWindow.close();
        activeMenu = null;
    }

    // Revision Logic
    // Derived state to check if revision menu items should be enabled
    let revEnabled = $derived(selection.type === "data" && selection.value.type === "Detail");
    let revImmutable = $derived(
        selection.type === "data" &&
            selection.value.type === "Detail" &&
            selection.value.header.is_immutable
    );
    let revWorkingCopy = $derived(
        selection.type === "data" &&
            selection.value.type === "Detail" &&
            selection.value.header.is_working_copy
    );
    let revParentsCount = $derived(
        selection.type === "data" && selection.value.type === "Detail"
            ? selection.value.header.parent_ids.length
            : 0
    );

    function emitRevision(action: string) {
        emit("vizjj://menu/revision", action);
        activeMenu = null;
    }

    // Edit Actions
    function exec(command: string) {
        document.execCommand(command);
        activeMenu = null;
    }

    function toggleMaximize() {
        appWindow.toggleMaximize();
    }

    function minimize() {
        appWindow.minimize();
    }

    function close() {
        appWindow.close();
    }
</script>

<div class="titlebar">
    <!-- Left: Menus -->
    <div class="menus">
        <!-- Repository Menu -->
        <div class="menu-container">
            <button class="menu-trigger" onclick={() => toggleMenu("repository")}
                >Repository</button>
            {#if activeMenu === "repository"}
                <div class="dropdown">
                    <button onclick={repoOpen}>Open... <span class="shortcut">Ctrl+O</span></button>
                    <button onclick={repoReopen}>Reopen <span class="shortcut">F5</span></button>
                    <div class="separator"></div>
                    <button onclick={repoClose}>Close</button>
                </div>
            {/if}
        </div>

        <!-- Revision Menu -->
        <div class="menu-container">
            <button class="menu-trigger" onclick={() => toggleMenu("revision")}>Revision</button>
            {#if activeMenu === "revision"}
                <div class="dropdown">
                    <button disabled={!revEnabled} onclick={() => emitRevision("new")}
                        >New child <span class="shortcut">Ctrl+N</span></button>
                    <button
                        disabled={!revEnabled || revImmutable || revWorkingCopy}
                        onclick={() => emitRevision("edit")}>Edit as working copy</button>
                    <button disabled={!revEnabled} onclick={() => emitRevision("backout")}
                        >Backout into working copy</button>
                    <button disabled={!revEnabled} onclick={() => emitRevision("duplicate")}
                        >Duplicate</button>
                    <button
                        disabled={!revEnabled || revImmutable}
                        onclick={() => emitRevision("abandon")}>Abandon</button>
                    <div class="separator"></div>
                    <button
                        disabled={!revEnabled || revImmutable || revParentsCount !== 1}
                        onclick={() => emitRevision("squash")}>Squash into parent</button>
                    <button
                        disabled={!revEnabled || revImmutable || revParentsCount !== 1}
                        onclick={() => emitRevision("restore")}>Restore from parent</button>
                    <div class="separator"></div>
                    <button disabled={!revEnabled} onclick={() => emitRevision("branch")}
                        >Create bookmark</button>
                </div>
            {/if}
        </div>

        <!-- Edit Menu -->
        <div class="menu-container">
            <button class="menu-trigger" onclick={() => toggleMenu("edit")}>Edit</button>
            {#if activeMenu === "edit"}
                <div class="dropdown">
                    <button onclick={() => exec("undo")}>Undo</button>
                    <button onclick={() => exec("redo")}>Redo</button>
                    <div class="separator"></div>
                    <button onclick={() => exec("cut")}>Cut</button>
                    <button onclick={() => exec("copy")}>Copy</button>
                    <button onclick={() => exec("paste")}>Paste</button>
                    <button onclick={() => exec("selectAll")}>Select All</button>
                </div>
            {/if}
        </div>
    </div>

    <!-- Center: Title / Drag Region -->
    <div class="drag-region" data-tauri-drag-region>
        <!-- Optional Title if needed -->
    </div>

    <!-- Right: Window Controls -->
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <div class="window-controls">
        <button class="control-btn minimize" onclick={minimize}>
            <svg width="10" height="1" viewBox="0 0 10 1"><path d="M0 0h10v1H0z" /></svg>
        </button>
        <button class="control-btn maximize" onclick={toggleMaximize}>
            <svg width="10" height="10" viewBox="0 0 10 10"
                ><path d="M0 0h10v10H0V0zm1 1v8h8V1H1z" /></svg>
        </button>
        <button class="control-btn close" onclick={close}>
            <svg width="10" height="10" viewBox="0 0 10 10"
                ><path d="M0 0h10v1H0z" transform="rotate(45 5 5)" /><path
                    d="M0 0h10v1H0z"
                    transform="rotate(-45 5 5)" /></svg>
        </button>
    </div>
</div>

<style>
    .titlebar {
        height: 30px;
        background: var(--color-surface-100);
        display: flex;
        align-items: center;
        justify-content: space-between;
        user-select: none;
        font-size: 13px;
        color: var(--color-surface-900);
        border-bottom: 1px solid var(--color-surface-300);
    }

    .menus {
        display: flex;
        height: 100%;
        position: relative;
        z-index: 50; /* Ensure menus are above other content */
    }

    .menu-container {
        position: relative;
        height: 100%;
    }

    .menu-trigger {
        height: 100%;
        padding: 0 10px;
        background: transparent;
        border: none;
        cursor: default;
        color: inherit;
        outline: none;
    }

    .menu-trigger:hover,
    .menu-trigger:focus {
        background: var(--color-surface-200);
    }

    .dropdown {
        position: absolute;
        top: 100%;
        left: 0;
        background: var(--color-surface-100);
        border: 1px solid var(--color-surface-300);
        box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        min-width: 200px;
        padding: 5px 0;
        z-index: 100;
        display: flex;
        flex-direction: column;
    }

    .dropdown button {
        text-align: left;
        padding: 5px 15px;
        background: transparent;
        border: none;
        cursor: pointer;
        display: flex;
        justify-content: space-between;
        color: inherit;
        width: 100%;
    }

    .dropdown button:hover:not(:disabled) {
        background: var(--color-primary-500);
        color: white;
    }

    .dropdown button:disabled {
        opacity: 0.5;
        cursor: default;
    }

    .shortcut {
        opacity: 0.6;
        font-size: 0.9em;
        margin-left: 15px;
    }

    .separator {
        height: 1px;
        background: var(--color-surface-300);
        margin: 4px 0;
    }

    .drag-region {
        flex: 1;
        height: 100%;
    }

    .window-controls {
        display: flex;
        height: 100%;
    }

    .control-btn {
        width: 45px;
        height: 100%;
        background: transparent;
        border: none;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: default;
        fill: currentColor;
    }

    .control-btn:hover {
        background: var(--color-surface-200);
    }

    .control-btn.close:hover {
        background: #e81123;
        color: white;
        fill: white;
    }

    /* Dark mode adjustments - assuming --color-surface-xxx variables handle it,
       but standardizing SVG fill */
    :global(html.dark) .control-btn {
        fill: #ffffff;
    }
    :global(html.dark) .control-btn.close:hover {
        fill: #ffffff;
    }
</style>
