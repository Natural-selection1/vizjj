<script lang="ts">
    import { getContext } from "svelte";
    import type { RevHeader } from "../messages/RevHeader";
    import type { StoreRef } from "../messages/StoreRef";
    import type { Operand } from "../messages/Operand";
    import type Settings from "../shell/Settings";
    import Icon from "../controls/Icon.svelte";
    import Chip from "../controls/Chip.svelte";
    import Object from "./Object.svelte";
    import Zone from "./Zone.svelte";

    interface Props {
        header: RevHeader;
        ref: Extract<StoreRef, { type: "LocalBookmark" | "RemoteBookmark" }>;
    }
    let { header, ref }: Props = $props();

    let operand: Operand = { type: "Ref", header, ref };

    let branchInfo = $derived.by(() => {
        switch (ref.type) {
            case "LocalBookmark":
                return {
                    label: ref.branch_name,
                    branchState: ref.is_synced ? "change" : "add",
                    disconnected: ref.available_remotes == 0 && ref.potential_remotes > 0,
                };
            case "RemoteBookmark":
                return {
                    label: `${ref.branch_name}@${ref.remote_name}`,
                    branchState: ref.is_tracked ? "remove" : "change",
                    disconnected: ref.is_tracked && ref.is_absent,
                };
        }
    });
    let label = $derived(branchInfo.label);
    let branchState = $derived(branchInfo.branchState) as "add" | "change" | "remove";
    let disconnected = $derived.by(() => {
        if (!getContext<Settings>("settings").markUnpushedBranches) {
            return false;
        }
        return branchInfo.disconnected;
    });
    let tip = $derived.by(() => {
        switch (ref.type) {
            case "LocalBookmark":
                if (disconnected) {
                    return "local-only bookmark";
                } else {
                    let tipText = "local bookmark";
                    if (ref.tracking_remotes.length > 0) {
                        tipText += " (tracking ";
                        let first = true;
                        for (let r of ref.tracking_remotes) {
                            if (first) {
                                first = false;
                            } else {
                                tipText += ", ";
                            }
                            tipText += r;
                        }
                        tipText += ")";
                    }
                    return tipText;
                }
            case "RemoteBookmark":
                let tipText = "remote bookmark";
                if (disconnected) {
                    return tipText + " (deleting)";
                } else if (ref.is_tracked) {
                    return tipText + " (tracked)";
                } else {
                    return tipText + " (untracked)";
                }
        }
    });
</script>

<Object {operand} {label} conflicted={ref.has_conflict}>
    {#snippet children({ context, hint: dragHint })}
        <Zone {operand}>
            {#snippet children({ target, hint: dropHint })}
                <Chip {context} {target} {disconnected} {tip}>
                    <Icon name="bookmark" state={context ? null : branchState} />
                    <span>{dragHint ?? dropHint ?? label}</span>
                </Chip>
            {/snippet}
        </Zone>
    {/snippet}
</Object>
