import { createBrowserRouter } from 'react-router-dom'

import { HomePage } from '@/pages/home'
import { TransactionGraphPage } from '@/pages/transaction-graph'
import { TransactionGraphPreviewPage } from '@/pages/transaction-graph-preview'

export const router = createBrowserRouter([
  {
    path: '/',
    element: <HomePage />,
  },
  {
    path: '/transaction-graph',
    element: <TransactionGraphPage />,
  },
  {
    path: '/transaction-graph-preview',
    element: <TransactionGraphPreviewPage />,
  },
])
