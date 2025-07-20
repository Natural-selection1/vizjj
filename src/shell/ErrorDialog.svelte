<script lang="ts">
    import ActionWidget from "../controls/ActionWidget.svelte";
    import ModalDialog from "./ModalDialog.svelte";

    interface Props {
        title: string;
        severe?: boolean;
        onClose?: () => void;
        children?: import("svelte").Snippet;
    }
    let { title, severe = false, onClose, children }: Props = $props();
</script>

<ModalDialog {title} error={severe} cancel={onClose}>
    <div class="error-content">
        {@render children?.()}
    </div>

    {#snippet commands()}
        {#if onClose}
            <ActionWidget tip="close dialog" safe onClick={onClose}>OK</ActionWidget>
        {/if}
    {/snippet}
</ModalDialog>

<style>
    .error-content {
        word-wrap: break-word;
        overflow-wrap: break-word;
        white-space: normal;
        line-height: 1.5;
        margin-bottom: 12px;
    }
</style>
