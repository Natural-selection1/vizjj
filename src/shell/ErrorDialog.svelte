<script lang="ts">
    import ActionWidget from "../controls/ActionWidget.svelte";
    import Icon from "../controls/Icon.svelte";
    import ModalDialog from "./ModalDialog.svelte";

    interface Props {
        title: string;
        severe?: boolean;
        onClose?: (() => void) | null;
        children?: import("svelte").Snippet;
    }
    let { title, severe = false, onClose = null, children }: Props = $props();

    function handleCancel() {
        if (onClose) {
            onClose();
        }
    }
</script>

<ModalDialog {title} error={severe} on:cancel={handleCancel}>
    {@render children?.()}

    <svelte:fragment slot="commands">
        {#if onClose}
            <ActionWidget tip="close dialog" safe onClick={onClose}>OK</ActionWidget>
        {/if}
    </svelte:fragment>
</ModalDialog>
