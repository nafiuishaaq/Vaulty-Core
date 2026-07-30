import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import {
  WalletState,
  Vault,
  Streak,
  DisciplineScore,
  FundingOrder,
  WithdrawalOrder,
  PaymentStatus,
  BankAccount,
  FeatureFlags,
  User,
  TransactionNotification,
} from '@/types'
import { setAccessToken, setRefreshToken } from '@/lib/api'

// ---------------------------------------------------------------------------
// Auth slice
// ---------------------------------------------------------------------------

interface AuthState {
  accessToken: string | null
  refreshToken: string | null
  user: User | null
  setTokens: (accessToken: string | null, refreshToken: string | null) => void
  setUser: (user: User | null) => void
  clearAuth: () => void
}

// ---------------------------------------------------------------------------
// Full AppState
// ---------------------------------------------------------------------------

interface AppState extends AuthState {
  wallet: WalletState
  /** True when the wallet's active network doesn't match the app's configured network. */
  networkMismatch: boolean
  vaults: Vault[]
  streak: Streak | null
  disciplineScore: DisciplineScore | null
  bankAccounts: BankAccount[]
  selectedBankAccountId: string | null
  fundingOrders: FundingOrder[]
  withdrawalOrders: WithdrawalOrder[]
  regulatedFeatures: FeatureFlags
  transactionNotifications: TransactionNotification[]

  // Transaction notification actions
  addTransactionNotification: (notification: Omit<TransactionNotification, 'id' | 'createdAt' | 'updatedAt'>) => string
  updateTransactionNotification: (id: string, updates: Partial<Omit<TransactionNotification, 'id' | 'createdAt'>>) => void
  dismissTransactionNotification: (id: string) => void
  removeTransactionNotification: (id: string) => void
  findNotificationByIdempotencyKey: (idempotencyKey: string) => TransactionNotification | undefined

  setWalletConnected: (publicKey: string, network: 'testnet' | 'mainnet') => void
  setWalletDisconnected: () => void
  setNetworkMismatch: (mismatch: boolean) => void

  setVaults: (vaults: Vault[]) => void
  addVault: (vault: Vault) => void
  updateVault: (id: string, updates: Partial<Vault>) => void

  setStreak: (streak: Streak) => void
  setDisciplineScore: (score: DisciplineScore) => void

  setBankAccounts: (accounts: BankAccount[]) => void
  setSelectedBankAccount: (id: string | null) => void

  addFundingOrder: (order: FundingOrder) => void
  updateFundingOrderStatus: (id: string, status: PaymentStatus, failureReason?: string) => void
  removeFundingOrder: (id: string) => void

  addWithdrawalOrder: (order: WithdrawalOrder) => void
  updateWithdrawalOrderStatus: (id: string, status: PaymentStatus, failureReason?: string) => void
  removeWithdrawalOrder: (id: string) => void

  setRegulatedFeatures: (flags: FeatureFlags) => void
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      // -------------------------------------------------------------------
      // Auth initial state
      // -------------------------------------------------------------------
      accessToken: null,
      refreshToken: null,
      user: null,

      // -------------------------------------------------------------------
      // Wallet initial state
      // -------------------------------------------------------------------
      wallet: {
        isConnected: false,
        publicKey: null,
        network: 'testnet',
      },
      // Never persisted — re-evaluated on every connection attempt
      networkMismatch: false,
      vaults: [],
      streak: null,
      disciplineScore: null,
      bankAccounts: [],
      selectedBankAccountId: null,
      fundingOrders: [],
      withdrawalOrders: [],
      // Default all regulated features to disabled; updated by useFeatureFlags on mount.
      regulatedFeatures: {
        lending: false,
        borrowing: false,
        investments: false,
      },

      // -------------------------------------------------------------------
      // Auth actions
      // -------------------------------------------------------------------
      setTokens: (accessToken, refreshToken) => {
        set({ accessToken, refreshToken })
        // Keep the API client's in-memory token cache in sync
        setAccessToken(accessToken)
        setRefreshToken(refreshToken)
      },

      setUser: (user) => set({ user }),

      clearAuth: () => {
        set({ accessToken: null, refreshToken: null, user: null })
        setAccessToken(null)
        setRefreshToken(null)
      },

      // -------------------------------------------------------------------
      // Wallet actions
      // -------------------------------------------------------------------
      setWalletConnected: (publicKey, network) =>
        set({
          wallet: { isConnected: true, publicKey, network },
          networkMismatch: false,
        }),

      setWalletDisconnected: () =>
        set({
          // Clear connection state — never persist wallet credentials
          wallet: { isConnected: false, publicKey: null, network: 'testnet' },
          networkMismatch: false,
        }),

      setNetworkMismatch: (mismatch) => set({ networkMismatch: mismatch }),

      setVaults: (vaults) => set({ vaults }),

      addVault: (vault) =>
        set((state) => ({ vaults: [...state.vaults, vault] })),

      updateVault: (id, updates) =>
        set((state) => ({
          vaults: state.vaults.map((vault) =>
            vault.id === id ? { ...vault, ...updates } : vault
          ),
        })),

      setStreak: (streak) => set({ streak }),

      setDisciplineScore: (score) => set({ disciplineScore: score }),

      setBankAccounts: (accounts) => set({ bankAccounts: accounts }),

      setSelectedBankAccount: (id) => set({ selectedBankAccountId: id }),

      addFundingOrder: (order) =>
        set((state) => ({
          fundingOrders: [order, ...state.fundingOrders],
        })),

      updateFundingOrderStatus: (id, status, failureReason) =>
        set((state) => ({
          fundingOrders: state.fundingOrders.map((order) =>
            order.id === id
              ? {
                  ...order,
                  status,
                  failureReason: failureReason ?? order.failureReason,
                  updatedAt: new Date().toISOString(),
                }
              : order
          ),
        })),

      removeFundingOrder: (id) =>
        set((state) => ({
          fundingOrders: state.fundingOrders.filter((order) => order.id !== id),
        })),

      addWithdrawalOrder: (order) =>
        set((state) => ({
          withdrawalOrders: [order, ...state.withdrawalOrders],
        })),

      updateWithdrawalOrderStatus: (id, status, failureReason) =>
        set((state) => ({
          withdrawalOrders: state.withdrawalOrders.map((order) =>
            order.id === id
              ? {
                  ...order,
                  status,
                  failureReason: failureReason ?? order.failureReason,
                  updatedAt: new Date().toISOString(),
                }
              : order
          ),
        })),

      removeWithdrawalOrder: (id) =>
        set((state) => ({
          withdrawalOrders: state.withdrawalOrders.filter(
            (order) => order.id !== id
          ),
        })),

      setRegulatedFeatures: (flags) => set({ regulatedFeatures: flags }),
    }),
    {
      name: 'vaulty-payments',
      // Wallet state, networkMismatch, and auth tokens are intentionally
      // excluded from persistence — private keys must never be stored,
      // connection/auth state must be re-validated on every page load.
      partialize: (state) => ({
        fundingOrders: state.fundingOrders,
        withdrawalOrders: state.withdrawalOrders,
        bankAccounts: state.bankAccounts,
        selectedBankAccountId: state.selectedBankAccountId,
      }),
    }
  )
)