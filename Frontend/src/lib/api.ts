/**
 * api.ts — Typed HTTP client for the Vaulty backend API.
 *
 * Design:
 *  - Single configurable base URL from NEXT_PUBLIC_API_URL (defaults to
 *    http://localhost:3000/api/v1 — the actual backend port).
 *  - Authentication token is managed externally (e.g. via Zustand or React
 *    state) and injected via `setAccessToken()` / `setRefreshToken()`.
 *  - Idempotency keys are sent as the X-Idempotency-Key header on mutating
 *    requests that accept one.
 *  - All methods return typed responses. Errors are thrown as ApiError which
 *    carries the backend error code(s) and the request ID if the backend
 *    supplied one.
 */

import type {
  ApiResponse,
  RegisterInput,
  RegisterResponse,
  LoginInput,
  LoginResponse,
  RefreshTokenInput,
  RefreshTokenResponse,
  LogoutInput,
  LogoutResponse,
  ForgotPasswordInput,
  ResetPasswordInput,
  VerifyEmailInput,
  UpdateProfileInput,
  User,
  Vault,
  CreateVaultInput,
  DepositToVaultInput,
  WithdrawFromVaultInput,
  LockVaultInput,
  VaultTransaction,
  FeatureFlags,
  PaymentInstructions,
  FeeInfo,
  ConversionInfo,
  PaymentStatus,
} from '@/types'

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const API_BASE_URL =
  process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3000/api/v1'

// ---------------------------------------------------------------------------
// Token management (in-memory; persist to store via setAccessToken callback)
// ---------------------------------------------------------------------------

let _accessToken: string | null = null
let _refreshToken: string | null = null

export function setAccessToken(token: string | null): void {
  _accessToken = token
}

export function setRefreshToken(token: string | null): void {
  _refreshToken = token
}

export function getAccessToken(): string | null {
  return _accessToken
}

export function getRefreshToken(): string | null {
  return _refreshToken
}

// ---------------------------------------------------------------------------
// Feature-flag env fallback
// ---------------------------------------------------------------------------

function envFallbackFlags(): FeatureFlags {
  return {
    lending: process.env.NEXT_PUBLIC_ENABLE_LENDING === 'true',
    borrowing: process.env.NEXT_PUBLIC_ENABLE_BORROWING === 'true',
    investments: process.env.NEXT_PUBLIC_ENABLE_INVESTMENTS === 'true',
  }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

export class ApiError extends Error {
  public readonly statusCode: number
  /** Validation-error details from the backend, if any. */
  public readonly errors?: { path: (string | number)[]; message: string; code: string }[]
  /** Raw response body. Callers must NOT render this directly in the UI. */
  public readonly body?: unknown

  constructor(
    message: string,
    statusCode: number,
    errors?: { path: (string | number)[]; message: string; code: string }[],
    body?: unknown
  ) {
    super(message)
    this.name = 'ApiError'
    this.statusCode = statusCode
    this.errors = errors
    this.body = body
  }
}

export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError
}

// ---------------------------------------------------------------------------
// Idempotency key helper
// ---------------------------------------------------------------------------

export function generateIdempotencyKey(): string {
  return `${Date.now()}-${crypto.randomUUID()}`
}

// ---------------------------------------------------------------------------
// HTTP client internals
// ---------------------------------------------------------------------------

