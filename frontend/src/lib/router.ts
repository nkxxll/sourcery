import { writable } from "svelte/store";

const normalizePath = (path: string) =>
    path === "/" ? "/" : path.replace(/\/+$/, "") || "/";

const initialPath =
    typeof window === "undefined" ? "/" : normalizePath(window.location.pathname);

export const currentPath = writable(initialPath);

let latestPath = initialPath;
currentPath.subscribe((path) => {
    latestPath = path;
});

const syncRoute = () => {
    currentPath.set(normalizePath(window.location.pathname));
};

export const startRouter = () => {
    if (typeof window === "undefined") {
        return () => {};
    }

    syncRoute();
    window.addEventListener("popstate", syncRoute);
    return () => window.removeEventListener("popstate", syncRoute);
};

export const navigate = (path: string, options: { replace?: boolean } = {}) => {
    if (typeof window === "undefined") {
        return;
    }

    const nextPath = normalizePath(path);
    if (nextPath === latestPath) {
        return;
    }

    if (options.replace) {
        window.history.replaceState({}, "", nextPath);
    } else {
        window.history.pushState({}, "", nextPath);
    }

    currentPath.set(nextPath);
};
