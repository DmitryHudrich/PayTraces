import {
  HubConnectionBuilder,
  HubConnectionState,
  LogLevel,
  type HubConnection,
} from '@microsoft/signalr'

import { getToken } from '@/shared/auth/token-storage'
import { env } from '@/shared/config/env'
import type { CaseGraphPage, NodePosition } from '@/entities/case-graph/model/graph'

/** One streamed item: an engine page plus (first item only) pinned positions. */
export type GraphStreamItem = {
  page: CaseGraphPage
  positions: NodePosition[] | null
}

export function createGraphConnection(): HubConnection {
  return new HubConnectionBuilder()
    .withUrl(env.graphHubUrl, { accessTokenFactory: () => getToken() ?? '' })
    .withAutomaticReconnect()
    .configureLogging(LogLevel.Warning)
    .build()
}

export { HubConnectionState }
export type { HubConnection }
