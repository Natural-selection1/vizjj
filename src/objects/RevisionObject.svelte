<script lang="ts">
    import type { RevHeader } from "../messages/RevHeader";
    import type { Operand } from "../messages/Operand";
    import { currentTarget, revisionSelectEvent } from "../stores.js";
    import IdSpan from "../controls/IdSpan.svelte";
    import BranchObject from "./BranchObject.svelte";
    import Object from "./Object.svelte";
    import Zone from "./Zone.svelte";
    import RevisionMutator from "../mutators/RevisionMutator";
    import TagObject from "./TagObject.svelte";
    import AuthorSpan from "../controls/AuthorSpan.svelte";
    import EmojiText from "../controls/EmojiText.svelte";

    interface Props {
        header: RevHeader;
        child?: RevHeader | null;
        // same as the imported event, but parent may want to force a value
        selected: boolean;
        noBranches?: boolean;
    }
    let { header, child = null, selected, noBranches = false }: Props = $props();

    let operand: Operand = child ? { type: "Parent", header, child } : { type: "Revision", header };

    function onSelect() {
        revisionSelectEvent.set(header);
    }
    function onEdit() {
        new RevisionMutator(header).onEdit();
    }
</script>

<Object
    {operand}
    suffix={header.id.commit.prefix}
    conflicted={header.has_conflict}
    {selected}
    label={header.description.lines[0]}
    click={onSelect}
    dblclick={onEdit}>
    {#snippet children({ context, hint: dragHint })}
        {#if child}
            <!-- Parents aren't a drop target -->
            <div class="layout">
                <IdSpan
                    id={header.id.change}
                    pronoun={context ||
                        ($currentTarget?.type == "Merge" &&
                            $currentTarget.header.parent_ids.findIndex(
                                (id) => id.hex == header.id.commit.hex
                            ) != -1)} />
                <span
                    class="text desc truncate"
                    class:indescribable={!context && header.description.lines[0] == ""}>
                    {#if header.description.lines[0] == ""}
                        {dragHint ?? "(no description set)"}
                    {:else}
                        <EmojiText text={dragHint ?? header.description.lines[0]} />
                    {/if}
                </span>
                <span class="email"><AuthorSpan author={header.author} /></span>
                <span class="refs">
                    {#each header.refs as ref}
                        {#if ref.type != "Tag"}
                            {#if !noBranches && (ref.type == "LocalBookmark" || !ref.is_synced || !ref.is_tracked)}
                                <div>
                                    <BranchObject {header} {ref} />
                                </div>
                            {/if}
                        {:else}
                            <div>
                                <TagObject {header} {ref} />
                            </div>
                        {/if}
                    {/each}
                </span>
            </div>
        {:else}
            <Zone {operand}>
                {#snippet children({ target, hint: dropHint })}
                    <div class="layout" class:target>
                        <IdSpan
                            id={header.id.change}
                            pronoun={context || target || dropHint != null} />
                        <span
                            class="text desc truncate"
                            class:indescribable={!context && header.description.lines[0] == ""}>
                            {#if header.description.lines[0] == ""}
                                {dragHint ?? dropHint ?? "(no description set)"}
                            {:else}
                                <EmojiText
                                    text={dragHint ?? dropHint ?? header.description.lines[0]} />
                            {/if}
                        </span>
                        <span class="email"><AuthorSpan author={header.author} /></span>
                        <span class="refs">
                            {#each header.refs as ref}
                                {#if ref.type != "Tag"}
                                    {#if ref.type == "LocalBookmark" || !ref.is_synced || !ref.is_tracked}
                                        <div>
                                            <BranchObject {header} {ref} />
                                        </div>
                                    {/if}
                                {:else}
                                    <div>
                                        <TagObject {header} {ref} />
                                    </div>
                                {/if}
                            {/each}
                        </span>
                    </div>
                {/snippet}
            </Zone>
        {/if}
    {/snippet}
</Object>

<style>
    .layout {
        pointer-events: auto;
        /* layout summary components along a text line */
        width: 100%;
        display: grid;
        grid-template-areas: ". desc refs";
        grid-template-columns: auto 1fr auto;
        align-items: baseline;
        gap: 6px;

        /* skip past svg lines when used in a graph */
        padding-left: var(--leftpad);
    }

    .layout.target {
        background: var(--ctp-flamingo);
        color: black;
    }

    .desc {
        grid-area: desc;
    }

    .desc.indescribable {
        opacity: 0.5;
    }

    .email {
        display: none;
        grid-area: email;
        text-align: right;
    }

    .refs {
        grid-area: refs;
        align-self: center;
        display: flex;
        justify-content: end;
        gap: 3px;
        color: var(--ctp-text);
    }

    /* multiple elements can have these */
    .truncate {
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .text {
        pointer-events: none;
    }

    @media (width >= 1680px) {
        .layout {
            grid-template-areas: ". desc refs email";
            grid-template-columns: auto auto 1fr auto;
            gap: 9px;
        }

        .email {
            display: initial;
        }
    }
</style>
