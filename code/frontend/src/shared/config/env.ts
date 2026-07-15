// The public API is the C# Ledgerscope.Accounts service. In dev we go through
// the vite proxy (same-origin) because that service ships no CORS policy; the
// proxy rewrites `/api/*` -> `/*` and upgrades `/hubs/*` to websockets.
export const env = {
  apiBaseUrl: import.meta.env.VITE_API_URL ?? '/api',
  graphHubUrl: import.meta.env.VITE_GRAPH_HUB_URL ?? '/hubs/graph',
  apiVersion: '1',
}
