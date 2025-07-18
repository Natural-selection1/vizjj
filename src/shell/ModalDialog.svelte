<script lang="ts">
    import { onMount } from "svelte";

    interface Props {
        title: string;
        error?: boolean;
        cancel?: () => void;
        default?: () => void;
        children?: import("svelte").Snippet;
        commands?: import("svelte").Snippet;
    }
    let {
        title,
        error = false,
        cancel,
        default: defaultAction,
        children,
        commands,
    }: Props = $props();

    onMount(() => {
        document.addEventListener("keydown", onKeyDown);
        return () => {
            document.removeEventListener("keydown", onKeyDown);
        };
    });

    function onKeyDown(event: KeyboardEvent) {
        if (event.key == "Escape") {
            cancel?.();
        } else if (event.key == "Enter") {
            defaultAction?.();
        }
    }
</script>

<div id="dialog-chrome" role="dialog" aria-modal="true">
    <h3 id="dialog-header" class:error>{title}</h3>
    <div id="dialog-content">{@render children?.()}</div>
    <div id="dialog-commands">{@render commands?.()}</div>
</div>

<style>
    @reference "tailwindcss";

    #dialog-chrome {
        grid-area: 2/2/2/2;

        background: light-dark(var(--color-neutral-300), var(--color-neutral-800));
        border-radius: 9px;
        border: 3px solid var(--ctp-overlay1);

        display: grid;
        grid-template-columns: 30px 1fr 33px;
        grid-template-rows: 30px auto 30px;
    }

    #dialog-header {
        margin-top: 6px;
        padding: 0 15px;
        grid-area: 1/2/2/2;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    #dialog-content {
        grid-area: 2/2/2/2;
        display: grid;
        grid-template-columns: auto 1fr;
        align-items: baseline;
        gap: 3px 6px;
    }

    #dialog-content :global(select),
    #dialog-content :global(input) {
        min-height: 30px;
        min-width: 180px;
        font-size: 14px;
    }

    #dialog-commands {
        margin-right: 3px;
        grid-area: 3/1/3/4;
        display: flex;
        align-items: center;
        justify-content: end;
        gap: 6px;
    }

    .error {
        color: var(--color-red-600);
    }
</style>
