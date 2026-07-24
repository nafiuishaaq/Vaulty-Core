'use client'

/**
 * useWallet — React hook for Stellar wallet connection state.
 *
 * Handles:
 *  - Connect / disconnect lifecycle
 *  - Rejected signatures (WalletUserRejectedError)
 *  - Disconnects and account changes (Freighter event listeners)
 *  - Network changes (auto-disconnect + mismatch banner flag)
 *  - Private keys never enter this hook or app state
 *  - Network read from NEXT_PUBLIC_STELLAR_NETWORK env var, not hardcoded
 */

import { useState, useEffect, useCallback, useRef } from 'react'
import { useAppStore } from '@/stores'
import {
  walletManager,
  WalletConnectionError,
  WalletNetworkMismatchError,
  WalletUserRejectedError,
} from '@/lib/stellar'

export type WalletErrorKind =
  | 'wallet_not_installed'
  | 'user_rejected'
  | 'network_mismatch'
  | 'connection_error'
  | 'signing_error'
  | null

export interface UseWalletReturn {
  /** Current wallet state from the global store */
  wallet: ReturnType<typeof useAppStore>['wallet']
  /** True while a connect() or disconnect() call is in flight */
  isConnecting: boolean
  /** Human-readable error message, or null */
  error: string | null
  /** Structured error kind for targeted UI recovery copy */
  errorKind: WalletErrorKind
  /** True if a wallet-specific error occurred (legacy compat) */
  isWalletError: boolean
  /** True if the wallet's active network doesn't match configured network */
  networkMismatch: boolean
  /** Initiate wallet connection flow */
  connect: () => Promise<void>
  /** Disconnect wallet from the app */
  disconnect: () => Promise<void>
  /** Clear current error state */
  clearError: () => void
}

export function useWallet(): UseWalletReturn {
  const { wallet, setWalletConnected, setWalletDisconnected, setNetworkMismatch } = useAppStore()
  const [isConnecting, setIsConnecting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [errorKind, setErrorKind] = useState<WalletErrorKind>(null)
  const [networkMismatch, setNetworkMismatchLocal] = useState(false)

  // Track whether the component is still mounted to avoid setState-after-unmount
  const isMountedRef = useRef(true)

  // -------------------------------------------------------------------------
  // Freighter event listeners — account change & network change
  // -------------------------------------------------------------------------
  useEffect(() => {
    isMountedRef.current = true

    const unsubAccount = walletManager.onAccountChange((publicKey) => {
      if (!isMountedRef.current) return
      if (publicKey === null) {
        // Account was switched away or disconnected
        setWalletDisconnected()
        setNetworkMismatch(false)
        setNetworkMismatchLocal(false)
      } else {
        // Account was switched to a different key — re-validate and update
        const network = walletManager.getConfiguredNetwork().toLowerCase() as 'testnet' | 'mainnet'
        setWalletConnected(publicKey, network === 'public' ? 'mainnet' : network)
      }
    })

    const unsubNetwork = walletManager.onNetworkChange((newNetwork) => {
      if (!isMountedRef.current) return
      const configuredNet = walletManager.getConfiguredNetwork()
      const mismatch =
        newNetwork.toUpperCase() !== configuredNet &&
        !(newNetwork.toUpperCase() === 'PUBLIC' && configuredNet === 'PUBLIC')

      setNetworkMismatchLocal(mismatch)
      setNetworkMismatch(mismatch)

      if (mismatch) {
        // Disconnect the app session when network changes to a wrong network
        setWalletDisconnected()
        setError(
          `Wallet network changed to ${newNetwork}. ` +
            `Please switch back to ${configuredNet} in Freighter.`
        )
        setErrorKind('network_mismatch')
      }
    })

    return () => {
      isMountedRef.current = false
      unsubAccount()
      unsubNetwork()
    }
  }, [setWalletConnected, setWalletDisconnected, setNetworkMismatch])

  // -------------------------------------------------------------------------
  // Helpers
  // -------------------------------------------------------------------------

  const classifyError = useCallback((err: unknown): void => {
    if (err instanceof WalletUserRejectedError) {
      setError('You rejected the wallet request. Click "Connect" to try again.')
      setErrorKind('user_rejected')
      return
    }
    if (err instanceof WalletNetworkMismatchError) {
      setError(err.message)
      setErrorKind('network_mismatch')
      setNetworkMismatchLocal(true)
      return
    }
    if (err instanceof WalletConnectionError) {
      const msg = err.message
      if (msg.includes('not installed') || msg.includes('Extension not found')) {
        setError(
          'Freighter wallet is not installed. ' +
            'Install it from https://freighter.app and reload the page.'
        )
        setErrorKind('wallet_not_installed')
      } else {
        setError(msg)
        setErrorKind('connection_error')
      }
      return
    }
    // Generic / unexpected errors
    setError(err instanceof Error ? err.message : 'Failed to connect wallet')
    setErrorKind('connection_error')
  }, [])

  // -------------------------------------------------------------------------
  // Connect
  // -------------------------------------------------------------------------

  const connect = useCallback(async () => {
    setIsConnecting(true)
    setError(null)
    setErrorKind(null)
    setNetworkMismatchLocal(false)

    try {
      const publicKey = await walletManager.connectWallet()

      // Derive network label from env — never hardcode
      const configuredNet = walletManager.getConfiguredNetwork()
      const networkLabel: 'testnet' | 'mainnet' =
        configuredNet === 'PUBLIC' ? 'mainnet' : 'testnet'

      setWalletConnected(publicKey, networkLabel)
      setNetworkMismatch(false)
    } catch (err) {
      classifyError(err)
    } finally {
      if (isMountedRef.current) {
        setIsConnecting(false)
      }
    }
  }, [setWalletConnected, setNetworkMismatch, classifyError])

  // -------------------------------------------------------------------------
  // Disconnect
  // -------------------------------------------------------------------------

  const disconnect = useCallback(async () => {
    setError(null)
    setErrorKind(null)
    try {
      await walletManager.disconnectWallet()
      setWalletDisconnected()
      setNetworkMismatch(false)
      setNetworkMismatchLocal(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to disconnect wallet')
      setErrorKind('connection_error')
    }
  }, [setWalletDisconnected, setNetworkMismatch])

  // -------------------------------------------------------------------------
  // Clear error
  // -------------------------------------------------------------------------

  const clearError = useCallback(() => {
    setError(null)
    setErrorKind(null)
  }, [])

  return {
    wallet,
    isConnecting,
    error,
    errorKind,
    isWalletError:
      errorKind === 'wallet_not_installed' ||
      errorKind === 'connection_error' ||
      errorKind === 'user_rejected',
    networkMismatch,
    connect,
    disconnect,
    clearError,
  }
}
