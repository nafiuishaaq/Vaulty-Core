/**
 * Unit tests for the useWallet hook.
 *
 * Strategy:
 *  - Mock walletManager (src/lib/stellar) to avoid needing a real browser extension.
 *  - Mock the zustand store to spy on wallet state mutations.
 *  - Cover: happy-path connect/disconnect, rejected signatures, network mismatch,
 *    account-change events, network-change events, and error clearing.
 */

import { renderHook, act } from '@testing-library/react'
import { useWallet } from '../useWallet'

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// Capture the event listener callbacks so we can fire them in tests
let accountChangeCallback: ((key: string | null) => void) | null = null
let networkChangeCallback: ((net: string) => void) | null = null

jest.mock('@/lib/stellar', () => {
  class WalletConnectionError extends Error {
    constructor(message: string) {
      super(message)
      this.name = 'WalletConnectionError'
    }
  }

  class WalletNetworkMismatchError extends Error {
    constructor(public connectedNetwork: string, public requiredNetwork: string) {
      super(`Wallet is connected to ${connectedNetwork}, but this app requires ${requiredNetwork}.`)
      this.name = 'WalletNetworkMismatchError'
    }
  }

  class WalletUserRejectedError extends Error {
    constructor(message = 'User rejected the wallet request.') {
      super(message)
      this.name = 'WalletUserRejectedError'
    }
  }

  return {
    walletManager: {
      connectWallet: jest.fn(),
      disconnectWallet: jest.fn(),
      getConfiguredNetwork: jest.fn().mockReturnValue('TESTNET'),
      onAccountChange: jest.fn((cb) => {
        accountChangeCallback = cb
        return () => { accountChangeCallback = null }
      }),
      onNetworkChange: jest.fn((cb) => {
        networkChangeCallback = cb
        return () => { networkChangeCallback = null }
      }),
    },
    WalletConnectionError,
    WalletNetworkMismatchError,
    WalletUserRejectedError,
  }
})

jest.mock('@/stores', () => ({
  useAppStore: jest.fn(),
}))

import { walletManager, WalletConnectionError, WalletNetworkMismatchError, WalletUserRejectedError } from '@/lib/stellar'
import { useAppStore } from '@/stores'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const mockConnect = walletManager.connectWallet as jest.Mock
const mockDisconnect = walletManager.disconnectWallet as jest.Mock
const mockUseAppStore = useAppStore as unknown as jest.Mock

function buildStoreMock(overrides?: object) {
  const setWalletConnected = jest.fn()
  const setWalletDisconnected = jest.fn()
  const setNetworkMismatch = jest.fn()

  return {
    wallet: { isConnected: false, publicKey: null, network: 'testnet' as const },
    setWalletConnected,
    setWalletDisconnected,
    setNetworkMismatch,
    ...overrides,
  }
}

