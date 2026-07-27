'use client'

/**
 * WalletAccountMenu — Accessible, responsive wallet connection menu that shows:
 *  - Connected/disconnected state on all pages
 *  - Shortened public key
 *  - Current network (testnet/mainnet)
 *  - Connect, disconnect, and error states
 *  - Full keyboard and screen reader support
 */

import { useState, useRef, useEffect } from 'react'
import { useWallet } from '@/hooks/useWallet'
import { Button } from '@/components/Button'

/** Truncate a Stellar public key for display: GABCD…WXYZ */
function truncateKey(key: string): string {
  if (key.length <= 12) return key
  return `${key.slice(0, 6)}…${key.slice(-4)}`
}

export function WalletAccountMenu() {
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

  const [isOpen, setIsOpen] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        menuRef.current &&
        !menuRef.current.contains(event.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false)
      }
    }

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOpen(false)
        triggerRef.current?.focus()
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside)
      document.addEventListener('keydown', handleEscape)
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [isOpen])

  // Focus first menu item when opened
  useEffect(() => {
    if (isOpen && menuRef.current) {
      const firstButton = menuRef.current.querySelector('button')
      firstButton?.focus()
    }
  }, [isOpen])

  // Keyboard navigation for dropdown
  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (!isOpen) {
      if (event.key === 'Enter' || event.key === ' ' || event.key === 'ArrowDown') {
        event.preventDefault()
        setIsOpen(true)
      }
      return
    }

    const focusableElements = menuRef.current?.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    )
    if (!focusableElements) return

    const firstElement = focusableElements[0] as HTMLElement
    const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement
    const currentIndex = Array.from(focusableElements).indexOf(event.target as HTMLElement)

    if (event.key === 'ArrowDown') {
      event.preventDefault()
      const nextIndex = currentIndex < focusableElements.length - 1 ? currentIndex + 1 : 0
      ;(focusableElements[nextIndex] as HTMLElement).focus()
    } else if (event.key === 'ArrowUp') {
      event.preventDefault()
      const prevIndex = currentIndex > 0 ? currentIndex - 1 : focusableElements.length - 1
      ;(focusableElements[prevIndex] as HTMLElement).focus()
    } else if (event.key === 'Tab') {
      if (!event.shiftKey && event.target === lastElement) {
        setIsOpen(false)
      } else if (event.shiftKey && event.target === firstElement) {
        setIsOpen(false)
      }
    }
  }

  const handleDisconnect = () => {
    disconnect()
    setIsOpen(false)
  }

  return (
    <div className="relative">
      {/* Network mismatch warning */}
      {networkMismatch && (
        <div
          role="alert"
          className="absolute right-0 bottom-full mb-2 rounded-md bg-amber-50 border border-amber-300 px-3 py-2 text-sm text-amber-800 max-w-xs"
        >
          <span className="font-semibold">Wrong network.</span> Switch your Freighter wallet to{' '}
          <span className="font-mono">
            {process.env.NEXT_PUBLIC_STELLAR_NETWORK?.toUpperCase() ?? 'TESTNET'}
          </span>{' '}
          and reconnect.
        </div>
      )}

      {/* Error display */}
      {error && (
        <div
          role="alert"
          className="absolute right-0 bottom-full mb-2 rounded-md bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700 max-w-xs"
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

      {/* Trigger button */}
      {wallet.isConnected && wallet.publicKey ? (
        <>
          <button
            ref={triggerRef}
            onClick={() => setIsOpen(!isOpen)}
            onKeyDown={handleKeyDown}
            aria-expanded={isOpen}
            aria-haspopup="true"
            className="flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
          >
            <span className="flex flex-col items-end">
              <span
                className="text-sm font-mono text-slate-700"
                title={wallet.publicKey}
                aria-label={`Connected wallet: ${wallet.publicKey}`}
              >
                {truncateKey(wallet.publicKey)}
              </span>
              <span className="text-xs text-slate-500 capitalize">
                {wallet.network}
              </span>
            </span>
            <svg
              className={`w-4 h-4 text-slate-400 transition-transform ${isOpen ? 'rotate-180' : ''}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>

          {/* Dropdown menu */}
          {isOpen && (
            <div
              ref={menuRef}
              className="absolute right-0 top-full mt-2 w-56 rounded-md bg-white border border-slate-200 shadow-lg py-1 z-50"
              role="menu"
              aria-labelledby="wallet-menu-trigger"
            >
              <div className="px-4 py-3 border-b border-slate-100">
                <p className="text-xs font-medium text-slate-500 uppercase tracking-wide">Connected</p>
                <p className="text-sm font-mono text-slate-700 mt-1">{truncateKey(wallet.publicKey)}</p>
                <p className="text-xs text-slate-500 mt-1 capitalize">Network: {wallet.network}</p>
              </div>
              <button
                onClick={handleDisconnect}
                className="w-full text-left px-4 py-2 text-sm text-red-600 hover:bg-red-50 transition-colors focus:outline-none focus:bg-red-50"
                role="menuitem"
              >
                Disconnect wallet
              </button>
            </div>
          )}
        </>
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
    </div>
  )
}