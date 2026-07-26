/**
 * stellar.ts — Stellar wallet integration via Freighter browser extension.
 *
 * Design constraints:
 *  - Private keys and signing secrets NEVER touch application state.
 *  - All signing happens inside the user's wallet extension; we only pass XDR
 *    strings and receive signed XDR strings back.
 *  - Network is read from NEXT_PUBLIC_STELLAR_NETWORK at module load time and
 *    validated before every contract action.
 *  - Account-change and network-change events from Freighter are surfaced via
 *    an EventTarget so the hook layer can react without polling.
 */

import {
  isConnected,
  isAllowed,
  requestAccess,
  getPublicKey,
  getNetwork,
  getNetworkDetails,
  signTransaction,
  addRecentAddress,
} from '@stellar/freighter-api'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type StellarNetwork = 'TESTNET' | 'PUBLIC' // Freighter canonical names

/** Distinguishes wallet-connectivity failures from generic app errors. */
export class WalletConnectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'WalletConnectionError'
  }
}

/** Thrown when the user's wallet is on a different network than configured. */
export class WalletNetworkMismatchError extends Error {
  constructor(public connectedNetwork: string, public requiredNetwork: string) {
    super(
      `Wallet is connected to ${connectedNetwork}, but this app requires ${requiredNetwork}. ` +
        `Please switch your Freighter wallet to ${requiredNetwork} and try again.`
    )
    this.name = 'WalletNetworkMismatchError'
  }
}

/** Thrown when the user rejects the wallet prompt. */
export class WalletUserRejectedError extends Error {
  constructor(message = 'User rejected the wallet request.') {
    super(message)
    this.name = 'WalletUserRejectedError'
  }
}

export interface WalletNetworkDetails {
  network: string
  networkUrl: string
  networkPassphrase: string
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Map the app's env-var value to Freighter's canonical network name.
 *  NEXT_PUBLIC_STELLAR_NETWORK=testnet  → 'TESTNET'
 *  NEXT_PUBLIC_STELLAR_NETWORK=mainnet  → 'PUBLIC'
 */
function resolveConfiguredNetwork(): StellarNetwork {
  const raw = (process.env.NEXT_PUBLIC_STELLAR_NETWORK ?? 'testnet').toLowerCase()
  if (raw === 'mainnet' || raw === 'public') return 'PUBLIC'
  return 'TESTNET'
}

const CONFIGURED_NETWORK: StellarNetwork = resolveConfiguredNetwork()

/** Normalise a Freighter network string for comparison. */
function normaliseNetwork(net: string): StellarNetwork {
  const upper = net.toUpperCase()
  if (upper === 'PUBLIC' || upper === 'MAINNET') return 'PUBLIC'
  return 'TESTNET'
}

/**
 * Re-map Freighter error messages that indicate the user rejected a prompt so
 * callers can distinguish rejection from other failures.
 */
function classifyFreighterError(err: unknown): never {
  const msg = err instanceof Error ? err.message : String(err)

  // Freighter surfaces rejection as these strings (as of v4.x)
  if (
    msg.includes('User declined') ||
    msg.includes('User rejected') ||
    msg.includes('rejected') ||
    msg.includes('declined')
  ) {
    throw new WalletUserRejectedError()
  }

  if (
    msg.includes('not installed') ||
    msg.includes('not connected') ||
    msg.includes('not allowed') ||
    msg.includes('Extension not found')
  ) {
    throw new WalletConnectionError(msg)
  }

  throw new WalletConnectionError(msg)
}

// ---------------------------------------------------------------------------
// WalletManager
// ---------------------------------------------------------------------------

/**
 * WalletManager wraps the Freighter browser-extension API.
 *
 * Lifecycle:
 *  1. `connectWallet()` — requests access, validates network, returns public key.
 *  2. `signTransaction(xdr)` — validates network, signs via Freighter, returns signed XDR.
 *  3. `disconnectWallet()` — clears cached state; Freighter has no programmatic
 *     disconnect, so we just remove our local reference.
 *
 * Event listeners:
 *  `onAccountChange(cb)` and `onNetworkChange(cb)` wire up Freighter's window
 *  message events so the hook layer can react to changes without polling.
 */
export class WalletManager {
  /** Cached public key — only used for `getPublicKey()`, never for signing. */
  private _publicKey: string | null = null

