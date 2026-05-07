<script lang="ts">
    import Button from "@lib/Button.svelte";
    import type { HTMLButtonAttributes } from "svelte/elements";

    type Props = HTMLButtonAttributes & {
        text: string;
    };

    let { text, ...rest }: Props = $props();
</script>

<li>
    <Button class="align-middle text-lg" {...rest}>{text}</Button>
</li>