const PUBLIC_KEY = 'GABCDEF1234567890'

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useWallet', () => {
  beforeEach(() => {
    jest.clearAllMocks()
    accountChangeCallback = null
    networkChangeCallback = null
  })

  // -------------------------------------------------------------------------
  // Initial state
  // -------------------------------------------------------------------------

  describe('initial state', () => {
    it('returns isConnecting=false, error=null, networkMismatch=false before any action', () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      const { result } = renderHook(() => useWallet())

      expect(result.current.isConnecting).toBe(false)
      expect(result.current.error).toBeNull()
      expect(result.current.errorKind).toBeNull()
      expect(result.current.networkMismatch).toBe(false)
      expect(result.current.wallet.isConnected).toBe(false)
    })
  })

  // -------------------------------------------------------------------------
  // connect()
  // -------------------------------------------------------------------------

  describe('connect()', () => {
    it('calls walletManager.connectWallet and updates store on success', async () => {
      const store = buildStoreMock()
      mockUseAppStore.mockReturnValue(store)
      mockConnect.mockResolvedValue(PUBLIC_KEY)

      const { result } = renderHook(() => useWallet())

      await act(async () => {
        await result.current.connect()
      })

      expect(mockConnect).toHaveBeenCalledTimes(1)
      expect(store.setWalletConnected).toHaveBeenCalledWith(PUBLIC_KEY, 'testnet')
      expect(result.current.error).toBeNull()
      expect(result.current.isConnecting).toBe(false)
    })

    it('sets isConnecting=true while the call is in flight', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      let resolve: (v: string) => void
      mockConnect.mockReturnValue(new Promise<string>((res) => { resolve = res }))

      const { result } = renderHook(() => useWallet())

      act(() => { result.current.connect() })
      expect(result.current.isConnecting).toBe(true)

      await act(async () => { resolve!(PUBLIC_KEY) })
      expect(result.current.isConnecting).toBe(false)
    })

    it('sets errorKind="user_rejected" on WalletUserRejectedError', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockConnect.mockRejectedValue(new WalletUserRejectedError())

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.connect() })

      expect(result.current.errorKind).toBe('user_rejected')
      expect(result.current.isWalletError).toBe(true)
    })

    it('sets errorKind="network_mismatch" on WalletNetworkMismatchError', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockConnect.mockRejectedValue(new WalletNetworkMismatchError('PUBLIC', 'TESTNET'))

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.connect() })

      expect(result.current.errorKind).toBe('network_mismatch')
      expect(result.current.networkMismatch).toBe(true)
    })

    it('sets errorKind="wallet_not_installed" when error includes "not installed"', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockConnect.mockRejectedValue(new WalletConnectionError('Freighter is not installed'))

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.connect() })

      expect(result.current.errorKind).toBe('wallet_not_installed')
    })

    it('sets errorKind="connection_error" on generic WalletConnectionError', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockConnect.mockRejectedValue(new WalletConnectionError('Something went wrong'))

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.connect() })

      expect(result.current.errorKind).toBe('connection_error')
    })

    it('sets generic error message when a non-Error is thrown', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockConnect.mockRejectedValue('unexpected string')

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.connect() })

      expect(result.current.error).toBe('Failed to connect wallet')
    })
  })

  // -------------------------------------------------------------------------
  // disconnect()
  // -------------------------------------------------------------------------

  describe('disconnect()', () => {
    it('calls walletManager.disconnectWallet and clears store', async () => {
      const store = buildStoreMock({
        wallet: { isConnected: true, publicKey: PUBLIC_KEY, network: 'testnet' },
      })
      mockUseAppStore.mockReturnValue(store)
      mockDisconnect.mockResolvedValue(undefined)

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.disconnect() })

      expect(mockDisconnect).toHaveBeenCalledTimes(1)
      expect(store.setWalletDisconnected).toHaveBeenCalledTimes(1)
      expect(result.current.error).toBeNull()
    })

    it('sets error when walletManager.disconnectWallet throws', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockDisconnect.mockRejectedValue(new Error('Disconnect failed'))

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.disconnect() })

      expect(result.current.error).toBe('Disconnect failed')
      expect(result.current.errorKind).toBe('connection_error')
    })
  })

  // -------------------------------------------------------------------------
  // clearError()
  // -------------------------------------------------------------------------

  describe('clearError()', () => {
    it('resets error and errorKind to null', async () => {
      mockUseAppStore.mockReturnValue(buildStoreMock())
      mockConnect.mockRejectedValue(new WalletConnectionError('Something went wrong'))

      const { result } = renderHook(() => useWallet())
      await act(async () => { await result.current.connect() })
      expect(result.current.error).not.toBeNull()

      act(() => { result.current.clearError() })
      expect(result.current.error).toBeNull()
      expect(result.current.errorKind).toBeNull()
    })
  })

  // -------------------------------------------------------------------------
  // Account-change event listener
  // -------------------------------------------------------------------------

  describe('account-change event', () => {
    it('calls setWalletDisconnected when account changes to null', () => {
      const store = buildStoreMock()
      mockUseAppStore.mockReturnValue(store)

      renderHook(() => useWallet())

      act(() => {
        accountChangeCallback?.(null)
      })

      expect(store.setWalletDisconnected).toHaveBeenCalledTimes(1)
    })

    it('calls setWalletConnected when account changes to a new key', () => {
      const store = buildStoreMock()
      mockUseAppStore.mockReturnValue(store)

      renderHook(() => useWallet())

      act(() => {
        accountChangeCallback?.('GNEWKEY123')
      })

      expect(store.setWalletConnected).toHaveBeenCalledWith('GNEWKEY123', 'testnet')
    })
  })

  // -------------------------------------------------------------------------
  // Network-change event listener
  // -------------------------------------------------------------------------

  describe('network-change event', () => {
    it('sets networkMismatch=true and disconnects on wrong network', () => {
      const store = buildStoreMock()
      mockUseAppStore.mockReturnValue(store)

      const { result } = renderHook(() => useWallet())

      act(() => {
        networkChangeCallback?.('PUBLIC')
      })

      expect(result.current.networkMismatch).toBe(true)
      expect(store.setWalletDisconnected).toHaveBeenCalledTimes(1)
      expect(result.current.errorKind).toBe('network_mismatch')
    })

    it('clears networkMismatch when network changes back to correct network', () => {
      const store = buildStoreMock()
      mockUseAppStore.mockReturnValue(store)

      const { result } = renderHook(() => useWallet())

      // First trigger a mismatch
      act(() => { networkChangeCallback?.('PUBLIC') })
      expect(result.current.networkMismatch).toBe(true)

      // Then switch back — setNetworkMismatch(false) should be called
      act(() => { networkChangeCallback?.('TESTNET') })
      expect(store.setNetworkMismatch).toHaveBeenCalledWith(false)
    })
  })
})
