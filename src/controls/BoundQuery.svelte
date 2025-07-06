<!--
@component
Abstraction of dubious utility - it's only used in one place, because most IPC does not follow a query-like pattern.
-->

<script lang="ts" generics="T">
    import type { Query } from "../ipc";

    interface Props {
        query: Query<T>;
        wait?: import("svelte").Snippet;
        error?: import("svelte").Snippet<[any]>;
        children?: import("svelte").Snippet<[any]>;
    }
    let { query, wait, error, children }: Props = $props();
</script>

{#key query}
    {#if query.type == "wait"}
        {@render wait?.()}
    {:else if query.type == "error"}
        {#if error}
            {@render error({ message: query.message })}
        {:else}
            <span class="red">{query.message}</span>
        {/if}
    {:else}
        {@render children?.({ data: query.value })}
    {/if}
{/key}

<style>
    .red {
        color: #d20f39;
    }
</style>
