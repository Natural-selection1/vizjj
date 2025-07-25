<script lang="ts">
    import ActionWidget from "./controls/ActionWidget.svelte";
    import AuthorSpan from "./controls/AuthorSpan.svelte";
    import CheckWidget from "./controls/CheckWidget.svelte";
    import Icon from "./controls/Icon.svelte";
    import IdSpan from "./controls/IdSpan.svelte";
    import ListWidget, { type List } from "./controls/ListWidget.svelte";
    import { onEvent } from "./ipc";
    import type { ChangeId } from "./messages/ChangeId";
    import type { CommitId } from "./messages/CommitId";
    import type { RevChange } from "./messages/RevChange";
    import type { RevResult } from "./messages/RevResult";
    import RevisionMutator from "./mutators/RevisionMutator";
    import ChangeObject from "./objects/ChangeObject.svelte";
    import RevisionObject from "./objects/RevisionObject.svelte";
    import Zone from "./objects/Zone.svelte";
    import Pane from "./shell/Pane.svelte";
    import { changeSelectEvent, dragOverWidget } from "./stores";

    let { rev }: { rev: Extract<RevResult, { type: "Detail" }> } = $props();

    let shortChangeId = $derived(getShortId(rev.header.id.change));
    let standardChangeId = $derived(getStandardId(rev.header.id.change));
    let shortCommitId = $derived(getShortId(rev.header.id.commit));
    let standardCommitId = $derived(getStandardId(rev.header.id.commit));

    const currentDescription = $derived(rev.header.description.lines.join("\n"));
    let mutator = $state(new RevisionMutator(rev.header));
    let fullDescription = $derived(currentDescription);
    let descriptionChanged = $derived(fullDescription !== currentDescription);
    let resetAuthor = $state(false);
    let unresolvedConflicts = $derived.by(() =>
        rev.conflicts.filter(
            (conflict) =>
                rev.changes.findIndex(
                    (change) =>
                        !change.has_conflict && change.path.repo_path == conflict.path.repo_path
                ) == -1
        )
    );
    let syntheticChanges = $derived.by(() =>
        rev.changes
            .concat(
                unresolvedConflicts.map((conflict) => ({
                    kind: "None",
                    path: conflict.path,
                    has_conflict: true,
                    hunks: [conflict.hunk],
                }))
            )
            .sort((a, b) => a.path.relative_path.localeCompare(b.path.relative_path))
    );
    let selectedChange = $changeSelectEvent;
    let unset = $derived.by(() => {
        for (let change of syntheticChanges) {
            if (selectedChange?.path?.repo_path === change.path.repo_path) {
                return false;
            }
        }
        return true;
    });

    $effect(() => {
        if (unset) {
            changeSelectEvent.set(syntheticChanges[0]);
        }
    });

    let list: List = {
        getSize() {
            return syntheticChanges.length;
        },
        getSelection() {
            return (
                syntheticChanges.findIndex(
                    (row) => row.path.repo_path == $changeSelectEvent?.path.repo_path
                ) ?? -1
            );
        },
        selectRow(row: number) {
            $changeSelectEvent = syntheticChanges[row];
        },
        editRow(row: number) {},
    };

    onEvent<string>("vizjj://menu/revision", (event) => mutator.handle(event));

    function updateDescription() {
        mutator.onDescribe(fullDescription, resetAuthor);
    }

    function minLines(change: RevChange): number {
        if (change.hunks.length === 1) {
            return change.hunks[0].lines.lines.length + 2;
        } else {
            let total = 0;
            for (let hunk of change.hunks) {
                total += hunk.lines.lines.length + 2;
            }
            return Math.floor(total / 2);
        }
    }

    function lineColour(line: string): string | null {
        if (line.startsWith("+")) {
            return "add";
        } else if (line.startsWith("-")) {
            return "remove";
        } else {
            return null;
        }
    }

    function getShortId(id: ChangeId | CommitId) {
        return id.prefix + id.rest.substring(0, 2);
    }
    function getStandardId(id: ChangeId | CommitId) {
        return id.prefix + id.rest.substring(0, 8 - id.prefix.length);
    }
    function copyId(id: ChangeId | CommitId, event: MouseEvent) {
        if (event.ctrlKey || event.metaKey) {
            navigator.clipboard.writeText(getStandardId(id));
        } else {
            navigator.clipboard.writeText(getShortId(id));
        }
    }
