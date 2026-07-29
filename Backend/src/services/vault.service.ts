import { VaultStatus, VaultTransactionType, VaultTransactionStatus, VaultType, OutboxEventType, VaultEventType, StellarSubmissionStatus } from '@prisma/client';
import { vaultRepository } from '../repositories/vault.repository';
import { vaultTransactionRepository } from '../repositories/vault-transaction.repository';
import { vaultEventRepository } from '../repositories/vault-event.repository';
import { AppError } from '../utils/AppError';
import { getVaultQueue } from '../queues';
import { prisma } from '../database';
import type {
  CreateVaultInput,
  DepositToVaultInput,
  WithdrawFromVaultInput,
  LockVaultInput,
} from '../validators/vault.validator';

const generateReference = (type: string): string => `${type.toLowerCase()}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;

export class VaultService {
  /**
   * Create a new savings vault for the authenticated user.
   */
  async createVault(userId: string, input: CreateVaultInput) {
    const existing = await this.checkDuplicateIdempotency(input.idempotencyKey, userId);
    if (existing) {
      return existing;
    }

    const vault = await prisma.$transaction(async (tx) => {
      const createdVault = await tx.savingsVault.create({
        data: {
          userId,
          name: input.name,
          description: input.description,
          targetAmount: input.targetAmount,
          lockPeriod: input.lockPeriod,
          assetCode: input.assetCode,
          assetIssuer: input.assetIssuer,
          contractAddress: input.contractAddress,
          onChainVaultId: input.onChainVaultId,
          type: input.type,
          goalDescription: input.goalDescription,
        },
      });

      await prisma.outboxEvent.create({
        data: {
        data: {
          eventType: OutboxEventType.VAULT_CLOSE,
          aggregateId: createdVault.id,
          aggregateType: 'SavingsVault',
          payload: JSON.stringify({
            name: createdVault.name,
            targetAmount: createdVault.targetAmount,
            assetCode: createdVault.assetCode,
            type: createdVault.type,
          }),
        },
      });

      return createdVault;
    });

    return vault;
  }

  /**
   * List all vaults for the authenticated user with pagination.
   */
  async getVaults(userId: string, query: { page?: number; limit?: number; status?: VaultStatus; type?: VaultType }) {
    const page = query.page ?? 1;
    const limit = query.limit ?? 20;
    const skip = (page - 1) * limit;

    const [vaults, total] = await Promise.all([
      vaultRepository.findByUserId(userId, { skip, take: limit, status: query.status }),
      vaultRepository.countByUserId(userId, { status: query.status }),
    ]);

    return {
      vaults,
      pagination: {
        total,
        page,
        limit,
        pages: Math.ceil(total / limit),
      },
    };
  }

  /**
   * Get a single vault detail. Ownership is enforced.
   */
  async getVault(userId: string, vaultId: string) {
    const vault = await vaultRepository.findById(vaultId);
    if (!vault) {
      throw new AppError('Vault not found', 404);
    }
    if (vault.userId !== userId) {
      throw new AppError('Vault not found', 404);
    }
    return vault;
  }

  /**
   * Deposit funds into a vault. Records a PENDING transaction and never
   * mutates `currentAmount` before on-chain confirmation.
   */
  async deposit(userId: string, vaultId: string, input: DepositToVaultInput) {
    const vault = await this.getVault(userId, vaultId);

    if (vault.status === VaultStatus.CLOSED) {
      throw new AppError('Cannot deposit into a closed vault', 400);
    }

    const existing = await vaultTransactionRepository.findByIdempotencyKey(input.idempotencyKey);
    if (existing) {
      if (existing.userId !== userId) {
        throw new AppError('Idempotency key belongs to another user', 403);
      }
      if (existing.vaultId !== vaultId) {
        throw new AppError('Idempotency key belongs to a different vault', 400);
      }
      return existing;
    }

    const reference = generateReference('DEP');

    const result = await prisma.$transaction(async (tx) => {
      const transaction = await tx.vaultTransaction.create({
        data: {
          vaultId,
          userId,
          type: 'DEPOSIT',
          status: 'PENDING',
          amount: input.amount,
          description: input.description,
          reference,
          idempotencyKey: input.idempotencyKey,
          onChainVaultId: vault.onChainVaultId,
        },
      });

      await prisma.outboxEvent.create({
        data: {
        data: {
          eventType: OutboxEventType.VAULT_DEPOSIT,
          aggregateId: transaction.id,
          aggregateType: 'VaultTransaction',
          payload: JSON.stringify({
            amount: input.amount,
            reference,
            type: 'DEPOSIT',
          }),
        },
      });

      return transaction;
    });

    const queue = getVaultQueue();
    await queue.add('vault-reconcile', { vaultTransactionId: result.id, type: 'DEPOSIT' }, {
      attempts: 5,
      backoff: { type: 'exponential', delay: 5000 },
      removeOnComplete: true,
      removeOnFail: false,
    });

    return result;
  }

  /**
   * Withdraw funds from a vault. Enforces lock period and checks the ledger
   * balance which only includes CONFIRMED transactions.
   */
  async withdraw(userId: string, vaultId: string, input: WithdrawFromVaultInput) {
    const vault = await this.getVault(userId, vaultId);

    if (vault.status === VaultStatus.CLOSED) {
      throw new AppError('Cannot withdraw from a closed vault', 400);
    }

    if (vault.status === VaultStatus.LOCKED && vault.unlocksAt && new Date() < vault.unlocksAt) {
      throw new AppError('Vault is locked until ' + vault.unlocksAt.toISOString(), 400);
    }

    const vaultAmount = parseFloat(vault.currentAmount);
    const withdrawAmount = parseFloat(input.amount);

    if (withdrawAmount > vaultAmount) {
      throw new AppError('Insufficient vault balance', 400);
    }

    const existing = await vaultTransactionRepository.findByIdempotencyKey(input.idempotencyKey);
    if (existing) {
      if (existing.userId !== userId) {
        throw new AppError('Idempotency key belongs to another user', 403);
      }
      if (existing.vaultId !== vaultId) {
        throw new AppError('Idempotency key belongs to a different vault', 400);
      }
      return existing;
    }

    const reference = generateReference('WTH');

    const result = await prisma.$transaction(async (tx) => {
      const transaction = await tx.vaultTransaction.create({
        data: {
          vaultId,
          userId,
          type: 'WITHDRAWAL',
          status: 'PENDING',
          amount: input.amount,
          description: input.description,
          reference,
          idempotencyKey: input.idempotencyKey,
          onChainVaultId: vault.onChainVaultId,
        },
      });

      await prisma.outboxEvent.create({
        data: {
        data: {
          eventType: OutboxEventType.VAULT_WITHDRAWAL,
          aggregateId: transaction.id,
          aggregateType: 'VaultTransaction',
          payload: JSON.stringify({
            amount: input.amount,
            reference,
            type: 'WITHDRAWAL',
          }),
        },
      });

      return transaction;
    });

    const queue = getVaultQueue();
    await queue.add('vault-reconcile', { vaultTransactionId: result.id, type: 'WITHDRAWAL' }, {
      attempts: 5,
      backoff: { type: 'exponential', delay: 5000 },
      removeOnComplete: true,
      removeOnFail: false,
    });

    return result;
  }

  /**
   * Lock a vault for a specified period. Updates the lifecycle status.
   */
  async lockVault(userId: string, vaultId: string, input: LockVaultInput) {
    const vault = await this.getVault(userId, vaultId);

    if (vault.status === VaultStatus.CLOSED) {
      throw new AppError('Cannot lock a closed vault', 400);
    }

    const unlocksAt = new Date();
    unlocksAt.setDate(unlocksAt.getDate() + input.lockPeriod);

    const updated = await vaultRepository.update(vaultId, {
      status: VaultStatus.LOCKED,
      lockedAt: new Date(),
      unlocksAt,
    });

    await prisma.$transaction(async (tx) => {
      await tx.vaultEvent.create({
        data: {
          vaultId,
          userId,
          eventType: 'LOCK' as VaultEventType,
          status: 'CONFIRMED' as StellarSubmissionStatus,
          payload: JSON.stringify({
            lockPeriod: input.lockPeriod,
            unlocksAt: unlocksAt.toISOString(),
          }),
          confirmedAt: new Date(),
        },
      });

      await prisma.outboxEvent.create({
        data: {
        eventType: OutboxEventType.VAULT_LOCK,
        aggregateId: vaultId,
        aggregateType: 'SavingsVault',
        payload: JSON.stringify({
          lockPeriod: input.lockPeriod,
          unlocksAt: unlocksAt.toISOString(),
        }),
      });
    });

    return updated;
  }

  /**
   * Unlock a vault that has passed its unlock date.
   */
  async unlockVault(userId: string, vaultId: string) {
    const vault = await this.getVault(userId, vaultId);

    if (vault.status !== VaultStatus.LOCKED) {
      throw new AppError('Vault is not locked', 400);
    }

    if (vault.unlocksAt && new Date() < vault.unlocksAt) {
      throw new AppError('Vault is still locked until ' + vault.unlocksAt.toISOString(), 400);
    }

    const updated = await vaultRepository.update(vaultId, {
      status: VaultStatus.ACTIVE,
    });

    await prisma.$transaction(async (tx) => {
      await tx.vaultEvent.create({
        data: {
          vaultId,
          userId,
          eventType: 'UNLOCK' as VaultEventType,
          status: 'CONFIRMED' as StellarSubmissionStatus,
          payload: JSON.stringify({ unlockedAt: new Date().toISOString() }),
          confirmedAt: new Date(),
        },
      });

      await prisma.outboxEvent.create({
        data: {
        eventType: OutboxEventType.VAULT_UNLOCK,
        aggregateId: vaultId,
        aggregateType: 'SavingsVault',
        payload: JSON.stringify({ unlockedAt: new Date().toISOString() }),
      });
    });

    return updated;
  }

  /**
   * Close a vault permanently.
   */
  async closeVault(userId: string, vaultId: string) {
    const vault = await this.getVault(userId, vaultId);

    if (vault.status === VaultStatus.CLOSED) {
      throw new AppError('Vault is already closed', 400);
    }

    if (vault.currentAmount !== '0' && parseFloat(vault.currentAmount) > 0) {
      throw new AppError('Cannot close vault with a non-zero balance. Withdraw all funds first.', 400);
    }

    const updated = await vaultRepository.update(vaultId, {
      status: VaultStatus.CLOSED,
    });

    await prisma.$transaction(async (tx) => {
      await tx.vaultEvent.create({
        data: {
          vaultId,
          userId,
          eventType: 'CLOSE' as VaultEventType,
          status: 'CONFIRMED' as StellarSubmissionStatus,
          payload: JSON.stringify({ closedAt: new Date().toISOString() }),
          confirmedAt: new Date(),
        },
      });

      await prisma.outboxEvent.create({
        data: {
        eventType: OutboxEventType.VAULT_CLOSE,
        aggregateId: vaultId,
        aggregateType: 'SavingsVault',
        payload: JSON.stringify({ closedAt: new Date().toISOString() }),
      });
    });

    return updated;
  }

  /**
   * Get transaction history for a specific vault with pagination.
   * Only the owner can access this.
   */
  async getVaultHistory(userId: string, vaultId: string, query: { page?: number; limit?: number; status?: VaultTransactionStatus; type?: VaultTransactionType }) {
    await this.getVault(userId, vaultId);

    const page = query.page ?? 1;
    const limit = query.limit ?? 20;
    const skip = (page - 1) * limit;

    const [transactions, total] = await Promise.all([
      vaultTransactionRepository.findByVaultId(vaultId, { skip, take: limit, status: query.status, type: query.type }),
      vaultTransactionRepository.countByVaultId(vaultId, { status: query.status, type: query.type }),
    ]);

    return {
      transactions,
      pagination: {
        total,
        page,
        limit,
        pages: Math.ceil(total / limit),
      },
    };
  }

  /**
   * Get transaction history across all user vaults with pagination.
   */
  async getAllVaultHistory(userId: string, query: { page?: number; limit?: number; type?: VaultTransactionType }) {
    const page = query.page ?? 1;
    const limit = query.limit ?? 20;
    const skip = (page - 1) * limit;

    const [transactions, total] = await Promise.all([
      vaultTransactionRepository.findByUserId(userId, { skip, take: limit, type: query.type }),
      vaultTransactionRepository.countByUserId(userId, { type: query.type }),
    ]);

    return {
      transactions,
      pagination: {
        total,
        page,
        limit,
        pages: Math.ceil(total / limit),
      },
    };
  }

  /**
   * Reconcile a pending vault transaction with on-chain state.
   * This is called by the background worker.
   * BALANCE UPDATES ONLY HAPPEN FOR CONFIRMED TRANSACTIONS.
   */
  async reconcileVaultTransaction(vaultTransactionId: string) {
    const tx = await vaultTransactionRepository.findById(vaultTransactionId);
    if (!tx) {
      return;
    }

    if (tx.status === VaultTransactionStatus.CONFIRMED || tx.status === VaultTransactionStatus.FAILED || tx.status === VaultTransactionStatus.CANCELLED) {
      return;
    }

    const event = await vaultEventRepository.findByVaultId(tx.vaultId);

    const latestEvent = event[0];

    if (latestEvent && latestEvent.status === 'CONFIRMED' && latestEvent.transactionHash) {
      if (tx.type === 'DEPOSIT') {
        await vaultRepository.addBalance(tx.vaultId, tx.amount);
      } else if (tx.type === VaultTransactionType.WITHDRAWAL) {
        await vaultRepository.subtractBalance(tx.vaultId, tx.amount);
      }

      await vaultTransactionRepository.updateStatus(tx.id, VaultTransactionStatus.CONFIRMED, {
        stellarTransactionHash: latestEvent.transactionHash,
        confirmedAt: new Date(),
      });
      return;
    }

    if (latestEvent && latestEvent.status === 'FAILED') {
      await vaultTransactionRepository.updateStatus(tx.id, VaultTransactionStatus.FAILED, {
        failureCode: latestEvent.failureCode,
        failureReason: latestEvent.failureReason,
      });
      return;
    }

    await vaultTransactionRepository.update(tx.id, {});
  }

  /**
   * Background reconciliation for pending vault events.
   */
  async reconcileVaultEvents() {
    const pending = await vaultEventRepository.findPending(50);
    for (const event of pending) {
      await vaultEventRepository.updateStatus(event.id, event.status as any, {
        attempts: event.attempts + 1,
      });
    }
    return pending.length;
  }

  private async checkDuplicateIdempotency(idempotencyKey: string, userId: string) {
    const existing = await vaultRepository.findByUserId(userId);
    const match = existing.find((v) => v.id === idempotencyKey);
    return match || null;
  }
}

export const vaultService = new VaultService();
