<script lang="ts">
    import { dragOverWidget } from "../stores";

    interface Props {
        id?: string | null;
        checked: boolean;
        children?: import("svelte").Snippet;
    }
    let { id = null, checked = $bindable(), children }: Props = $props();
</script>

<label>
    <input
        {id}
        type="checkbox"
        bind:checked
        ondragenter={dragOverWidget}
        ondragover={dragOverWidget} />
    <slot />
</label>

<style>
    label {
        display: flex;
        align-items: center;
        font-family: var(--stack-industrial);
        color: var(--ctp-text);
        gap: 3px;
    }

    input:focus-visible {
        outline: 1px solid var(--ctp-lavender);
        outline-offset: 0;
    }
</style>
