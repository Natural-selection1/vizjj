<script lang="ts">
    import sprite from "feather-icons/dist/feather-sprite.svg";
    interface Props {
        name: string;
        state?: "add" | "change" | "remove" | null;
    }
    let { name, state = null }: Props = $props();
</script>

<svg
    class="feather"
    class:add={state == "add"}
    class:change={state == "change"}
    class:remove={state == "remove"}
    viewBox="0 0 24 24">
    <use href="{sprite}#{name}" />
</svg>

<style>
    .feather {
        pointer-events: none;
        height: var(--app-font-size);
        fill: none;
        stroke: currentColor;
        stroke-linecap: round;
        stroke-linejoin: round;
        stroke-width: 2;
    }

    .add {
        stroke: var(--ctp-green);
    }

    .change {
        color: var(--ctp-blue);
    }

    .remove {
        color: var(--ctp-red);
    }
</style>
