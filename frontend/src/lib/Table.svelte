<script lang="ts">
    import TableHeader from "@lib/TableHeader.svelte";
    import TableElement from "@lib/TableElement.svelte";
    const props = $props();
    const keys = $derived(Object.keys(props.rows));
</script>

<div id="table" class="flex flex-row w-full border border-red-600">
    {#each keys as key}
        <div class="grow border border-red-600">
            <TableHeader {key} />
            <ul class="m-0 flex list-none flex-col gap-2 p-4">
                {#each props.rows[key] as r}
                    <TableElement text={r} onclick={() => {}} />
                {/each}
            </ul>
        </div>
    {/each}
</div>
