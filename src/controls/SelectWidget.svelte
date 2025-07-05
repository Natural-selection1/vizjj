<script lang="ts" generics="T extends {value: string}">
    import { createEventDispatcher } from "svelte";
    import Icon from "./Icon.svelte";

    interface $$Slots {
        default: { option: T };
    }

    interface $$Events {
        change: CustomEvent<Event>;
    }

    interface Props {
        id?: string | null;
        options: T[];
        value: string;
        children?: import("svelte").Snippet<[any]>;
    }
    let { id = null, options, value = $bindable(), children }: Props = $props();

    let dispatch = createEventDispatcher();
</script>

<div class="wrapper">
    <select class="select" {id} bind:value onchange={(event) => dispatch("change", event)}>
        {#each options as option}
            <option selected={value == option.value} value={option.value}>
                <slot {option}>{option.value}</slot>
            </option>
        {/each}
    </select>
    <Icon name="chevron-down" />
</div>

<style>
    select {
        appearance: none;
        padding-left: 3px;

        &:focus-visible {
            padding-left: 2px;
        }
    }

    .wrapper {
        display: flex;
        position: relative;
    }

    .wrapper > :global(:last-child) {
        position: absolute;
        right: 0;
        height: 32px;
        right: 3px;
    }
</style>
