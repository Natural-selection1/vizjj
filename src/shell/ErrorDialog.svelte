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
    {@render children?.()}

    {#snippet commands()}
        {#if onClose}
            <ActionWidget tip="close dialog" safe onClick={onClose}>OK</ActionWidget>
        {/if}
    {/snippet}
</ModalDialog>
