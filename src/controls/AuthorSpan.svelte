<script lang="ts">
    import type { RevAuthor } from "../messages/RevAuthor";
    interface Props {
        author: RevAuthor;
        includeTimestamp?: boolean;
    }
    let { author, includeTimestamp = false }: Props = $props();
    let datetime = new Date(author.timestamp);

    function relativeDate() {
        const cutoffs = [60, 3600, 86400, 86400 * 7, 86400 * 30, 86400 * 365, Infinity];
        const units: Intl.RelativeTimeFormatUnit[] = [
            "second",
            "minute",
            "hour",
            "day",
            "week",
            "month",
            "year",
        ];
        const formatter = new Intl.RelativeTimeFormat(undefined, {
            style: "long",
            numeric: "auto",
        });
        let ticks = datetime.getTime();
        let seconds = Math.round((ticks - Date.now()) / 1000);
        let unitIndex = cutoffs.findIndex((cutoff) => cutoff > Math.abs(seconds));
        let divisor = unitIndex ? cutoffs[unitIndex - 1] : 1;

        return formatter.format(Math.floor(seconds / divisor), units[unitIndex]);
    }
</script>

<span class="author">
    {#if includeTimestamp}
        <div class="inline" title={author.email}>{author.name},</div>
        <div class="inline" title={datetime.toLocaleString()}>{relativeDate()}</div>
    {:else}
        <div class="inline" title={author.email + ", " + datetime.toLocaleString()}>
            {author.name}
        </div>
    {/if}
</span>

<style>
    .author {
        opacity: 0.55;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .inline {
        display: inline-block;
        pointer-events: auto;
    }
</style>
