<script lang="ts">
    import { trigger } from "../ipc.js";

    let { workspaces = [] }: { workspaces?: string[] } = $props();
</script>

{#if workspaces.length > 0}
    <div class="recent-workspaces">
        <p><strong>recent workspaces:</strong></p>
        <ul class="workspaces-list">
            {#each workspaces as workspace}
                <li class="workspace-item">
                    <button
                        class="workspace-button"
                        onclick={() =>
                            trigger("open_workspace_at_path", {
                                path: workspace,
                            })}
                        title="open this workspace">
                        {workspace}
                    </button>
                </li>
            {/each}
        </ul>
    </div>
{/if}

<style>
    @reference "tailwindcss";

    .recent-workspaces {
        margin-top: 16px;
    }

    .workspaces-list {
        text-align: left;
        margin: 8px 0;
        padding-left: 20px;
    }

    .workspace-item {
        margin: 10px 0;
    }

    .workspace-button {
        color: light-dark(var(--color-cyan-700), var(--color-cyan-500));
        font-size: 18px;

        &:hover {
            color: var(--color-emerald-600);
            text-decoration: underline;
        }
    }
</style>