</script>

<Pane>
    {#snippet header()}
        <h2 class="header">
            <span class="title">
                <ActionWidget
                    tip={`click to copy ${shortChangeId}\
                    \nCtrl/⌘+click to copy ${standardChangeId}`}
                    onClick={(event) => {
                        copyId(rev.header.id.change, event);
                    }}>
                    <Icon name="copy" /><IdSpan id={rev.header.id.change} />
                </ActionWidget>
                |
                <ActionWidget
                    tip={`click to copy ${shortCommitId}\
                    \nCtrl/⌘+click to copy ${standardCommitId}`}
                    onClick={(event) => {
                        copyId(rev.header.id.commit, event);
                    }}>
                    <Icon name="copy" /><IdSpan id={rev.header.id.commit} />
                </ActionWidget>
                {#if rev.header.is_working_copy}
                    | Working copy
                {/if}
                {#if rev.header.is_immutable}
                    | Immutable
                {/if}
            </span>

            <div class="checkout-commands">
                <ActionWidget
                    tip="make working copy"
                    onClick={mutator.onEdit}
                    disabled={rev.header.is_immutable || rev.header.is_working_copy}>
                    <Icon name="edit-2" /> Edit
                </ActionWidget>
                <ActionWidget tip="create a child" onClick={mutator.onNew}>
                    <Icon name="edit" /> New
                </ActionWidget>
            </div>
        </h2>
    {/snippet}

    {#snippet body()}
        <div class="body">
            <textarea
                class="description"
                placeholder="(no description set)"
                spellcheck="false"
                disabled={rev.header.is_immutable}
                bind:value={fullDescription}
                ondragenter={dragOverWidget}
                ondragover={dragOverWidget}
                onkeydown={(ev) => {
                    if (descriptionChanged && ev.key === "Enter" && (ev.metaKey || ev.ctrlKey)) {
                        updateDescription();
                    }
                }}></textarea>

            <div class="signature-commands">
                <span>Author:</span>
                <AuthorSpan author={rev.header.author} includeTimestamp />
                <CheckWidget bind:checked={resetAuthor}>Reset</CheckWidget>
                <span></span>
                <ActionWidget
                    tip="set commit message"
                    onClick={() => mutator.onDescribe(fullDescription, resetAuthor)}
                    disabled={rev.header.is_immutable || !descriptionChanged}>
                    <Icon name="file-text" /> Describe
                </ActionWidget>
            </div>

            {#if rev.parents.length > 0}
                <Zone operand={{ type: "Merge", header: rev.header }}>
                    {#snippet children({ target })}
                        <div class="parents" class:target>
                            {#each rev.parents as parent}
                                <div class="parent">
                                    <span>Parent:</span>
                                    <RevisionObject
                                        header={parent}
                                        child={rev.header}
                                        selected={false}
                                        noBranches />
                                </div>
                            {/each}
                        </div>
                    {/snippet}
                </Zone>
            {/if}

            {#if syntheticChanges.length > 0}
                <div class="move-commands">
                    <span>Changes:</span>
                    <ActionWidget
                        tip="move all changes to parent"
                        onClick={mutator.onSquash}
                        disabled={rev.header.is_immutable || rev.header.parent_ids.length != 1}>
                        <Icon name="upload" /> Squash
                    </ActionWidget>
                    <ActionWidget
                        tip="copy all changes from parent"
                        onClick={mutator.onRestore}
                        disabled={rev.header.is_immutable || rev.header.parent_ids.length != 1}>
                        <Icon name="download" /> Restore
                    </ActionWidget>
                </div>

                <ListWidget {list} type="Change" descendant={$changeSelectEvent?.path.repo_path}>
                    <div class="changes">
                        {#each syntheticChanges as change}
                            <ChangeObject
                                {change}
                                header={rev.header}
                                selected={$changeSelectEvent?.path?.repo_path ===
                                    change.path.repo_path} />
                            {#if $changeSelectEvent?.path?.repo_path === change.path.repo_path}
                                <div class="change" style="--lines: {minLines(change)}">
                                    {#each change.hunks as hunk}
                                        <div class="hunk">
                                            @@ -{hunk.location.from_file.start},
                                            {hunk.location.from_file.len}
                                            +{hunk.location.to_file.start},
                                            {hunk.location.to_file.len} @@
                                        </div>
                                        <pre class="diff">{#each hunk.lines.lines as line}<span
                                                    class={lineColour(line)}>{line}</span
                                                >{/each}</pre>
                                    {/each}
                                </div>
                            {/if}
                        {/each}
                    </div>
                </ListWidget>
            {:else}
                <div class="move-commands">
                    <span>Changes: <span class="no-changes">(empty)</span></span>
                </div>
            {/if}
        </div>
    {/snippet}
</Pane>

<style>
    @reference "tailwindcss";

    .header {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        align-items: center;
        text-wrap: nowrap;
        font-weight: normal;
    }

    .title {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .checkout-commands {
        height: 30px;
        padding: 0 3px;
        display: flex;
        align-items: center;
        justify-content: end;
        gap: 6px;
    }

    .body {
        height: 100%;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        margin: 0 -6px -3px -6px;
        padding: 0 6px 3px 6px;
        gap: 0;
    }

    .description {
        resize: vertical;
        min-height: 90px;
        overflow: auto;
        background: light-dark(
            --alpha(var(--color-stone-400) / 30%),
            --alpha(var(--color-zinc-600) / 60%)
        );
    }

    .signature-commands {
        width: 100%;
        display: grid;
        grid-template-columns: min-content auto auto 1fr auto;
        align-items: center;
        gap: 6px;
        padding: 4px 2px;
        flex-shrink: 0;
    }

    .parents {
        border-top: 1px solid var(--ctp-overlay0);
        padding: 4px 2px;
    }

    .parent {
        display: grid;
        grid-template-columns: min-content 1fr;
        align-items: baseline;
        gap: 6px;
    }

    .move-commands {
        border-top: 1px solid var(--ctp-overlay0);
        padding: 4px 2px;
        display: grid;
        grid-template-columns: 1fr auto auto;
        align-items: center;
        gap: 6px;
    }

    .move-commands > :global(button) {
        margin-top: -1px;
    }

    .no-changes {
        opacity: 0.5;
    }

    .changes {
        border-top: 1px solid var(--ctp-overlay0);
        display: flex;
        flex-direction: column;
        pointer-events: auto;
        overflow-x: hidden;
        overflow-y: auto;
        scrollbar-width: thin;
        flex: 1;
        min-height: 0;
    }

    .change {
        margin: 0;
        pointer-events: auto;
        overflow-x: auto;
        overflow-y: scroll;
        scrollbar-width: thin;
        min-height: calc(var(--lines) * 1em);
    }

    .hunk {
        margin: 0;
        text-align: center;
    }

    .diff {
        margin: 0;
        user-select: text;
        background: light-dark(
            --alpha(var(--color-stone-400) / 20%),
            --alpha(var(--color-zinc-600) / 30%)
        );
    }

    .add {
        color: light-dark(var(--color-emerald-700), var(--color-green-400));
    }

    .remove {
        color: light-dark(var(--color-red-600), var(--color-red-400));
    }

    .target {
        color: black;
        background: var(--ctp-flamingo);
    }
</style>
