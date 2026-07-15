# Ledgerscope Console (frontend)

Case-centric investigation UI for the **Ledgerscope.Accounts** C# public API. Log in,
open a case, stream its transaction graph, then pin canvas views, label entities and
organise addresses into groups.

## Stack

React 19 · Vite · TypeScript · TanStack Query · Tailwind v4 · Radix/shadcn UI ·
Sigma + graphology (graph rendering) · `@microsoft/signalr` (live graph stream).
Structured with Feature-Sliced Design (`app` / `pages` / `widgets` / `features` /
`entities` / `shared`).

## Running

The frontend talks to the C# API (default `http://localhost:5107`) **through the vite
dev proxy**, because that service ships no CORS policy.

```bash
npm install
npm run dev            # http://localhost:5173
```

Start the backend separately (from `code/ledgerscope-accounts`): `dotnet run`. It also
needs Postgres + Redis (`infrastructure/docker-compose.yaml`) and the Rust `aml-core`
engine on :8080 for graph data.

Point the proxy elsewhere with `VITE_PROXY_TARGET`, or bypass the proxy entirely with
`VITE_API_URL` / `VITE_GRAPH_HUB_URL` (see `.env.example`).

## How it fits together

- **Auth** — `POST /auth/login` / `/auth/register`; JWT stored in `localStorage`
  (`shared/auth`) and attached to every request; a 401 clears the session.
- **Graph** — delivered over the SignalR hub `/hubs/graph`: `StreamCaseGraph` streams
  paged BFS around a case address (with saved view positions merged in), `ExpandNode`
  expands a node on click. See `features/case-graph-stream`.
- **Permissions** — `GET /me/permissions` drives which controls are shown
  (`entities/permission`).
- **Views** — saved canvas layouts; the sigma adapter can seed pinned positions and
  export the current ones.

## Scripts

```bash
npm run dev       # dev server
npm run build     # tsc -b && vite build
npm run lint      # oxlint
npm run lint:fsd  # steiger (FSD boundaries)
```
