<!--
@component
Core component for direct-manipulation objects. A drag&drop source.
-->

<script lang="ts">
    import type { Operand } from "../messages/Operand";
    import { trigger } from "../ipc";
    import { currentContext, currentSource } from "../stores";
    import BinaryMutator from "../mutators/BinaryMutator";

    interface $$Slots {
        default: { context: boolean; hint: string | null };
    }

    interface Props {
        suffix?: string | null;
        label: string;
        selected?: boolean;
        conflicted: boolean;
        operand: Operand;
        click?: () => void;
        dblclick?: () => void;
        children?: import("svelte").Snippet<[any]>;
    }
    let {
        suffix = null,
        label,
        selected = false,
        conflicted,
        operand,
        click,
        dblclick,
        children,
    }: Props = $props();

    let id = suffix == null ? null : `${operand.type}-${suffix}`;
    let dragging: boolean = $state(false);
    let dragHint: string | null = $state(null);

    function onMenu(event: Event) {
        if (operand.type == "Ref" || operand.type == "Change" || operand.type == "Revision") {
            event.preventDefault();
            event.stopPropagation();
            currentContext.set(operand);
            trigger("forward_context_menu", { context: operand });
        }
    }

    function onDragStart(event: DragEvent) {
        currentContext.set(null);
        event.stopPropagation();
        let canDrag = BinaryMutator.canDrag(operand);
        if (canDrag.type == "no") {
            return;
        } else {
            // if we need more than one drag to be active, this could store a key
            event.dataTransfer?.setData("text/plain", "");
            // it would've been nice to just put this in the drag data
            // but chrome says That's Insecure
            $currentSource = operand;
            dragging = true;
            if (canDrag.type == "maybe") {
                dragHint = canDrag.hint;
                let empty = document.createElement("div");
                event.dataTransfer?.setDragImage(empty, 0, 0);
            }
        }
    }

    function onDragEnd() {
        $currentSource = null;
        dragging = false;
        dragHint = null;
    }
</script>

<button
    {id}
    class:selected
    class:conflict={conflicted}
    class:context={dragging || $currentContext == operand}
    class:hint={dragHint}
    tabindex="-1"
    draggable="true"
    role="option"
    aria-label={label}
    aria-selected={selected}
    onclick={click}
    ondblclick={dblclick}
    oncontextmenu={onMenu}
    ondragstart={onDragStart}
    ondragend={onDragEnd}>
    <slot context={dragging || $currentContext == operand} hint={dragHint} />
</button>

<style>
    button {
        /* reset button styles */
        background: transparent;
        border: none;
        margin: 0;
        padding: 0;
        color: inherit;
        text-align: left;

        cursor: pointer;
        width: 100%;
        display: flex;
        align-items: center;
    }

    .selected {
        background: var(--ctp-base);
    }

    .conflict {
        background: repeating-linear-gradient(
            120deg,
            transparent 0px,
            transparent 12px,
            var(--ctp-surface0) 12px,
            var(--ctp-surface0) 15px
        );
    }

    .selected.conflict {
        background: repeating-linear-gradient(
            120deg,
            var(--ctp-surface0) 0px,
            var(--ctp-surface0) 12px,
            var(--ctp-base) 12px,
            var(--ctp-base) 15px
        );
    }

    .context {
        color: var(--ctp-rosewater);
    }

    .hint {
        color: var(--ctp-peach);
    }
</style>
