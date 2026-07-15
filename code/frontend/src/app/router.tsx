import { createBrowserRouter } from 'react-router-dom'

import { RequireAuth } from '@/app/ui/RequireAuth'
import { RouteErrorPage } from '@/app/ui/RouteErrorPage'
import { LoginPage } from '@/pages/login'
import { CasesPage } from '@/pages/cases'
import { CaseWorkspacePage } from '@/pages/case-workspace'
import { AppShell } from '@/widgets/app-shell/ui/AppShell'

export const router = createBrowserRouter([
  {
    path: '/login',
    element: <LoginPage />,
  },
  {
    element: <RequireAuth />,
    errorElement: <RouteErrorPage />,
    children: [
      {
        element: <AppShell />,
        children: [
          { index: true, element: <CasesPage /> },
          { path: 'cases/:caseId', element: <CaseWorkspacePage /> },
        ],
      },
    ],
  },
])
