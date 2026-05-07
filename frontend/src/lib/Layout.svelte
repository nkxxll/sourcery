<script lang="ts">
    export let currentPath: string;
    export let navigate: (path: string) => void;
    export let links: Array<{ path: string; label: string }> = [];
</script>

<div class="app-layout">
    <header class="top-nav">
        <nav aria-label="Primary navigation">
            {#each links as link}
                <a
                    href={link.path}
                    class:active={currentPath === link.path}
                    on:click|preventDefault={() => navigate(link.path)}
                >
                    {link.label}
                </a>
            {/each}
        </nav>
    </header>

    <main class="page-content">
        <slot />
    </main>
</div>

<style>
    .app-layout {
        min-height: 100vh;
    }

    .top-nav {
        position: sticky;
        top: 0;
        z-index: 10;
        border-bottom: 1px solid #d0d7de;
        background: #ffffff;
        padding: 0.75rem 1rem;
    }

    nav a {
        color: #4d4f53;
        font-weight: 500;
        margin-right: 1rem;
        text-decoration: none;
    }

    nav a.active {
        color: #0f3f88;
    }

    .page-content {
        padding: 1rem;
    }
</style>
