<script lang="ts">
    import type { HTMLButtonAttributes } from "svelte/elements";

    type Props = HTMLButtonAttributes;

    let { class: className = "", children, type = "button", ...rest }: Props = $props();
</script>

<button
    type={type}
    class={[
        "inline-flex items-center justify-center rounded-md bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-sm transition hover:bg-indigo-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600 disabled:cursor-not-allowed disabled:opacity-50",
        className,
    ]}
    {...rest}
>
    {@render children?.()}
</button>
