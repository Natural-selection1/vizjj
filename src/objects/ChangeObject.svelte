<script lang="ts">
    import type { RevChange } from "../messages/RevChange";
    import type { RevHeader } from "../messages/RevHeader";
    import type { Operand } from "../messages/Operand";
    import Icon from "../controls/Icon.svelte";
    import Object from "./Object.svelte";
    import Zone from "./Zone.svelte";
    import { changeSelectEvent } from "../stores";

    interface Props {
        header: RevHeader;
        change: RevChange;
        selected: boolean;
    }
    let { header, change, selected }: Props = $props();

    let operand: Operand = { type: "Change", header, path: change.path };

    let icon = $state("file");
    let fileState: "add" | "change" | "remove" | null = $state(null);
    let stats = $derived(calculateStats(change));

    switch (change.kind) {
        case "Added":
            icon = "file-plus";
            fileState = "add";
            break;
        case "Deleted":
            icon = "file-minus";
            fileState = "remove";
            break;
        case "Modified":
            icon = "file";
            fileState = "change";
            break;
    }

    function calculateStats(change: RevChange): { additions: number; deletions: number } {
        let additions = 0;
        let deletions = 0;
        for (const hunk of change.hunks) {
            for (const line of hunk.lines.lines) {
                if (line.startsWith("+")) {
                    additions++;
                } else if (line.startsWith("-")) {
                    deletions++;
                }
            }
        }
        return { additions, deletions };
    }

    function onSelect() {
        changeSelectEvent.set(change);
    }
</script>

<Object
    {operand}
    {selected}
    suffix={change.path.repo_path}
    conflicted={change.has_conflict}
    label={change.path.relative_path}
    click={onSelect}>
    {#snippet children({ context, hint })}
        <Zone {operand}>
            {#snippet children({ target })}
                <div class="layout" class:target>
                    <div class="file-info">
                        <Icon name={icon} state={context ? null : fileState} />
                        <span>{hint ?? change.path.relative_path}</span>
                    </div>
                    {#if stats.additions > 0 || stats.deletions > 0}
                        <div class="stats">
                            {#if stats.additions > 0}
                                <span class="additions">+{stats.additions}</span>
                            {/if}
                            {#if stats.deletions > 0}
                                <span class="deletions">-{stats.deletions}</span>
                            {/if}
                        </div>
                    {/if}
                </div>
            {/snippet}
        </Zone>
    {/snippet}
</Object>

<style>
    @reference "tailwindcss";

    .layout {
        height: calc(var(--app-font-size) * 1.5);
        display: flex;
        align-items: center;
        justify-content: space-between;
        cursor: pointer;
        padding: 0 3px;
        gap: 6px;
    }

    .layout.target {
        background: var(--ctp-flamingo);
        color: black;
    }

    .file-info {
        display: flex;
        align-items: center;
        gap: 6px;
        overflow: hidden;
        flex: 1;
    }

    .stats {
        display: flex;
        align-items: center;
        gap: 4px;
        font-weight: 500;
        margin-left: 8px;
        flex-shrink: 0;
    }

    .additions {
        color: light-dark(var(--color-emerald-700), var(--color-green-400));
    }

    .deletions {
        color: light-dark(var(--color-red-600), var(--color-red-400));
    }
</style>
