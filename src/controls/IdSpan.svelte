<script lang="ts">
    import type { ChangeId } from "../messages/ChangeId";
    import type { CommitId } from "../messages/CommitId";
    import { currentTarget } from "../stores";
    interface Props {
        id: ChangeId | CommitId;
        pronoun?: boolean;
    }
    let { id, pronoun = false }: Props = $props();
    let suffix = id.rest.substring(0, 8 - id.prefix.length);
</script>

<span class="id" class:pronoun={pronoun || $currentTarget?.type == "Repository"}>
    <span class="prefix {id.type}">{id.prefix}</span>{suffix}
</span>

<style>
    @reference "tailwindcss";

    .id {
        font-size: calc(var(--app-font-size) - 4px);
        pointer-events: none;
        color: light-dark(
            --alpha(var(--color-stone-700) / 60%),
            --alpha(var(--color-stone-300) / 60%)
        );
    }

    .ChangeId {
        font-size: var(--app-font-size);
        color: light-dark(var(--color-rose-500), var(--color-rose-400));
    }

    .CommitId {
        font-size: var(--app-font-size);
        color: light-dark(var(--color-teal-500), var(--color-teal-400));
    }

    .pronoun {
        color: inherit;
        pointer-events: none;
    }

    .pronoun > .prefix {
        color: inherit;
        font-weight: bold;
    }
</style>
