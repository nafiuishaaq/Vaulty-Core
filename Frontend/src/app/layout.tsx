import type { Metadata } from 'next'
import './globals.css'
import { WalletAccountMenu } from '@/components/WalletAccountMenu'

export const metadata: Metadata = {
  title: 'Vaulty — Save Consistently. Grow Your Wealth.',
  description:
    'A non-custodial decentralized savings platform on the Stellar network. ' +
    'Save consistently, track streaks, and grow your wealth.',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-slate-50 text-slate-900 antialiased">
        {/* Global navigation header with wallet connect control */}
        <header className="border-b border-slate-200 bg-white shadow-sm">
          <div className="container mx-auto flex items-center justify-between px-4 py-3">
            <span className="text-lg font-bold tracking-tight text-slate-900">Vaulty</span>
            {/* WalletAccountMenu is a Client Component — safe to render in a Server layout */}
            <WalletAccountMenu />
          </div>
        </header>

        <main>{children}</main>
      </body>
    </html>
  )
}