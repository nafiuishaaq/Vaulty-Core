import { useCallback, useState } from 'react'
import { Vault, FundingOrder, WithdrawalOrder } from '@/types'
import { useAppStore } from '@/stores'
import { apiClient, ApiError, generateIdempotencyKey } from '@/lib/api'

export function useVault() {
  const vaults = useAppStore((s) => s.vaults)
  const addVault = useAppStore((s) => s.addVault)
  const updateVault = useAppStore((s) => s.updateVault)
  const setVaults = useAppStore((s) => s.setVaults)
  const addFundingOrder = useAppStore((s) => s.addFundingOrder)
  const addWithdrawalOrder = useAppStore((s) => s.addWithdrawalOrder)
  const addTransactionNotification = useAppStore((s) => s.addTransactionNotification)
  const updateTransactionNotification = useAppStore((s) => s.updateTransactionNotification)

  const [isProcessing, setIsProcessing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // -------------------------------------------------------------------------
  // Vault CRUD — backed by the real API
  // -------------------------------------------------------------------------

  const createVault = useCallback(
    async (
      vaultData: Omit<
        Vault,
        'id' | 'userId' | 'currentAmount' | 'status' | 'createdAt' | 'updatedAt'
      >
    ) => {
      setIsProcessing(true)
      setError(null)
      const idempotencyKey = generateIdempotencyKey()
      // Add pending notification
      const notificationId = addTransactionNotification({
        type: 'vault',
        action: 'Create vault',
        status: 'pending',
        message: `Creating your vault "${vaultData.name}"...`,
        idempotencyKey,
      })
      
      try {
        const { vault } = await apiClient.createVault({
          ...vaultData,
          targetAmount: Number(vaultData.targetAmount),
          idempotencyKey,
        })
        addVault(vault)
        // Update to success
        updateTransactionNotification(notificationId, {
          status: 'success',
          message: `Vault "${vault.name}" created successfully!`,
          reference: vault.id,
        })
        return vault
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to create vault'
        setError(message)
        // Update to error
        updateTransactionNotification(notificationId, {
          status: 'error',
          message: message,
        })
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [addVault, addTransactionNotification, updateTransactionNotification]
  )

  const fetchVaults = useCallback(
    async (params?: { page?: number; limit?: number; status?: string }) => {
      setIsProcessing(true)
      setError(null)
      try {
        const result = await apiClient.getVaults(params)
        setVaults(result.vaults)
        return result
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to fetch vaults'
        setError(message)
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [setVaults]
  )

  const fetchVault = useCallback(async (vaultId: string) => {
    setIsProcessing(true)
    setError(null)
    try {
      const { vault } = await apiClient.getVault(vaultId)
      return vault
    } catch (err) {
      const message =
        err instanceof ApiError ? err.message : 'Failed to fetch vault'
      setError(message)
      throw err
    } finally {
      setIsProcessing(false)
    }
  }, [])

  // -------------------------------------------------------------------------
  // Deposit / Withdraw — backed by the real vault API
  // -------------------------------------------------------------------------

  const depositToVault = useCallback(
    async (vaultId: string, amount: number) => {
      setIsProcessing(true)
      setError(null)
      const idempotencyKey = generateIdempotencyKey()
      // Add pending notification
      const notificationId = addTransactionNotification({
        type: 'vault',
        action: 'Deposit to vault',
        status: 'pending',
        message: `Processing deposit of $${amount.toFixed(2)} to your vault...`,
        idempotencyKey,
      })
      
      try {
        const { transaction } = await apiClient.depositToVault(vaultId, {
          amount,
          idempotencyKey,
        })

        // Reload vault data from backend to get the updated balance
        await fetchVaults()
        
        // Update to success
        updateTransactionNotification(notificationId, {
          status: 'success',
          message: `Successfully deposited $${amount.toFixed(2)} to your vault!`,
          reference: transaction.id,
        })

        return transaction
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to deposit'
        setError(message)
        // Update to error
        updateTransactionNotification(notificationId, {
          status: 'error',
          message: message,
        })
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [fetchVaults, addTransactionNotification, updateTransactionNotification]
  )

  const withdrawFromVault = useCallback(
    async (vaultId: string, amount: number) => {
      setIsProcessing(true)
      setError(null)
      const idempotencyKey = generateIdempotencyKey()
      // Add pending notification
      const notificationId = addTransactionNotification({
        type: 'vault',
        action: 'Withdraw from vault',
        status: 'pending',
        message: `Processing withdrawal of $${amount.toFixed(2)} from your vault...`,
        idempotencyKey,
      })
      
      try {
        const { transaction } = await apiClient.withdrawFromVault(vaultId, {
          amount,
          idempotencyKey,
        })

        // Reload vault data from backend
        await fetchVaults()
        
        // Update to success
        updateTransactionNotification(notificationId, {
          status: 'success',
          message: `Successfully withdrew $${amount.toFixed(2)} from your vault!`,
          reference: transaction.id,
        })

        return transaction
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to withdraw'
        setError(message)
        // Update to error
        updateTransactionNotification(notificationId, {
          status: 'error',
          message: message,
        })
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [fetchVaults, addTransactionNotification, updateTransactionNotification]
  )

  // -------------------------------------------------------------------------
  // Lock / Unlock / Close
  // -------------------------------------------------------------------------

  const lockVault = useCallback(
    async (vaultId: string, lockPeriod: number) => {
      setIsProcessing(true)
      setError(null)
      const idempotencyKey = generateIdempotencyKey()
      // Add pending notification
      const notificationId = addTransactionNotification({
        type: 'vault',
        action: 'Lock vault',
        status: 'pending',
        message: 'Locking your vault...',
        idempotencyKey,
      })
      
      try {
        const { vault } = await apiClient.lockVault(vaultId, { lockPeriod })
        updateVault(vaultId, vault)
        // Update to success
        updateTransactionNotification(notificationId, {
          status: 'success',
          message: `Vault locked successfully for ${lockPeriod} days!`,
          reference: vault.id,
        })
        return vault
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to lock vault'
        setError(message)
        // Update to error
        updateTransactionNotification(notificationId, {
          status: 'error',
          message: message,
        })
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [updateVault, addTransactionNotification, updateTransactionNotification]
  )

  const unlockVault = useCallback(
    async (vaultId: string) => {
      setIsProcessing(true)
      setError(null)
      const idempotencyKey = generateIdempotencyKey()
      // Add pending notification
      const notificationId = addTransactionNotification({
        type: 'vault',
        action: 'Unlock vault',
        status: 'pending',
        message: 'Unlocking your vault...',
        idempotencyKey,
      })
      
      try {
        const { vault } = await apiClient.unlockVault(vaultId)
        updateVault(vaultId, vault)
        // Update to success
        updateTransactionNotification(notificationId, {
          status: 'success',
          message: 'Vault unlocked successfully!',
          reference: vault.id,
        })
        return vault
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to unlock vault'
        setError(message)
        // Update to error
        updateTransactionNotification(notificationId, {
          status: 'error',
          message: message,
        })
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [updateVault, addTransactionNotification, updateTransactionNotification]
  )

  const closeVault = useCallback(
    async (vaultId: string) => {
      setIsProcessing(true)
      setError(null)
      const idempotencyKey = generateIdempotencyKey()
      // Add pending notification
      const notificationId = addTransactionNotification({
        type: 'vault',
        action: 'Close vault',
        status: 'pending',
        message: 'Closing your vault...',
        idempotencyKey,
      })
      
      try {
        const { vault } = await apiClient.closeVault(vaultId)
        // Reload vaults from backend to get the updated list
        await fetchVaults()
        // Update to success
        updateTransactionNotification(notificationId, {
          status: 'success',
          message: 'Vault closed successfully!',
          reference: vault.id,
        })
        return vault
      } catch (err) {
        const message =
          err instanceof ApiError ? err.message : 'Failed to close vault'
        setError(message)
        // Update to error
        updateTransactionNotification(notificationId, {
          status: 'error',
          message: message,
        })
        throw err
      } finally {
        setIsProcessing(false)
      }
    },
    [fetchVaults, addTransactionNotification, updateTransactionNotification]
  )

  // -------------------------------------------------------------------------
  // Legacy fiat-ramp methods (FundingFlow / WithdrawalFlow compatibility)
  // These create local PaymentOrder records. When the backend adds fiat-ramp
  // endpoints, replace these with real API calls.
  // -------------------------------------------------------------------------

  const initiateFunding = useCallback(
    async (
      vaultId: string,
      amount: number,
      bankAccountId: string
    ): Promise<FundingOrder | null> => {
      setIsProcessing(true)
      setError(null)
      try {
        const idempotencyKey = generateIdempotencyKey()
        // Attempt the real API call — will throw 501 until fiat endpoints exist
        const result = await apiClient.initiateDeposit(
          amount,
          bankAccountId,
          idempotencyKey
        )
        const order: FundingOrder = {
          id: result.depositId,
          flow: 'deposit',
          vaultId,
          amount,
          bankAccountId,
          status: result.status,
          paymentInstructions: result.paymentInstructions,
          fees: result.fees,
          conversion: result.conversion,
          failureReason: null,
          idempotencyKey,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        }
        addFundingOrder(order)
        return order
      } catch (err) {
        if (err instanceof ApiError && err.statusCode === 501) {
          // Fiat endpoint not available — create a local order with mock data
          // so the UI flow can be demonstrated without a backend.
          const order: FundingOrder = {
            id: `local-${idempotencyKey}`,
            flow: 'deposit',
            vaultId,
            amount,
            bankAccountId,
            status: 'awaiting_bank_transfer',
            paymentInstructions: null,
            fees: null,
            conversion: null,
            failureReason: null,
            idempotencyKey,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
          }
          addFundingOrder(order)
          return order
        }
        const message =
          err instanceof ApiError ? err.message : 'Failed to initiate funding'
        setError(message)
        return null
      } finally {
        setIsProcessing(false)
      }
    },
    [addFundingOrder]
  )

  const initiateWithdrawal = useCallback(
    async (
      vaultId: string,
      amount: number,
      bankAccountId: string
    ): Promise<WithdrawalOrder | null> => {
      setIsProcessing(true)
      setError(null)
      try {
        const idempotencyKey = generateIdempotencyKey()
        // Attempt the real API call — will throw 501 until fiat endpoints exist
        const result = await apiClient.initiateWithdrawal(
          amount,
          bankAccountId,
          idempotencyKey
        )
        const order: WithdrawalOrder = {
          id: result.withdrawalId,
          flow: 'withdrawal',
          vaultId,
          amount,
          bankAccountId,
          status: result.status,
          fees: result.fees,
          conversion: result.conversion,
          failureReason: null,
          idempotencyKey,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        }
        addWithdrawalOrder(order)
        return order
      } catch (err) {
        if (err instanceof ApiError && err.statusCode === 501) {
          // Create a local order as fallback
          const idempotencyKey = generateIdempotencyKey()
          const order: WithdrawalOrder = {
            id: `local-${idempotencyKey}`,
            flow: 'withdrawal',
            vaultId,
            amount,
            bankAccountId,
            status: 'pending',
            fees: null,
            conversion: null,
            failureReason: null,
            idempotencyKey,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
          }
          addWithdrawalOrder(order)
          return order
        }
        const message =
          err instanceof ApiError
            ? err.message
            : 'Failed to initiate withdrawal'
        setError(message)
        return null
      } finally {
        setIsProcessing(false)
      }
    },
    [addWithdrawalOrder]
  )

  return {
    vaults,
    isProcessing,
    error,
    createVault,
    fetchVaults,
    fetchVault,
    depositToVault,
    withdrawFromVault,
    lockVault,
    unlockVault,
    closeVault,
    initiateFunding,
    initiateWithdrawal,
  }
}