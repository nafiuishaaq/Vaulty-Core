/**
 * Unit tests for WalletManager (src/lib/stellar.ts).
 *
 * @stellar/freighter-api is mapped to src/__mocks__/freighter-api.js via
 * jest.config.js moduleNameMapper, so it never tries to load the real browser
 * extension package in Node/jsdom.
 *
 * Individual tests configure the stub's jest.fn() return values to cover
 * happy paths, user-rejection, network mismatch, and other error cases.
 */

import * as freighterApi from '@stellar/freighter-api'
import {
  WalletManager,
  WalletConnectionError,
  WalletNetworkMismatchError,
  WalletUserRejectedError,
} from '../stellar'

// ---------------------------------------------------------------------------
// Typed references to the stub fns
// ---------------------------------------------------------------------------

const mockIsConnected = freighterApi.isConnected as jest.Mock
const mockIsAllowed = freighterApi.isAllowed as jest.Mock
const mockRequestAccess = freighterApi.requestAccess as jest.Mock
const mockGetPublicKey = freighterApi.getPublicKey as jest.Mock
const mockGetNetwork = freighterApi.getNetwork as jest.Mock
const mockGetNetworkDetails = freighterApi.getNetworkDetails as jest.Mock
const mockSignTransaction = freighterApi.signTransaction as jest.Mock
const mockAddRecentAddress = freighterApi.addRecentAddress as jest.Mock

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PUBLIC_KEY = 'GABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890ABCDEFGHIJKLMNOPQRS'
const SIGNED_XDR = 'AAAA_SIGNED_XDR_BASE64=='
const UNSIGNED_XDR = 'AAAA_UNSIGNED_XDR_BASE64=='
const PASSPHRASE = 'Test SDF Network ; September 2015'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function setupHappyPath() {
  mockIsConnected.mockResolvedValue({ isConnected: true })
  mockIsAllowed.mockResolvedValue({ isAllowed: true })
  mockGetNetwork.mockResolvedValue({ network: 'TESTNET', error: undefined })
  mockGetNetworkDetails.mockResolvedValue({
    network: 'TESTNET',
    networkUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: PASSPHRASE,
    error: undefined,
  })
  mockGetPublicKey.mockResolvedValue({ publicKey: PUBLIC_KEY, error: undefined })
  mockAddRecentAddress.mockResolvedValue({})
  mockSignTransaction.mockResolvedValue({ signedTxXdr: SIGNED_XDR, error: undefined })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('WalletManager', () => {
  let manager: WalletManager

  beforeEach(() => {
    jest.clearAllMocks()
    manager = new WalletManager()
  })

  // -------------------------------------------------------------------------
  // connectWallet — happy path
  // -------------------------------------------------------------------------

  describe('connectWallet() — happy path', () => {
    it('returns the public key on successful connection', async () => {
      setupHappyPath()
      const key = await manager.connectWallet()
      expect(key).toBe(PUBLIC_KEY)
    })

    it('caches the public key in getPublicKey()', async () => {
      setupHappyPath()
      await manager.connectWallet()
      expect(manager.getPublicKey()).toBe(PUBLIC_KEY)
    })

    it('calls requestAccess when isAllowed returns false', async () => {
      setupHappyPath()
      mockIsAllowed.mockResolvedValue({ isAllowed: false })
      mockRequestAccess.mockResolvedValue({ publicKey: PUBLIC_KEY, error: undefined })

      await manager.connectWallet()
      expect(mockRequestAccess).toHaveBeenCalledTimes(1)
    })

    it('does not call requestAccess when already allowed', async () => {
      setupHappyPath()
      await manager.connectWallet()
      expect(mockRequestAccess).not.toHaveBeenCalled()
    })
  })

  // -------------------------------------------------------------------------
  // connectWallet — error cases
  // -------------------------------------------------------------------------

  describe('connectWallet() — errors', () => {
    it('throws WalletConnectionError when Freighter is not installed', async () => {
      mockIsConnected.mockResolvedValue({ isConnected: false })

      await expect(manager.connectWallet()).rejects.toThrow(WalletConnectionError)
      await expect(manager.connectWallet()).rejects.toThrow('not installed')
    })

    it('throws WalletUserRejectedError when user declines access', async () => {
      mockIsConnected.mockResolvedValue({ isConnected: true })
      mockIsAllowed.mockResolvedValue({ isAllowed: false })
      mockRequestAccess.mockResolvedValue({ error: 'User declined access' })

      await expect(manager.connectWallet()).rejects.toThrow(WalletUserRejectedError)
    })

    it('throws WalletNetworkMismatchError when wallet is on wrong network', async () => {
      mockIsConnected.mockResolvedValue({ isConnected: true })
      mockIsAllowed.mockResolvedValue({ isAllowed: true })
      mockGetNetwork.mockResolvedValue({ network: 'PUBLIC', error: undefined })

      await expect(manager.connectWallet()).rejects.toThrow(WalletNetworkMismatchError)
    })

    it('throws WalletConnectionError when getPublicKey returns an error', async () => {
      setupHappyPath()
      mockGetPublicKey.mockResolvedValue({ error: 'Could not retrieve public key' })

      await expect(manager.connectWallet()).rejects.toThrow(WalletConnectionError)
    })

    it('throws WalletConnectionError when getNetwork returns an error', async () => {
      mockIsConnected.mockResolvedValue({ isConnected: true })
      mockIsAllowed.mockResolvedValue({ isAllowed: true })
      mockGetNetwork.mockResolvedValue({ error: 'Network error' })

      await expect(manager.connectWallet()).rejects.toThrow(WalletConnectionError)
    })
  })

  // -------------------------------------------------------------------------
  // disconnectWallet
  // -------------------------------------------------------------------------

  describe('disconnectWallet()', () => {
    it('resolves without throwing', async () => {
      await expect(manager.disconnectWallet()).resolves.toBeUndefined()
    })

    it('clears the cached public key', async () => {
      setupHappyPath()
      await manager.connectWallet()
      expect(manager.getPublicKey()).toBe(PUBLIC_KEY)

      await manager.disconnectWallet()
      expect(manager.getPublicKey()).toBeNull()
    })

    it('fires account-change listeners with null', async () => {
      setupHappyPath()
      await manager.connectWallet()

      const listener = jest.fn()
      manager.onAccountChange(listener)

      await manager.disconnectWallet()
      expect(listener).toHaveBeenCalledWith(null)
    })
  })

  // -------------------------------------------------------------------------
  // signTransaction
  // -------------------------------------------------------------------------

  describe('signTransaction()', () => {
    it('throws WalletConnectionError when not connected', async () => {
      await expect(manager.signTransaction(UNSIGNED_XDR)).rejects.toThrow(WalletConnectionError)
      await expect(manager.signTransaction(UNSIGNED_XDR)).rejects.toThrow('not connected')
    })

    it('returns signed XDR after successful signing', async () => {
      setupHappyPath()
      await manager.connectWallet()

      const signed = await manager.signTransaction(UNSIGNED_XDR)
      expect(signed).toBe(SIGNED_XDR)
    })

    it('passes the correct network passphrase to Freighter', async () => {
      setupHappyPath()
      await manager.connectWallet()

      await manager.signTransaction(UNSIGNED_XDR)

      expect(mockSignTransaction).toHaveBeenCalledWith(
        UNSIGNED_XDR,
        expect.objectContaining({ networkPassphrase: PASSPHRASE })
      )
    })

    it('throws WalletUserRejectedError when user rejects signing', async () => {
      setupHappyPath()
      await manager.connectWallet()
      mockSignTransaction.mockResolvedValue({ error: 'User declined transaction signing' })

      await expect(manager.signTransaction(UNSIGNED_XDR)).rejects.toThrow(WalletUserRejectedError)
    })

    it('throws WalletConnectionError on generic signing error', async () => {
      setupHappyPath()
      await manager.connectWallet()
      mockSignTransaction.mockResolvedValue({ error: 'Signing failed: unknown reason' })

      await expect(manager.signTransaction(UNSIGNED_XDR)).rejects.toThrow(WalletConnectionError)
    })

    it('throws WalletNetworkMismatchError when network changes before signing', async () => {
      setupHappyPath()
      await manager.connectWallet()

      // Simulate network switch between connect and sign
      mockGetNetwork.mockResolvedValue({ network: 'PUBLIC', error: undefined })

      await expect(manager.signTransaction(UNSIGNED_XDR)).rejects.toThrow(WalletNetworkMismatchError)
    })
  })

  // -------------------------------------------------------------------------
  // getPublicKey
  // -------------------------------------------------------------------------

  describe('getPublicKey()', () => {
    it('returns null when not connected', () => {
      expect(manager.getPublicKey()).toBeNull()
    })

    it('returns the public key after connecting', async () => {
      setupHappyPath()
      await manager.connectWallet()
      expect(manager.getPublicKey()).toBe(PUBLIC_KEY)
    })
  })

  // -------------------------------------------------------------------------
  // checkNetworkMatch
  // -------------------------------------------------------------------------

  describe('checkNetworkMatch()', () => {
    it('returns true when wallet is on the correct network', async () => {
      mockGetNetwork.mockResolvedValue({ network: 'TESTNET', error: undefined })
      expect(await manager.checkNetworkMatch()).toBe(true)
    })

    it('returns false when wallet is on the wrong network', async () => {
      mockGetNetwork.mockResolvedValue({ network: 'PUBLIC', error: undefined })
      expect(await manager.checkNetworkMatch()).toBe(false)
    })
  })

  // -------------------------------------------------------------------------
  // Event listeners
  // -------------------------------------------------------------------------

  describe('onAccountChange()', () => {
    it('fires null when disconnectWallet is called', async () => {
      setupHappyPath()
      await manager.connectWallet()

      const cb = jest.fn()
      const unsub = manager.onAccountChange(cb)

      await manager.disconnectWallet()
      expect(cb).toHaveBeenCalledWith(null)

      unsub()
    })

    it('unsubscribe stops notifications', async () => {
      setupHappyPath()
      await manager.connectWallet()

      const cb = jest.fn()
      const unsub = manager.onAccountChange(cb)
      unsub()

      await manager.disconnectWallet()
      expect(cb).not.toHaveBeenCalled()
    })
  })

  describe('onNetworkChange()', () => {
    it('returns an unsubscribe function', () => {
      const cb = jest.fn()
      const unsub = manager.onNetworkChange(cb)
      expect(typeof unsub).toBe('function')
      unsub()
    })
  })

  // -------------------------------------------------------------------------
  // Singleton export
  // -------------------------------------------------------------------------

  describe('walletManager singleton', () => {
    it('is exported and is an instance of WalletManager', async () => {
      const { walletManager } = await import('../stellar')
      expect(walletManager).toBeInstanceOf(WalletManager)
    })
  })
})
