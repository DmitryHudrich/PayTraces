import { motion } from 'framer-motion'
import { Link } from 'react-router-dom'

import { Button } from '@/shared/ui/button'

export const HomePage = () => {
  return (
    <main className='min-h-screen bg-background text-foreground'>
      <section className='mx-auto flex w-full max-w-3xl flex-col items-start gap-6 px-6 py-24'>
        <motion.h1
          className='text-4xl font-semibold tracking-tight'
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.35 }}
        >
          PayTraces Frontend
        </motion.h1>

        <p className='text-muted-foreground'>
          Open mock transaction graph page built with full FSD layers: entities, features, widgets, pages.
        </p>

        <Button asChild>
          <Link to='/transaction-graph'>Open transaction graph</Link>
        </Button>
        <Button asChild variant='outline'>
          <Link to='/transaction-graph-preview'>Open preview graph</Link>
        </Button>
      </section>
    </main>
  )
}
