<script lang="ts">
    import ActionWidget from "../controls/ActionWidget.svelte";
    import Icon from "../controls/Icon.svelte";
    import ModalDialog from "./ModalDialog.svelte";

    export let title: string;
    export let severe: boolean = false;
    export let onClose: (() => void) | null = null;

    function handleCancel() {
        if (onClose) {
            onClose();
        }
    }
</script>

<ModalDialog {title} error={severe} on:cancel={handleCancel}>
    <slot />

    <svelte:fragment slot="commands">
        {#if onClose}
            <ActionWidget tip="close dialog" safe onClick={onClose}>OK</ActionWidget>
        {/if}
    </svelte:fragment>
</ModalDialog>
