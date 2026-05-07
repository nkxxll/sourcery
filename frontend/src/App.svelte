<script lang="ts">
    import { onMount } from "svelte";
    import CodeBase from "@lib/CodeBase.svelte";
    import DiffGraph from "@lib/DiffGraph.svelte";
    import Layout from "@lib/Layout.svelte";
    import Table from "@lib/Table.svelte";
    import { currentPath, navigate, startRouter } from "@lib/router";

    const rows = {
        golang: ["go-yaml", "echo", "bubbletea"],
        ocaml: ["ocaml-yaml", "dream"],
        elixir: ["phoenix", "jason"],
        c: ["cjson", "raylib"],
    };
    const navLinks = [
        { path: "/", label: "Home" },
        { path: "/codebase", label: "Codebase" },
        { path: "/diff-graph", label: "Diff graph" },
    ];

    onMount(() => {
        return startRouter();
    });
</script>

<Layout currentPath={$currentPath} {navigate} links={navLinks}>
    {#if $currentPath === "/"}
        <Table {rows} />
    {:else if $currentPath === "/codebase"}
        <CodeBase />
    {:else if $currentPath === "/diff-graph"}
        <DiffGraph />
    {:else}
        <section class="not-found">
            <h1>Page not found</h1>
            <p>
                No route exists for <code>{$currentPath}</code
                >.
            </p>
            <a href="/" on:click|preventDefault={() => navigate("/")}>Go to home</a>
        </section>
    {/if}
</Layout>

<style>
    .not-found {
        display: grid;
        gap: 0.75rem;
        max-width: 40rem;
    }

    .not-found h1 {
        font-size: 1.5rem;
        font-weight: 600;
    }

    .not-found a {
        width: fit-content;
        color: #0f3f88;
        text-decoration: underline;
    }
</style>
