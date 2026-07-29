import { Prisma, OutboxEventStatus, OutboxEventType } from '@prisma/client';
import { prisma } from '../database';

export class OutboxRepository {
  async create(data: Prisma.OutboxEventUncheckedCreateInput) {
    return prisma.outboxEvent.create({ data });
  }

  async findById(id: string) {
    return prisma.outboxEvent.findUnique({ where: { id } });
  }

  async findPending(limit = 50) {
    return prisma.outboxEvent.findMany({
      where: {
        status: OutboxEventStatus.PENDING,
        OR: [
          { nextRetryAt: { lte: new Date() } },
          { nextRetryAt: null },
        ],
        attemptCount: { lt: 5 },
      },
      orderBy: { createdAt: 'asc' },
      take: limit,
    });
  }

  async findByIdempotencyKey(aggregateId: string, eventType: OutboxEventType) {
    return prisma.outboxEvent.findFirst({
      where: {
        aggregateId,
        eventType,
        status: {
          in: [OutboxEventStatus.PENDING, OutboxEventStatus.PUBLISHED],
        },
      },
      orderBy: { createdAt: 'desc' },
    });
  }

  async markPublished(id: string) {
    return prisma.outboxEvent.update({
      where: { id },
      data: {
        status: OutboxEventStatus.PUBLISHED,
        publishedAt: new Date(),
        nextRetryAt: null,
        updatedAt: new Date(),
      },
    });
  }

  async markFailed(id: string, reason: string) {
    return prisma.outboxEvent.update({
      where: { id },
      data: {
        attemptCount: { increment: 1 },
        status: OutboxEventStatus.FAILED,
        nextRetryAt: new Date(Date.now() + Math.min(60000 * Math.pow(2, 0), 300000)),
        failedAt: new Date(),
        deadLetterReason: reason,
        updatedAt: new Date(),
      },
    });
  }

  async markDeadLetter(id: string, reason: string) {
    return prisma.outboxEvent.update({
      where: { id },
      data: {
        status: OutboxEventStatus.DEAD_LETTER,
        deadLetterReason: reason,
        failedAt: new Date(),
        updatedAt: new Date(),
      },
    });
  }

  async resetFailedEvents() {
    return prisma.outboxEvent.updateMany({
      where: {
        status: OutboxEventStatus.FAILED,
        attemptCount: { lt: 5 },
        nextRetryAt: { lte: new Date() },
      },
      data: {
        status: OutboxEventStatus.PENDING,
        nextRetryAt: null,
        updatedAt: new Date(),
      },
    });
  }

  async deletePublishedOlderThan(cutoff: Date) {
    return prisma.outboxEvent.deleteMany({
      where: {
        status: OutboxEventStatus.PUBLISHED,
        publishedAt: { lt: cutoff },
      },
    });
  }
}

export const outboxRepository = new OutboxRepository();