  private _accountChangeListeners: Set<(publicKey: string | null) => void> = new Set()
  private _networkChangeListeners: Set<(network: string) => void> = new Set()
  private _windowMessageHandler: ((event: MessageEvent) => void) | null = null

  constructor() {
    // Only attach window listener in browser context
    if (typeof window !== 'undefined') {
      this._attachWindowListeners()
    }
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Connect to the user's Freighter wallet.
   *
   * Steps:
   *  1. Verify Freighter extension is installed.
   *  2. Request access (prompts user if not already granted).
   *  3. Validate that the wallet's active network matches the configured network.
   *  4. Return the public key — never the secret key.
   */
  async connectWallet(): Promise<string> {
    try {
      // 1. Check Freighter is installed
      const connected = await isConnected()
      if (!connected || !connected.isConnected) {
        throw new WalletConnectionError(
          'Freighter wallet extension is not installed. ' +
            'Please install Freighter from https://freighter.app and try again.'
        )
      }

      // 2. Check / request access
      const allowed = await isAllowed()
      if (!allowed || !allowed.isAllowed) {
        const accessResult = await requestAccess()
        if (accessResult.error) {
          // requestAccess surfaces rejection as an error field
          if (
            accessResult.error.includes('User declined') ||
            accessResult.error.includes('rejected')
          ) {
            throw new WalletUserRejectedError()
          }
          throw new WalletConnectionError(accessResult.error)
        }
      }

      // 3. Validate network before trusting any data from the wallet
      await this._assertNetwork()

      // 4. Retrieve public key — private key stays in the extension
      const pkResult = await getPublicKey()
      if (pkResult.error) {
        throw new WalletConnectionError(pkResult.error)
      }

      this._publicKey = pkResult.publicKey

      // Hint Freighter to surface this address as "recently used"
      try {
        await addRecentAddress({ publicKey: this._publicKey })
      } catch {
        // Non-critical — ignore silently
      }

      return this._publicKey
    } catch (err) {
      // Re-throw our own error classes as-is; wrap anything else
      if (
        err instanceof WalletConnectionError ||
        err instanceof WalletNetworkMismatchError ||
        err instanceof WalletUserRejectedError
      ) {
        throw err
      }
      classifyFreighterError(err)
    }
  }

  /**
   * Sign a Stellar transaction XDR string using the connected wallet.
   *
   * The private key never leaves Freighter — we pass the raw XDR, the user
   * approves in the extension UI, and we get back the signed XDR.
   *
   * @param transactionXDR  Base64-encoded unsigned transaction XDR
   * @returns               Base64-encoded signed transaction XDR
   */
  async signTransaction(transactionXDR: string): Promise<string> {
    if (!this._publicKey) {
      throw new WalletConnectionError('Wallet not connected. Please connect your wallet first.')
    }

    try {
      // Always validate network before contract actions
      await this._assertNetwork()

      const networkDetails = await this._getNetworkDetails()

      const result = await signTransaction(transactionXDR, {
        networkPassphrase: networkDetails.networkPassphrase,
        address: this._publicKey,
      })

      if (result.error) {
        if (
          result.error.includes('User declined') ||
          result.error.includes('rejected') ||
          result.error.includes('declined')
        ) {
          throw new WalletUserRejectedError('Transaction signing was rejected by the user.')
        }
        throw new WalletConnectionError(`Signing failed: ${result.error}`)
      }

      return result.signedTxXdr
    } catch (err) {
      if (
        err instanceof WalletConnectionError ||
        err instanceof WalletNetworkMismatchError ||
        err instanceof WalletUserRejectedError
      ) {
        throw err
      }
      classifyFreighterError(err)
    }
  }

  /**
   * Disconnect the wallet from the app's perspective.
   *
   * Freighter has no programmatic "revoke access" API, so we clear our local
   * state and fire account-change listeners with `null`. The user can revoke
   * app access in the Freighter extension settings.
   */
  async disconnectWallet(): Promise<void> {
    this._publicKey = null
    this._notifyAccountChange(null)
  }

  /** Returns the cached public key, or null if not connected. */
  getPublicKey(): string | null {
    return this._publicKey
  }

  /** Returns the app's configured network. */
  getConfiguredNetwork(): StellarNetwork {
    return CONFIGURED_NETWORK
  }

  /**
   * Check whether the wallet's active network matches the configured one,
   * without throwing — useful for displaying a warning banner.
   */
  async checkNetworkMatch(): Promise<boolean> {
    try {
      await this._assertNetwork()
      return true
    } catch {
      return false
    }
  }

  // -------------------------------------------------------------------------
  // Event subscription
  // -------------------------------------------------------------------------

  /**
   * Subscribe to account-change events (Freighter account switched, or
   * disconnected from our side via `disconnectWallet`).
   *
   * @returns Unsubscribe function
   */
  onAccountChange(cb: (publicKey: string | null) => void): () => void {
    this._accountChangeListeners.add(cb)
    return () => this._accountChangeListeners.delete(cb)
  }

  /**
   * Subscribe to network-change events (user switched network in Freighter).
   *
   * @returns Unsubscribe function
   */
  onNetworkChange(cb: (network: string) => void): () => void {
    this._networkChangeListeners.add(cb)
    return () => this._networkChangeListeners.delete(cb)
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  private async _assertNetwork(): Promise<void> {
    const networkResult = await getNetwork()
    if (networkResult.error) {
      throw new WalletConnectionError(`Unable to read wallet network: ${networkResult.error}`)
    }

    const walletNetwork = normaliseNetwork(networkResult.network)
    if (walletNetwork !== CONFIGURED_NETWORK) {
      throw new WalletNetworkMismatchError(networkResult.network, CONFIGURED_NETWORK)
    }
  }

  private async _getNetworkDetails(): Promise<WalletNetworkDetails> {
    const result = await getNetworkDetails()
    if (result.error) {
      throw new WalletConnectionError(`Unable to read network details: ${result.error}`)
    }
    return {
      network: result.network,
      networkUrl: result.networkUrl,
      networkPassphrase: result.networkPassphrase,
    }
  }

  private _notifyAccountChange(publicKey: string | null): void {
    this._accountChangeListeners.forEach((cb) => {
      try {
        cb(publicKey)
      } catch {
        // Listeners must not crash the manager
      }
    })
  }

  private _notifyNetworkChange(network: string): void {
    this._networkChangeListeners.forEach((cb) => {
      try {
        cb(network)
      } catch {
        // Listeners must not crash the manager
      }
    })
  }

  /**
   * Freighter communicates account/network changes via `window.postMessage`.
   * We intercept those messages and dispatch them to our own listeners.
   *
   * Message shape (Freighter v4.x):
   *   { source: 'FREIGHTER_API', type: 'ACCOUNT_CHANGED', publicKey: string }
   *   { source: 'FREIGHTER_API', type: 'NETWORK_CHANGED', network: string }
   */
  private _attachWindowListeners(): void {
    this._windowMessageHandler = (event: MessageEvent) => {
      if (!event.data || event.data.source !== 'FREIGHTER_API') return

      switch (event.data.type) {
        case 'ACCOUNT_CHANGED': {
          const newKey: string | null = event.data.publicKey ?? null
          this._publicKey = newKey
          this._notifyAccountChange(newKey)
          break
        }
        case 'NETWORK_CHANGED': {
          const newNet: string = event.data.network ?? ''
          this._notifyNetworkChange(newNet)
          // If the new network doesn't match configured, clear the cached key
          // so the next action triggers a re-validation via connectWallet.
          if (normaliseNetwork(newNet) !== CONFIGURED_NETWORK) {
            this._publicKey = null
            this._notifyAccountChange(null)
          }
          break
        }
      }
    }

    window.addEventListener('message', this._windowMessageHandler)
  }

  /** Tear down the window listener (call in cleanup if needed). */
  destroy(): void {
    if (this._windowMessageHandler && typeof window !== 'undefined') {
      window.removeEventListener('message', this._windowMessageHandler)
      this._windowMessageHandler = null
    }
    this._accountChangeListeners.clear()
    this._networkChangeListeners.clear()
  }
}

// Singleton — shared across the app.
export const walletManager = new WalletManager()
