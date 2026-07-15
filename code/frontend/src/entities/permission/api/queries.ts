import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'

import { getMyPermissions } from '@/entities/permission/api/permissions'
import { PermissionSet } from '@/entities/permission/model/permissions'

export const permissionKeys = {
  all: ['permissions'] as const,
  scope: (caseId?: string) => ['permissions', caseId ?? 'global'] as const,
}

export function useMyPermissions(caseId?: string) {
  const query = useQuery({
    queryKey: permissionKeys.scope(caseId),
    queryFn: () => getMyPermissions(caseId),
    staleTime: 60_000,
  })

  const permissions = useMemo(() => new PermissionSet(query.data ?? []), [query.data])

  return { ...query, permissions }
}
