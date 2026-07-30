// Shared TypeScript types for the Vaulty frontend

// ---------------------------------------------------------------------------
// API response wrappers
// ---------------------------------------------------------------------------

/** Standard backend API response envelope. */
export interface ApiResponse<T> {
  success: boolean
  data?: T
  message?: string
  errors?: ApiValidationError[]
  /** Present when the backend is in development mode. */
  stack?: string
}

export interface ApiValidationError {
  path: (string | number)[]
  message: string
  code: string
}

export interface PaginatedResponse<T> {
  data: T[]
  pagination: {
    total: number
    page: number
    limit: number
    pages: number
  }
}

// ---------------------------------------------------------------------------
// Auth types
// ---------------------------------------------------------------------------

export interface User {
  id: string
  email: string
  firstName: string
  lastName: string
  phoneNumber: string | null
  role: string
  isEmailVerified: boolean
  emailVerifiedAt: string | null
  lastLoginAt: string | null
  tokenVersion: number
  createdAt: string
  updatedAt: string
}

export interface RegisterInput {
  email: string
  password: string
  firstName: string
  lastName: string
  phoneNumber?: string
}

export interface RegisterResponse {
  user: User
}

export interface LoginInput {
  email: string
  password: string
  device?: string
  ipAddress?: string
  userAgent?: string
}

export interface LoginResponse {
  user: User
  accessToken: string
  refreshToken: string
}

export interface RefreshTokenInput {
  refreshToken: string
}

export interface RefreshTokenResponse {
  accessToken: string
  refreshToken: string
}

export interface LogoutInput {
  refreshToken: string
}

export interface LogoutResponse {
  message: string
  revoked: number
}

export interface ForgotPasswordInput {
  email: string
}

export interface ResetPasswordInput {
  token: string
  password: string
}

export interface VerifyEmailInput {
  token: string
}

export interface UpdateProfileInput {
  firstName?: string
  lastName?: string
  phoneNumber?: string
}

// ---------------------------------------------------------------------------
// Vault types
// ---------------------------------------------------------------------------

export interface Vault {
  id: string
  userId: string
  name: string
  description: string | null
  targetAmount: string
  currentAmount: string
  status: VaultStatus
  type: VaultType
  lockPeriod: number
  lockedAt: string | null
  unlocksAt: string | null
  assetCode: string
  assetIssuer: string | null
  contractAddress: string | null
  onChainVaultId: string | null
  goalDescription: string | null
  createdAt: string
  updatedAt: string
}

export type VaultStatus = 'ACTIVE' | 'LOCKED' | 'CLOSED'
export type VaultType = 'SAVINGS' | 'GOAL' | 'FIXED_DEPOSIT'

export interface CreateVaultInput {
  name: string
  description?: string
  targetAmount: number
  lockPeriod: number
  assetCode?: string
  assetIssuer?: string
  contractAddress?: string
  onChainVaultId?: string
  type?: VaultType
  goalDescription?: string
  idempotencyKey?: string
}

export interface DepositToVaultInput {
  amount: number
  description?: string
  idempotencyKey: string
}

export interface WithdrawFromVaultInput {
  amount: number
  description?: string
  idempotencyKey: string
}

export interface LockVaultInput {
  lockPeriod: number
}

// ---------------------------------------------------------------------------
// Transaction types
// ---------------------------------------------------------------------------

export interface VaultTransaction {
  id: string
  vaultId: string
  userId: string
  type: VaultTransactionType
  status: VaultTransactionStatus
  amount: string
  description: string | null
  reference: string
  stellarTransactionHash: string | null
  failureCode: string | null
  failureReason: string | null
  idempotencyKey: string | null
  onChainVaultId: string | null
  confirmedAt: string | null
  createdAt: string
  updatedAt: string
}

export type VaultTransactionType = 'DEPOSIT' | 'WITHDRAWAL'
export type VaultTransactionStatus = 'PENDING' | 'CONFIRMED' | 'FAILED' | 'CANCELLED'

// ---------------------------------------------------------------------------
// Transaction notification types (toast system)
// ---------------------------------------------------------------------------

