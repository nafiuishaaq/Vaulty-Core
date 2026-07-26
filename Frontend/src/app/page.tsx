'use client'

import { useState } from 'react'
import { useVault } from '@/hooks/useVault'
import { useWallet } from '@/hooks/useWallet'
import { usePaymentStatus } from '@/hooks/usePaymentStatus'
import { VaultList } from '@/features/vaults'
import { VaultDetail } from '@/features/vaults'
import { CreateVault } from '@/features/vaults'
import { StreakTracker, StreakCalendar } from '@/features/streaks'
import { Vault } from '@/types'

type ViewState =
  | { type: 'list' }
  | { type: 'create' }
  | { type: 'detail'; vault: Vault }

export default function Home() {
  const { vaults } = useVault()
  const { fundingOrders, withdrawalOrders } = usePaymentStatus()
  const { wallet, networkMismatch } = useWallet()

  const [view, setView] = useState<ViewState>({ type: 'list' })

  // Active orders: those that are neither failed nor expired
  const activeOrders = [...fundingOrders, ...withdrawalOrders].filter(
    (o) => o.status !== 'failed' && o.status !== 'expired'
  )

  const firstVaultId = vaults[0]?.id

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-3xl font-bold mb-2">Vaulty</h1>

      {/* Network mismatch inline notice (also shown in WalletButton, belt-and-braces) */}
      {networkMismatch && (
        <div
          role="alert"
          className="mb-4 rounded-md bg-amber-50 border border-amber-300 px-4 py-3 text-sm text-amber-800"
        >
          Your wallet is connected to the wrong Stellar network. Vault actions are disabled
          until you switch to{' '}
          <strong>{process.env.NEXT_PUBLIC_STELLAR_NETWORK?.toUpperCase() ?? 'TESTNET'}</strong> in
          Freighter.
        </div>
      )}

      {/* Wallet not connected notice */}
      {!wallet.isConnected && (
        <p className="mb-4 text-sm text-slate-500">
          Connect your Freighter wallet using the button in the top-right corner to interact with
          your vaults on-chain.
        </p>
      )}

      {/* Active orders count */}
      {activeOrders.length > 0 && (
        <p className="mb-4 text-sm text-slate-600">
          {activeOrders.length} active payment order{activeOrders.length !== 1 ? 's' : ''}
        </p>
      )}

      {/* Navigation */}
      <div className="flex gap-4 mb-6">
        <button
          onClick={() => setView({ type: 'list' })}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
        >
          Vaults
        </button>
        <button
          onClick={() => setView({ type: 'create' })}
          className="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700"
          disabled={!wallet.isConnected || networkMismatch}
          title={
            !wallet.isConnected
              ? 'Connect your wallet to create a vault'
              : networkMismatch
                ? 'Switch to the correct network first'
                : undefined
          }
        >
          Create Vault
        </button>
        {firstVaultId && (
          <button
            onClick={() => setView({ type: 'detail', vault: vaults[0] })}
            className="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700"
          >
            View Detail
          </button>
        )}
      </div>

      {/* Streak tracking section */}
      {wallet.isConnected && (
        <div className="mb-8 grid gap-6 md:grid-cols-2">
          <StreakTracker />
          <StreakCalendar />
        </div>
      )}

      {/* Render Views */}
      {view.type === 'list' && <VaultList />}
      {view.type === 'create' && <CreateVault />}
      {view.type === 'detail' && view.vault && <VaultDetail vaultId={view.vault.id} />}
    </div>
  )
}
