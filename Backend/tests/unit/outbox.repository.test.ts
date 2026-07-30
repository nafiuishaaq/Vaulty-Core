import { outboxRepository } from '../../src/repositories/outbox.repository';
import { OutboxEventStatus, OutboxEventType } from '@prisma/client';

jest.mock('../../src/database', () => ({
  prisma: {
    outboxEvent: {
      create: jest.fn(),
      findUnique: jest.fn(),
      findMany: jest.fn(),
      findFirst: jest.fn(),
      update: jest.fn(),
      updateMany: jest.fn(),
      deleteMany: jest.fn(),
    },
  },
}));

const mockPrisma = require('../../src/database').prisma;

describe('OutboxRepository', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('create', () => {
    it('creates an outbox event with the given data', async () => {
      const mockEvent = {
        id: 'event-1',
        eventType: OutboxEventType.EMAIL_VERIFICATION,
        aggregateId: 'user-1',
        aggregateType: 'User',
        payload: JSON.stringify({ to: 'test@example.com' }),
        attemptCount: 0,
        status: OutboxEventStatus.PENDING,
        createdAt: new Date(),
      };
      mockPrisma.outboxEvent.create.mockResolvedValue(mockEvent);

      const result = await outboxRepository.create({
        eventType: OutboxEventType.EMAIL_VERIFICATION,
        aggregateId: 'user-1',
        aggregateType: 'User',
        payload: JSON.stringify({ to: 'test@example.com' }),
      });

      expect(mockPrisma.outboxEvent.create).toHaveBeenCalledWith({
        data: {
          eventType: OutboxEventType.EMAIL_VERIFICATION,
          aggregateId: 'user-1',
          aggregateType: 'User',
          payload: JSON.stringify({ to: 'test@example.com' }),
        },
      });
      expect(result).toEqual(mockEvent);
    });
  });

  describe('findPending', () => {
    it('returns pending events that are due for retry', async () => {
      const mockEvents = [
        {
          id: 'event-1',
          eventType: OutboxEventType.EMAIL_VERIFICATION,
          aggregateId: 'user-1',
          aggregateType: 'User',
          payload: '{}',
          attemptCount: 0,
          status: OutboxEventStatus.PENDING,
          nextRetryAt: null,
        },
      ];
      mockPrisma.outboxEvent.findMany.mockResolvedValue(mockEvents);

      const result = await outboxRepository.findPending(10);

      expect(mockPrisma.outboxEvent.findMany).toHaveBeenCalledWith(
        expect.objectContaining({
          where: expect.objectContaining({
            status: OutboxEventStatus.PENDING,
            attemptCount: { lt: 5 },
          }),
          orderBy: { createdAt: 'asc' },
          take: 10,
        })
      );
      expect(result).toEqual(mockEvents);
    });

    it('returns empty array when no pending events exist', async () => {
      mockPrisma.outboxEvent.findMany.mockResolvedValue([]);

      const result = await outboxRepository.findPending(10);

      expect(result).toEqual([]);
    });
  });

  describe('findByIdempotencyKey', () => {
    it('returns the most recent pending or published event for the same aggregate and type', async () => {
      const mockEvent = {
        id: 'event-1',
        eventType: OutboxEventType.EMAIL_VERIFICATION,
        aggregateId: 'user-1',
        aggregateType: 'User',
        payload: '{}',
        status: OutboxEventStatus.PENDING,
      };
      mockPrisma.outboxEvent.findFirst.mockResolvedValue(mockEvent);

      const result = await outboxRepository.findByIdempotencyKey('user-1', OutboxEventType.EMAIL_VERIFICATION);

      expect(mockPrisma.outboxEvent.findFirst).toHaveBeenCalledWith(
        expect.objectContaining({
          where: expect.objectContaining({
            aggregateId: 'user-1',
            eventType: OutboxEventType.EMAIL_VERIFICATION,
            status: {
              in: [OutboxEventStatus.PENDING, OutboxEventStatus.PUBLISHED],
            },
          }),
          orderBy: { createdAt: 'desc' },
        })
      );
      expect(result).toEqual(mockEvent);
    });
  });

  describe('markPublished', () => {
    it('updates the event status to PUBLISHED with a timestamp', async () => {
      const mockEvent = {
        id: 'event-1',
        status: OutboxEventStatus.PUBLISHED,
        publishedAt: new Date(),
      };
      mockPrisma.outboxEvent.update.mockResolvedValue(mockEvent);

      const result = await outboxRepository.markPublished('event-1');

      expect(mockPrisma.outboxEvent.update).toHaveBeenCalledWith({
        where: { id: 'event-1' },
        data: expect.objectContaining({
          status: OutboxEventStatus.PUBLISHED,
          publishedAt: expect.any(Date),
          nextRetryAt: null,
        }),
      });
      expect(result).toEqual(mockEvent);
    });
  });

  describe('markFailed', () => {
    it('increments attempt count and sets FAILED status with retry timestamp', async () => {
      const mockEvent = {
        id: 'event-1',
        status: OutboxEventStatus.FAILED,
        attemptCount: 1,
        nextRetryAt: new Date(Date.now() + 60000),
      };
      mockPrisma.outboxEvent.update.mockResolvedValue(mockEvent);

      const result = await outboxRepository.markFailed('event-1', 'Publish failed');

      expect(mockPrisma.outboxEvent.update).toHaveBeenCalledWith({
        where: { id: 'event-1' },
        data: expect.objectContaining({
          attemptCount: { increment: 1 },
          status: OutboxEventStatus.FAILED,
          nextRetryAt: expect.any(Date),
          failedAt: expect.any(Date),
          deadLetterReason: 'Publish failed',
        }),
      });
      expect(result).toEqual(mockEvent);
    });
  });

  describe('markDeadLetter', () => {
    it('sets the event status to DEAD_LETTER with a reason', async () => {
      const mockEvent = {
        id: 'event-1',
        status: OutboxEventStatus.DEAD_LETTER,
        deadLetterReason: 'Max retries exceeded',
      };
      mockPrisma.outboxEvent.update.mockResolvedValue(mockEvent);

      const result = await outboxRepository.markDeadLetter('event-1', 'Max retries exceeded');

      expect(mockPrisma.outboxEvent.update).toHaveBeenCalledWith({
        where: { id: 'event-1' },
        data: expect.objectContaining({
          status: OutboxEventStatus.DEAD_LETTER,
          deadLetterReason: 'Max retries exceeded',
          failedAt: expect.any(Date),
        }),
      });
      expect(result).toEqual(mockEvent);
    });
  });

  describe('resetFailedEvents', () => {
    it('resets failed events that are within retry budget and past their retry time', async () => {
      const mockResult = { count: 3 };
      mockPrisma.outboxEvent.updateMany.mockResolvedValue(mockResult);

      const result = await outboxRepository.resetFailedEvents();

      expect(mockPrisma.outboxEvent.updateMany).toHaveBeenCalledWith(
        expect.objectContaining({
          where: expect.objectContaining({
            status: OutboxEventStatus.FAILED,
            attemptCount: { lt: 5 },
            nextRetryAt: { lte: expect.any(Date) },
          }),
          data: expect.objectContaining({
            status: OutboxEventStatus.PENDING,
            nextRetryAt: null,
          }),
        })
      );
      expect(result).toEqual(mockResult);
    });
  });

  describe('deletePublishedOlderThan', () => {
    it('deletes published events older than the cutoff date', async () => {
      const mockResult = { count: 50 };
      mockPrisma.outboxEvent.deleteMany.mockResolvedValue(mockResult);

      const cutoff = new Date('2024-01-01');
      const result = await outboxRepository.deletePublishedOlderThan(cutoff);

      expect(mockPrisma.outboxEvent.deleteMany).toHaveBeenCalledWith(
        expect.objectContaining({
          where: expect.objectContaining({
            status: OutboxEventStatus.PUBLISHED,
            publishedAt: { lt: cutoff },
          }),
        })
      );
      expect(result).toEqual(mockResult);
    });
  });
});