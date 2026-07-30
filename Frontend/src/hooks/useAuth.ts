/**
 * useAuth — React hook for authentication flows.
 *
 * Handles login, register, logout, token refresh, profile management, and
 * session lifecycle.  Tokens are stored both in the Zustand store (for React
 * reactivity) and in the API client's in-memory cache (for every HTTP request).
 */

'use client'

import { useCallback, useState } from 'react'
import { useAppStore } from '@/stores'
import {
  apiClient,
  ApiError,
  setAccessToken,
  setRefreshToken,
} from '@/lib/api'
import type {
  RegisterInput,
  LoginInput,
  User,
} from '@/types'

export interface UseAuthReturn {
  /** The authenticated user, or null */
  user: User | null
  /** True while any auth request is in flight */
  isLoading: boolean
  /** Human-readable error, or null */
  error: string | null
  /** Whether a user is currently authenticated (has a valid access token) */
  isAuthenticated: boolean

  /** Register a new account */
  register: (data: RegisterInput) => Promise<User | null>
  /** Log in with email and password */
  login: (data: LoginInput) => Promise<User | null>
  /** Log out the current session */
  logout: () => Promise<void>
  /** Refresh the access token using the stored refresh token */
  refreshSession: () => Promise<boolean>
  /** Fetch the user's profile */
  fetchProfile: () => Promise<User | null>
  /** Update the user's profile */
  updateProfile: (data: Partial<Pick<User, 'firstName' | 'lastName' | 'phoneNumber'>>) => Promise<User | null>
  /** Clear any error state */
  clearError: () => void
}

export function useAuth(): UseAuthReturn {
  const {
    user,
    accessToken,
    refreshToken: storedRefreshToken,
    setTokens,
    setUser,
    clearAuth,
  } = useAppStore()

  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const isAuthenticated = !!accessToken

  // -------------------------------------------------------------------------
  // Register
  // -------------------------------------------------------------------------

  const register = useCallback(
    async (data: RegisterInput): Promise<User | null> => {
      setIsLoading(true)
      setError(null)
      try {
        const response = await apiClient.register(data)
        return response.user
      } catch (err) {
        const message =
          err instanceof ApiError
            ? err.message
            : 'Registration failed. Please try again.'
        setError(message)
        return null
      } finally {
        setIsLoading(false)
      }
    },
    []
  )

  // -------------------------------------------------------------------------
  // Login
  // -------------------------------------------------------------------------

  const login = useCallback(
    async (data: LoginInput): Promise<User | null> => {
      setIsLoading(true)
      setError(null)
      try {
        const response = await apiClient.login(data)
        setTokens(response.accessToken, response.refreshToken)
        setUser(response.user)
        return response.user
      } catch (err) {
        const message =
          err instanceof ApiError
            ? err.message
            : 'Login failed. Please check your credentials.'
        setError(message)
        return null
      } finally {
        setIsLoading(false)
      }
    },
    [setTokens, setUser]
  )

  // -------------------------------------------------------------------------
  // Logout
  // -------------------------------------------------------------------------

  const logout = useCallback(async () => {
    setIsLoading(true)
    setError(null)
    try {
      if (storedRefreshToken) {
        await apiClient.logout({ refreshToken: storedRefreshToken })
      }
    } catch {
      // Even if the server call fails, clear local state
    } finally {
      clearAuth()
      setIsLoading(false)
    }
  }, [storedRefreshToken, clearAuth])

  // -------------------------------------------------------------------------
  // Refresh session
  // -------------------------------------------------------------------------

  const refreshSession = useCallback(async (): Promise<boolean> => {
    if (!storedRefreshToken) {
      clearAuth()
      return false
    }

    try {
      const response = await apiClient.refreshToken({
        refreshToken: storedRefreshToken,
      })
      setTokens(response.accessToken, response.refreshToken)
      return true
    } catch {
      clearAuth()
      return false
    }
  }, [storedRefreshToken, setTokens, clearAuth])

  // -------------------------------------------------------------------------
  // Fetch profile
  // -------------------------------------------------------------------------

  const fetchProfile = useCallback(async (): Promise<User | null> => {
    if (!accessToken) return null

    setIsLoading(true)
    setError(null)
    try {
      const profile = await apiClient.getProfile()
      setUser(profile)
      return profile
    } catch (err) {
      // If a 401 occurs, attempt a token refresh and retry once
      if (err instanceof ApiError && err.statusCode === 401) {
        const refreshed = await refreshSession()
        if (refreshed) {
          try {
            const profile = await apiClient.getProfile()
            setUser(profile)
            return profile
          } catch {
            // Fall through to error handling below
          }
        }
      }
      const message =
        err instanceof ApiError
          ? err.message
          : 'Failed to fetch profile.'
      setError(message)
      return null
    } finally {
      setIsLoading(false)
    }
  }, [accessToken, setUser, refreshSession])

  // -------------------------------------------------------------------------
  // Update profile
  // -------------------------------------------------------------------------

  const updateProfile = useCallback(
    async (
      data: Partial<Pick<User, 'firstName' | 'lastName' | 'phoneNumber'>>
    ): Promise<User | null> => {
      setIsLoading(true)
      setError(null)
      try {
        const updated = await apiClient.updateProfile(data)
        setUser(updated)
        return updated
      } catch (err) {
        const message =
          err instanceof ApiError
            ? err.message
            : 'Failed to update profile.'
        setError(message)
        return null
      } finally {
        setIsLoading(false)
      }
    },
    [setUser]
  )

  // -------------------------------------------------------------------------
  // Clear error
  // -------------------------------------------------------------------------

  const clearError = useCallback(() => setError(null), [])

  return {
    user,
    isLoading,
    error,
    isAuthenticated,
    register,
    login,
    logout,
    refreshSession,
    fetchProfile,
    updateProfile,
    clearError,
  }
}
