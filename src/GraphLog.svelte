<!-- Renders commit rows with an SVG graph drawn over them, virtualising the ui to allow for long graphs -->

<script module lang="ts">
    import type { LogLine } from "./messages/LogLine.js";

    export type EnhancedLine = LogLine & { key: number; parent: RevHeader; child: RevHeader };

    export interface EnhancedRow extends LogRow {
        passingLines: Array<EnhancedLine>;
    }
</script>

<script lang="ts">
    import GraphLine from "./GraphLine.svelte";
    import GraphNode from "./GraphNode.svelte";
    import type { LogRow } from "./messages/LogRow.js";
    import type { RevHeader } from "./messages/RevHeader.js";

    interface $$Slots {
        default: { row: EnhancedRow | null };
    }

    const columnWidth = 18;
    const rowHeight = 30;
    interface Props {
        containerHeight: number;
        containerWidth: number;
        scrollTop: number;
        rows: (EnhancedRow | null)[];
        children?: import("svelte").Snippet<[any]>;
    }
    let { containerHeight, containerWidth, scrollTop, rows, children }: Props = $props();

    let graphHeight = $derived(Math.max(containerHeight, rows.length * rowHeight));
    let visibleRows = $derived(Math.ceil(containerHeight / rowHeight) + 1);
    let startIndex = $derived(Math.floor(Math.max(scrollTop, 0) / rowHeight));
    let endIndex = $derived(startIndex + visibleRows);
    let overlap = $derived(startIndex % visibleRows);
    let visibleSlice = $derived({
        rows: shiftArray(sliceArray(rows, startIndex, endIndex), overlap),
        keys: new Set<number>(),
    });

    function sliceArray(arr: (EnhancedRow | null)[], start: number, end: number) {
        arr = arr.slice(start, end);
        let expectedLength = end - start;
        while (arr.length < expectedLength) {
            arr.push(null); // placeholders when there aren't enough items to fill the container
        }
        return arr;
    }

    function shiftArray(arr: (EnhancedRow | null)[], count: number) {
        for (let i = 0; i < count; i++) {
            arr.unshift(arr.pop()!);
        }
        return arr;
    }

    function distinctLines(keys: Set<number>, row: EnhancedRow | null): EnhancedLine[] {
        if (row === null) {
            return [];
        }
        return row.passingLines
            .filter((l) => {
                if (keys.has(l.key)) {
                    return false;
                } else {
                    keys.add(l.key);
                    return true;
                }
            })
            .sort((a, b) => {
                let aSameColumn = a.source[0] == a.target[0];
                let bSameColumn = b.source[0] == b.target[0];
                if (aSameColumn && !bSameColumn) {
                    return -1;
                } else if (bSameColumn && !aSameColumn) {
                    return 1;
                } else {
                    return 0;
                }
            });
    }
</script>

<svg class="graph" style="width: 100%; height: {graphHeight}px;">
    {#each visibleSlice.rows as row}
        {#key row}
            <g
                transform="translate({(row?.location[0] ?? 0) * columnWidth} {(row?.location[1] ??
                    0) * rowHeight})">
                <foreignObject
                    class:placeholder={row === null}
                    height={rowHeight}
                    width={containerWidth - (row?.location[0] ?? 0) * columnWidth}
                    style="--leftpad: {(row?.padding ?? 0) * columnWidth + columnWidth + 6}px;">
                    <slot {row} />
                </foreignObject>

                {#if row}
                    <GraphNode header={row.revision} />
                {/if}
            </g>
        {/key}
    {/each}

    {#each visibleSlice.rows as row}
        {#key row}
            {#each distinctLines(visibleSlice.keys, row) as line}
                <GraphLine {line} />
            {/each}
        {/key}
    {/each}
</svg>

<style>
    svg {
        stroke: var(--ctp-text);
        fill: var(--ctp-text);
        overflow: hidden;
    }

    foreignObject {
        overflow: hidden;
        display: flex;
        align-items: stretch;
    }

    .placeholder {
        pointer-events: none;
        visibility: hidden;
    }
</style>
