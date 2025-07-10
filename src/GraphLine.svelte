<script lang="ts">
    import type { EnhancedLine } from "./GraphLog.svelte";
    import type { Operand } from "./messages/Operand";
    import Zone from "./objects/Zone.svelte";
    import { currentTarget } from "./stores";

    let { line }: { line: EnhancedLine } = $props();

    let isMerge = line.type == "ToIntersection";
    let allowEarlyBreak = line.type == "FromNode";
    let c1 = line.source[0];
    let c2 = line.target[0];
    let r1 = line.source[1];
    let r2 = line.target[1];
    let childY = r1 * 30 + 21;
    let parentY = r2 * 30 + 9;
    let operand: Operand = { type: "Parent", header: line.parent, child: line.child };

    // draw path downward, from child to parent
    let graphLineInfo = $derived.by(() => {
        if (isMerge) {
            // instead of a parent, we have a mergepoint
            let childX = c1 * 18 + 9;
            let mergeX = c2 * 18 + 9;
            let midY = c2 > c1 ? childY + 9 : parentY - 9;
            let radius = c2 > c1 ? 6 : -6;
            let sweep = c2 > c1 ? 0 : 1;
            return {
                path: `M${childX},${childY}
                L${childX},${midY - 6}
                A6,6,0,0,${sweep},${childX + radius},${midY}
                L${mergeX - radius},${midY}
                A6,6,0,0,${1 - sweep},${mergeX},${midY + 6}
                L${mergeX},${parentY}`,
                blockX: c1 < c2 ? c1 * 18 + 2 : c2 * 18 + 2,
                blockY: r1 * 30 + 22,
                blockW: c1 < c2 ? (c2 - c1 + 1) * 18 - 5 : (c1 - c2 + 1) * 18 - 5,
                blockH: 14,
            };
        } else if (c1 == c2) {
            // same-column, straight line
            let x = c1 * 18 + 9;
            return {
                path: `M${x},${childY} L${x},${parentY}`,
                blockX: c1 * 18 + 2,
                blockY: r1 * 30 + 21,
                blockW: 14,
                blockH: (r2 - r1) * 30 - 12,
            };
        } else {
            // different-column, curved line
            let childX = c1 * 18 + 9;
            let parentX = c2 * 18 + 9;
            let midY = allowEarlyBreak && c1 > c2 ? parentY - 9 : childY + 9;
            let radius = c2 > c1 ? 6 : -6;
            let sweep = c2 > c1 ? 0 : 1;
            return {
                path: `M${childX},${childY}
                L${childX},${midY - 6}
                A6,6,0,0,${sweep},${childX + radius},${midY}
                L${parentX - radius},${midY}
                A6,6,0,0,${1 - sweep},${parentX},${midY + 6}
                L${parentX},${parentY}`,
                blockX: c1 < c2 ? c1 * 18 + 16 : c2 * 18 + 16,
                blockY: r1 * 30 + 22,
                blockW: c1 < c2 ? (c2 - c1) * 18 - 14 : (c1 - c2) * 18 - 14,
                blockH: 14,
            };
        }
    });

    let path: string = $derived(graphLineInfo.path);
    let blockX: number = $derived(graphLineInfo.blockX);
    let blockY: number = $derived(graphLineInfo.blockY);
    let blockW: number = $derived(graphLineInfo.blockW);
    let blockH: number = $derived(graphLineInfo.blockH);
</script>

{#if !line.indirect}
    <foreignObject x={blockX} y={blockY} width={blockW} height={blockH}>
        <Zone {operand}>
            {#snippet children({ target })}
                <div class="backdrop" class:target></div>
            {/snippet}
        </Zone>
    </foreignObject>
{/if}

<path
    d={path}
    fill="none"
    stroke-dasharray={line.indirect ? "1,2" : "none"}
    class:target={$currentTarget == operand} />

<style>
    @reference "tailwindcss";

    path {
        pointer-events: none;
        @apply stroke-sky-500;
    }

    foreignObject > :global(*) {
        height: 100%;
    }

    .backdrop {
        width: 100%;
        height: 100%;
    }

    .target {
        @apply stroke-black bg-rose-400;
    }
</style>
