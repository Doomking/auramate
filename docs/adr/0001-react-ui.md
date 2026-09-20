# React for the AuraMate UI

Phase 1 originally named Svelte in the requirement doc; the maintainer chose React instead. We stay on the official `create-tauri-app` `react-ts` template (Vite + React + TypeScript). Domain logic remains in Rust; React is only the ambient surface.

**Considered options:** Svelte/SvelteKit (earlier lock), Vue — rejected in favour of React for maintainer familiarity and ecosystem fit.

**Consequences:** Frontend is Vite+React, not SvelteKit; `frontendDist` is `dist`. Replacing Svelte after scaffold means one-time churn on the empty template only.
