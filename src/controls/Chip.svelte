<script lang="ts">
    interface Props {
        context: boolean;
        target: boolean;
        disconnected?: boolean;
        immobile?: boolean;
        tip?: string;
        children?: import("svelte").Snippet;
    }
    let {
        context,
        target,
        disconnected = false,
        immobile = false,
        tip = "",
        children,
    }: Props = $props();
</script>

<div class={["chip", context, target, disconnected, immobile]} title={tip}>
    {@render children?.()}
</div>

<style>
    @reference "tailwindcss";

    .chip {
        pointer-events: auto;

        font-family: var(--stack-code);
        font-size: smaller;

        height: 24px;
        line-height: 16px;

        display: flex;
        align-items: center;
        border: 1px solid var(--ctp-overlay1);
        border-radius: 12px;
        padding: 0 6px;
        background: light-dark(var(--color-stone-100), var(--color-stone-200));
        white-space: nowrap;
        gap: 3px;
    }

    .context {
        color: inherit;
        border-color: currentColor;
    }

    .target {
        background: var(--ctp-flamingo);
        color: black;
    }

    .disconnected {
        border-style: dashed;
    }

    .immobile {
        border-style: none;
    }
</style>
