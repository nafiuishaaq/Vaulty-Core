'use client'

/**
 * WalletButton — Freighter wallet connect / disconnect control.
 *
 * Displays:
 *  - "Connect Wallet"  when disconnected
 *  - Truncated public key + "Disconnect" when connected
 *  - A network-mismatch warning banner when the wallet is on the wrong network
 *  - Inline error messages with recovery hints based on error kind
 */

import { useWallet } from '@/hooks/useWallet'
import { Button } from '@/components/Button'

/** Truncate a Stellar public key for display: GABCD…WXYZ */
function truncateKey(key: string): string {
  if (key.length <= 12) return key
  return `${key.slice(0, 6)}…${key.slice(-4)}`
}

export function WalletButton() {
  const {
    wallet,
    isConnecting,
    error,
    errorKind,
    networkMismatch,
    connect,
    disconnect,
    clearError,
  } = useWallet()

  return (
    <div className="flex flex-col items-end gap-2">
      {/* Network mismatch warning */}
      {networkMismatch && (
        <div
          role="alert"
          className="rounded-md bg-amber-50 border border-amber-300 px-3 py-2 text-sm text-amber-800 max-w-xs"
        >
          <span className="font-semibold">Wrong network.</span> Switch your Freighter wallet to{' '}
          <span className="font-mono">
            {process.env.NEXT_PUBLIC_STELLAR_NETWORK?.toUpperCase() ?? 'TESTNET'}
          </span>{' '}
          and reconnect.
        </div>
      )}

      {/* Connect / Disconnect button */}
      {wallet.isConnected && wallet.publicKey ? (
        <div className="flex items-center gap-3">
          <span
            className="text-sm text-slate-600 font-mono bg-slate-100 rounded px-2 py-1"
            title={wallet.publicKey}
            aria-label={`Connected wallet: ${wallet.publicKey}`}
          >
            {truncateKey(wallet.publicKey)}
          </span>
          <Button
            variant="secondary"
            size="sm"
            onClick={disconnect}
            aria-label="Disconnect wallet"
          >
            Disconnect
          </Button>
        </div>
      ) : (
        <Button
          variant="primary"
          size="sm"
          isLoading={isConnecting}
          onClick={connect}
          disabled={isConnecting}
          aria-label="Connect Freighter wallet"
        >
          Connect Wallet
        </Button>
      )}

      {/* Error display with recovery hints */}
      {error && (
        <div
          role="alert"
          className="rounded-md bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700 max-w-xs"
        >
          <p>{error}</p>

          {errorKind === 'wallet_not_installed' && (
            <a
              href="https://freighter.app"
              target="_blank"
              rel="noopener noreferrer"
              className="mt-1 block underline text-red-600 hover:text-red-800"
            >
              Install Freighter →
            </a>
          )}

          <button
            onClick={clearError}
            className="mt-1 text-xs text-red-500 underline hover:text-red-700"
            aria-label="Dismiss error"
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  )
}