class HttpClient {
  private baseUrl: string

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {},
    idempotencyKey?: string
  ): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string> | undefined),
    }

    // Attach auth header if a token is available
    if (_accessToken) {
      headers['Authorization'] = `Bearer ${_accessToken}`
    }

    // Attach idempotency key if provided
    if (idempotencyKey) {
      headers['X-Idempotency-Key'] = idempotencyKey
    }

    const response = await fetch(url, { ...options, headers })

    // Try to parse the JSON body
    const body: ApiResponse<unknown> | null = await response
      .json()
      .catch(() => null)

    if (!response.ok) {
      const backendMessage =
        body?.message || `API request failed: ${response.statusText}`
      throw new ApiError(
        backendMessage,
        response.status,
        body?.errors,
        body
      )
    }

    if (body && body.success === false) {
      throw new ApiError(
        body.message || 'Unknown backend error',
        response.status,
        body.errors,
        body
      )
    }

    // Unwrap and return the data payload
    if (body && body.success === true && body.data !== undefined) {
      return body.data as T
    }

    // Fallback: return the whole body (for list endpoints or legacy responses)
    return (body ?? {}) as unknown as T
  }

  // -----------------------------------------------------------------------
  // Auth
  // -----------------------------------------------------------------------

  async register(data: RegisterInput): Promise<RegisterResponse> {
    return this.request<RegisterResponse>('/auth/register', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async login(data: LoginInput): Promise<LoginResponse> {
    return this.request<LoginResponse>('/auth/login', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async refreshToken(data: RefreshTokenInput): Promise<RefreshTokenResponse> {
    return this.request<RefreshTokenResponse>('/auth/refresh-token', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async logout(data: LogoutInput): Promise<LogoutResponse> {
    return this.request<LogoutResponse>('/auth/logout', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async logoutAll(): Promise<LogoutResponse> {
    return this.request<LogoutResponse>('/auth/logout-all', {
      method: 'POST',
    })
  }

  async forgotPassword(data: ForgotPasswordInput): Promise<{ message: string }> {
    return this.request<{ message: string }>('/auth/forgot-password', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async resetPassword(data: ResetPasswordInput): Promise<{ message: string }> {
    return this.request<{ message: string }>('/auth/reset-password', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async verifyEmail(data: VerifyEmailInput): Promise<{ message: string }> {
    return this.request<{ message: string }>('/auth/verify-email', {
      method: 'POST',
      body: JSON.stringify(data),
    })
  }

  async getProfile(): Promise<User> {
    return this.request<User>('/auth/profile')
  }

  async updateProfile(data: UpdateProfileInput): Promise<User> {
    return this.request<User>('/auth/profile', {
      method: 'PUT',
      body: JSON.stringify(data),
    })
  }

  // -----------------------------------------------------------------------
  // Vaults
  // -----------------------------------------------------------------------

  async getVaults(params?: {
    page?: number
    limit?: number
    status?: string
    type?: string
  }): Promise<{ vaults: Vault[]; pagination: { total: number; page: number; limit: number; pages: number } }> {
    const query = new URLSearchParams()
    if (params?.page) query.set('page', String(params.page))
    if (params?.limit) query.set('limit', String(params.limit))
    if (params?.status) query.set('status', params.status)
    if (params?.type) query.set('type', params.type)
    const qs = query.toString()
    return this.request(`/vaults${qs ? `?${qs}` : ''}`)
  }

  async getVault(vaultId: string): Promise<{ vault: Vault }> {
    return this.request<{ vault: Vault }>(`/vaults/${vaultId}`)
  }

  async createVault(data: CreateVaultInput): Promise<{ vault: Vault }> {
    return this.request<{ vault: Vault }>(
      '/vaults',
      {
        method: 'POST',
        body: JSON.stringify(data),
      },
      data.idempotencyKey
    )
  }

  async depositToVault(
    vaultId: string,
    data: DepositToVaultInput
  ): Promise<{ transaction: VaultTransaction }> {
    return this.request<{ transaction: VaultTransaction }>(
      `/vaults/${vaultId}/deposit`,
      {
        method: 'POST',
        body: JSON.stringify({ amount: data.amount, description: data.description, idempotencyKey: data.idempotencyKey }),
      },
      data.idempotencyKey
    )
  }

  async withdrawFromVault(
    vaultId: string,
    data: WithdrawFromVaultInput
  ): Promise<{ transaction: VaultTransaction }> {
    return this.request<{ transaction: VaultTransaction }>(
      `/vaults/${vaultId}/withdraw`,
      {
        method: 'POST',
        body: JSON.stringify({ amount: data.amount, description: data.description, idempotencyKey: data.idempotencyKey }),
      },
      data.idempotencyKey
    )
  }

  async lockVault(
    vaultId: string,
    data: LockVaultInput
  ): Promise<{ vault: Vault }> {
    return this.request<{ vault: Vault }>(
      `/vaults/${vaultId}/lock`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    )
  }

  async unlockVault(vaultId: string): Promise<{ vault: Vault }> {
    return this.request<{ vault: Vault }>(`/vaults/${vaultId}/unlock`, {
      method: 'POST',
    })
  }

  async closeVault(vaultId: string): Promise<{ vault: Vault }> {
    return this.request<{ vault: Vault }>(`/vaults/${vaultId}/close`, {
      method: 'POST',
    })
  }

  async getVaultHistory(
    vaultId: string,
    params?: {
      page?: number
      limit?: number
      status?: string
      type?: string
    }
  ): Promise<{
    transactions: VaultTransaction[]
    pagination: { total: number; page: number; limit: number; pages: number }
  }> {
    const query = new URLSearchParams()
    if (params?.page) query.set('page', String(params.page))
    if (params?.limit) query.set('limit', String(params.limit))
    if (params?.status) query.set('status', params.status)
    if (params?.type) query.set('type', params.type)
    const qs = query.toString()
    return this.request(`/vaults/${vaultId}/history${qs ? `?${qs}` : ''}`)
  }

  // -----------------------------------------------------------------------
  // Transactions
  // -----------------------------------------------------------------------

  async submitTransaction(xdr: string): Promise<{ transaction: VaultTransaction }> {
    return this.request<{ transaction: VaultTransaction }>(
      '/transactions/submit',
      {
        method: 'POST',
        body: JSON.stringify({ signedXdr: xdr }),
      }
    )
  }

  async getTransactionStatus(
    apiTransactionId: string
  ): Promise<{ transaction: VaultTransaction }> {
    return this.request<{ transaction: VaultTransaction }>(
      `/transactions/${apiTransactionId}`
    )
  }

  // -----------------------------------------------------------------------
  // Feature flags
  // -----------------------------------------------------------------------

  async getFeatureFlags(): Promise<FeatureFlags> {
    // The backend does not currently expose a /config/features endpoint under
    // the API v1 prefix.  Keep the request attempt but fall back to env vars
    // so local dev and CI don't require a running backend.
    try {
      return await this.request<FeatureFlags>('/config/features')
    } catch {
      return envFallbackFlags()
    }
  }

  // -----------------------------------------------------------------------
  // Legacy deposit/withdrawal methods (fiat ramp — mock for now)
  // These are kept so existing components (FundingFlow, WithdrawalFlow)
  // continue to compile. They will be replaced once the fiat-ramp endpoints
  // are added to the backend.
  // -----------------------------------------------------------------------

  async initiateDeposit(
    amount: number,
    bankAccountId: string,
    idempotencyKey: string
  ): Promise<{
    depositId: string
    status: PaymentStatus
    paymentInstructions: PaymentInstructions
    fees: FeeInfo
    conversion: ConversionInfo
  }> {
    // The backend does not expose these endpoints yet, so we throw a clear error.
    throw new ApiError(
      'Fiat deposit endpoints are not available in the current backend. ' +
        'Use vault deposit for on-chain deposits.',
      501
    )
  }

  async initiateWithdrawal(
    amount: number,
    bankAccountId: string,
    idempotencyKey: string
  ): Promise<{
    withdrawalId: string
    status: PaymentStatus
    fees: FeeInfo
    conversion: ConversionInfo
  }> {
    throw new ApiError(
      'Fiat withdrawal endpoints are not available in the current backend. ' +
        'Use vault withdrawal for on-chain withdrawals.',
      501
    )
  }

  async getDepositStatus(depositId: string): Promise<{
    status: PaymentStatus
    amount: number
    fees: FeeInfo
    conversion: ConversionInfo
    failureReason?: string
    completedAt?: string
  }> {
    throw new ApiError('Deposit status endpoints are not available in the current backend.', 501)
  }

  async getWithdrawalStatus(withdrawalId: string): Promise<{
    status: PaymentStatus
    amount: number
    fees: FeeInfo
    conversion: ConversionInfo
    failureReason?: string
    completedAt?: string
  }> {
    throw new ApiError('Withdrawal status endpoints are not available in the current backend.', 501)
  }

  async retryDeposit(depositId: string): Promise<{
    status: PaymentStatus
    paymentInstructions: PaymentInstructions
  }> {
    throw new ApiError('Retry deposit endpoints are not available in the current backend.', 501)
  }

  async retryWithdrawal(withdrawalId: string): Promise<{
    status: PaymentStatus
  }> {
    throw new ApiError('Retry withdrawal endpoints are not available in the current backend.', 501)
  }
}

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

export const apiClient = new HttpClient(API_BASE_URL)
