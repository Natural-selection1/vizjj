<script lang="ts">
    import { hasModal } from "../stores";
    interface Props {
        header?: import("svelte").Snippet;
        body?: import("svelte").Snippet;
    }
    let { header, body }: Props = $props();
</script>

<section inert={$hasModal}>
    <div class="header">{@render header?.()}</div>
    <div class="body">{@render body?.()}</div>
</section>

<style>
    section {
        display: grid;
        grid-template-rows: auto 1fr;
        grid-template-columns: 100%;
        overflow: hidden;
    }

    .header {
        border-bottom: 1px solid var(--ctp-overlay0);
        padding: 6px 6px 3px 6px;
    }

    .body {
        padding: 3px 6px 6px 6px;
        display: grid;
        overflow: hidden;
    }
</style>