export interface TransactionNotification {
  id: string
  type: 'vault' | 'funding' | 'lending' | 'borrowing'
  action: string
  status: 'pending' | 'success' | 'error' | 'dismissed'
  message: string
  reference?: string
  idempotencyKey?: string
  createdAt: string
  updatedAt: string
}

export type TransactionStatusType = 'pending' | 'success' | 'error' | 'dismissed'

// ---------------------------------------------------------------------------
// Legacy / payment flow types (for fiat ramp)
// ---------------------------------------------------------------------------

export type PaymentStatus =
  | 'pending'
  | 'awaiting_bank_transfer'
  | 'processing'
  | 'completed'
  | 'failed'
  | 'expired'

export type PaymentFlow = 'deposit' | 'withdrawal'

export interface BankAccount {
  id: string
  bankName: string
  accountNumber: string
  accountName: string
  bankCode: string
}

export interface FeeInfo {
  platformFee: number
  networkFee: number
  totalFee: number
  currency: string
}

export interface ConversionInfo {
  inputAmount: number
  inputCurrency: string
  outputAmount: number
  outputCurrency: string
  exchangeRate: number
}

export interface PaymentInstructions {
  bankName: string
  accountNumber: string
  accountName: string
  reference: string
  amount: number
  currency: string
  expiresAt: string
}

export interface FundingOrder {
  id: string
  flow: 'deposit'
  vaultId: string
  amount: number
  bankAccountId: string
  status: PaymentStatus
  paymentInstructions: PaymentInstructions | null
  fees: FeeInfo | null
  conversion: ConversionInfo | null
  failureReason: string | null
  idempotencyKey: string
  createdAt: string
  updatedAt: string
}

export interface WithdrawalOrder {
  id: string
  flow: 'withdrawal'
  vaultId: string
  amount: number
  bankAccountId: string
  status: PaymentStatus
  fees: FeeInfo | null
  conversion: ConversionInfo | null
  failureReason: string | null
  idempotencyKey: string
  createdAt: string
  updatedAt: string
}

export type PaymentOrder = FundingOrder | WithdrawalOrder

// ---------------------------------------------------------------------------
// Regulated feature availability
// ---------------------------------------------------------------------------

export interface FeatureFlags {
  lending: boolean
  borrowing: boolean
  investments: boolean
}

// ---------------------------------------------------------------------------
// Obsolete — kept for backward compatibility; consider migrating to VaultTransaction
// ---------------------------------------------------------------------------

/** @deprecated Use Vault type from backend response instead. */
export interface Deposit {
  id: string
  vaultId: string
  amount: number
  timestamp: Date
  transactionHash: string
}

/** @deprecated Use VaultTransaction type instead. */
export interface Withdrawal {
  id: string
  vaultId: string
  amount: number
  timestamp: Date
  transactionHash: string
}

// ---------------------------------------------------------------------------
// Streak & achievements
// ---------------------------------------------------------------------------

export interface Streak {
  currentStreak: number
  longestStreak: number
  freezesRemaining: number
  lastDepositDate: Date | null
  calendar: StreakDay[]
}

export interface StreakDay {
  date: Date
  deposited: boolean
  amount?: number
}

export interface DisciplineScore {
  score: number // 0-100
  factors: {
    consistency: number
    streakLength: number
    goalCompletion: number
    repaymentHistory: number
    investmentActivity: number
  }
}

export interface Achievement {
  id: string
  title: string
  description: string
  unlockedAt: Date | null
  icon: string
}

// ---------------------------------------------------------------------------
// Wallet state
// ---------------------------------------------------------------------------

export interface WalletState {
  isConnected: boolean
  /** Stellar public key (G... address). Private keys never enter app state. */
  publicKey: string | null
  network: 'testnet' | 'mainnet'
}

// ---------------------------------------------------------------------------
// Lending & borrowing
// ---------------------------------------------------------------------------

export interface Loan {
  id: string
  borrower: string
  amount: number
  collateralVaultId: string
  interestRate: number
  maturityDate: Date
  status: 'active' | 'repaid' | 'defaulted'
}

export interface Investment {
  id: string
  type: 'conservative' | 'balanced' | 'growth'
  amount: number
  expectedReturn: number
  currentValue: number
}