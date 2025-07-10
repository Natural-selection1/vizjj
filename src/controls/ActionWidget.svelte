<script lang="ts">
    import { dragOverWidget, hasModal } from "../stores";

    interface Props {
        tip?: string;
        onClick: (event: MouseEvent) => void;
        safe?: boolean;
        disabled?: boolean;
        children?: import("svelte").Snippet;
    }
    let { tip = "", onClick, safe = false, disabled = false, children }: Props = $props();
</script>

{#if disabled || (!safe && $hasModal)}
    <button disabled class:safe ondragenter={dragOverWidget} ondragover={dragOverWidget}>
        {@render children?.()}
    </button>
{:else}
    <button
        class:safe
        onclick={onClick}
        ondragenter={dragOverWidget}
        ondragover={dragOverWidget}
        title={safe ? "" : tip}>
        {@render children?.()}
    </button>
{/if}

<style>
    @reference "tailwindcss";

    button {
        display: flex;
        height: 24px;
        font-size: 16px;
        outline: none;
        background: light-dark(--alpha(var(--color-slate-400) / 50%), var(--color-stone-700));
        border-width: 1px;
        border-radius: 3px;
        border-color: light-dark(var(--color-stone-700), --alpha(var(--color-slate-400) / 50%));
        box-shadow: 2px 2px
            light-dark(var(--color-stone-700), --alpha(var(--color-slate-400) / 50%));
        align-items: center;
        gap: 3px;
        padding: 1px 6px;
    }

    button:not(:disabled) {
        &:hover {
            background: light-dark(var(--color-stone-400), var(--color-stone-500));
        }
        &:focus-visible {
            border-width: 2px;
            padding: 0px 5px;
            text-decoration: underline;
        }
        &:active {
            margin: 1px 0px 0px 1px;
            padding: 1px 5px 0px 6px;
            box-shadow: 1px 1px;
            &:focus-visible {
                padding: 1px 4px 0px 5px;
            }
        }
    }

    button.safe {
        background: var(--ctp-sapphire);
        &:hover {
            background: var(--ctp-teal);
        }
        &:active {
            border-right-color: var(--ctp-teal);
            border-bottom-color: var(--ctp-teal);
        }
    }

    button:disabled {
        background: --alpha(var(--color-stone-300) / 20%);
        color: light-dark(
            --alpha(var(--color-stone-800) / 50%),
            --alpha(var(--color-stone-200) / 60%)
        );
    }
</style>
