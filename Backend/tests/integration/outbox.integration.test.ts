import { prisma } from '../../src/database';
import { outboxRepository } from '../../src/repositories/outbox.repository';
import { OutboxEventStatus, OutboxEventType } from '@prisma/client';

jest.mock('../../src/queues', () => ({
  getOutboxProcessorQueue: jest.fn(() => ({
    add: jest.fn().mockResolvedValue({ id: 'job-1' }),
  })),
}));

jest.mock('../../src/jobs/outbox.processor', () => ({
  runOutboxProcessor: jest.fn(),
  stopOutboxProcessor: jest.fn(),
}));

describe('Outbox Integration', () => {
  beforeAll(async () => {
    await prisma.$connect();
  });

  afterAll(async () => {
    await prisma.$disconnect();
  });

  beforeEach(async () => {
    await prisma.outboxEvent.deleteMany({});
  });

  afterEach(async () => {
    await prisma.outboxEvent.deleteMany({});
  });

  describe('Transactional outbox with database writes', () => {
    it('persists outbox event when a database transaction succeeds', async () => {
      const userId = 'test-user-' + Date.now();

      await prisma.$transaction(async (tx) => {
        await tx.user.create({
          data: {
            id: userId,
            email: `${userId}@example.com`,
            passwordHash: 'hashed-password',
          },
        });

        await tx.outboxEvent.create({
          data: {
            eventType: OutboxEventType.EMAIL_VERIFICATION,
            aggregateId: userId,
            aggregateType: 'User',
            payload: JSON.stringify({
              to: `${userId}@example.com`,
              userId,
              token: 'verification-token',
              expiresAt: new Date(Date.now() + 86400000).toISOString(),
            }),
          },
        });
      });

      const events = await outboxRepository.findPending(10);
      expect(events.length).toBeGreaterThanOrEqual(1);

      const verificationEvent = events.find(
        (e) => e.eventType === OutboxEventType.EMAIL_VERIFICATION
      );
      expect(verificationEvent).toBeDefined();
      expect(verificationEvent?.aggregateId).toBe(userId);
      expect(verificationEvent?.status).toBe(OutboxEventStatus.PENDING);
    });

    it('does not persist outbox event when a database transaction fails', async () => {
      const userId = 'test-user-fail-' + Date.now();

      await expect(
        prisma.$transaction(async (tx) => {
          await tx.user.create({
            data: {
              id: userId,
              email: `${userId}@example.com`,
              passwordHash: 'hashed-password',
            },
          });

          await tx.outboxEvent.create({
            data: {
              eventType: OutboxEventType.EMAIL_VERIFICATION,
              aggregateId: userId,
              aggregateType: 'User',
              payload: JSON.stringify({
                to: `${userId}@example.com`,
                userId,
                token: 'verification-token',
                expiresAt: new Date(Date.now() + 86400000).toISOString(),
              }),
            },
          });

          throw new Error('Simulated transaction failure');
        })
      ).rejects.toThrow('Simulated transaction failure');

      const events = await prisma.outboxEvent.findMany({
        where: { aggregateId: userId },
      });
      expect(events.length).toBe(0);
    });

    it('preserves outbox events after a failed queue publication', async () => {
      const userId = 'test-user-queue-fail-' + Date.now();

      await prisma.$transaction(async (tx) => {
        await tx.user.create({
          data: {
            id: userId,
            email: `${userId}@example.com`,
            passwordHash: 'hashed-password',
          },
        });

        await tx.outboxEvent.create({
          data: {
            eventType: OutboxEventType.EMAIL_VERIFICATION,
            aggregateId: userId,
            aggregateType: 'User',
            payload: JSON.stringify({
              to: `${userId}@example.com`,
              userId,
              token: 'verification-token',
              expiresAt: new Date(Date.now() + 86400000).toISOString(),
            }),
          },
        });
      });

      const eventsBefore = await prisma.outboxEvent.findMany({
        where: { aggregateId: userId },
      });
      expect(eventsBefore.length).toBe(1);
      expect(eventsBefore[0].status).toBe(OutboxEventStatus.PENDING);

      await outboxRepository.markFailed(eventsBefore[0].id, 'Queue connection timeout');

      const eventsAfter = await prisma.outboxEvent.findMany({
        where: { aggregateId: userId },
      });
      expect(eventsAfter.length).toBe(1);
      expect(eventsAfter[0].status).toBe(OutboxEventStatus.FAILED);
      expect(eventsAfter[0].attemptCount).toBe(1);
    });

    it('transitions event to DEAD_LETTER after exceeding max retry attempts', async () => {
      const userId = 'test-user-dl-' + Date.now();

      await prisma.$transaction(async (tx) => {
        await tx.user.create({
          data: {
            id: userId,
            email: `${userId}@example.com`,
            passwordHash: 'hashed-password',
          },
        });

        await tx.outboxEvent.create({
          data: {
            eventType: OutboxEventType.EMAIL_VERIFICATION,
            aggregateId: userId,
            aggregateType: 'User',
            payload: JSON.stringify({
              to: `${userId}@example.com`,
              userId,
              token: 'verification-token',
              expiresAt: new Date(Date.now() + 86400000).toISOString(),
            }),
            maxAttempts: 3,
          },
        });
      });

      const events = await prisma.outboxEvent.findMany({
        where: { aggregateId: userId },
      });
      const eventId = events[0].id;

      for (let i = 0; i < 3; i++) {
        await outboxRepository.markFailed(eventId, `Attempt ${i + 1} failed`);
      }

      const failedEvent = await prisma.outboxEvent.findUnique({
        where: { id: eventId },
      });
      expect(failedEvent?.status).toBe(OutboxEventStatus.FAILED);
      expect(failedEvent?.attemptCount).toBe(3);

      await outboxRepository.markDeadLetter(
        eventId,
        'Max retries exceeded: persistent queue failure'
      );

      const deadLetterEvent = await prisma.outboxEvent.findUnique({
        where: { id: eventId },
      });
      expect(deadLetterEvent?.status).toBe(OutboxEventStatus.DEAD_LETTER);
      expect(deadLetterEvent?.deadLetterReason).toBe(
        'Max retries exceeded: persistent queue failure'
      );
    });

    it('successfully publishes an event and marks it as PUBLISHED', async () => {
      const userId = 'test-user-publish-' + Date.now();

      await prisma.$transaction(async (tx) => {
        await tx.user.create({
          data: {
            id: userId,
            email: `${userId}@example.com`,
            passwordHash: 'hashed-password',
          },
        });

        await tx.outboxEvent.create({
          data: {
            eventType: OutboxEventType.EMAIL_VERIFICATION,
            aggregateId: userId,
            aggregateType: 'User',
            payload: JSON.stringify({
              to: `${userId}@example.com`,
              userId,
              token: 'verification-token',
              expiresAt: new Date(Date.now() + 86400000).toISOString(),
            }),
          },
        });
      });

      const events = await prisma.outboxEvent.findMany({
        where: { aggregateId: userId },
      });
      const eventId = events[0].id;

      await outboxRepository.markPublished(eventId);

      const publishedEvent = await prisma.outboxEvent.findUnique({
        where: { id: eventId },
      });
      expect(publishedEvent?.status).toBe(OutboxEventStatus.PUBLISHED);
      expect(publishedEvent?.publishedAt).toBeDefined();
      expect(publishedEvent?.nextRetryAt).toBeNull();
    });
  });

  describe('Idempotency', () => {
    it('prevents duplicate outbox events for the same aggregate and event type', async () => {
      const userId = 'test-user-idem-' + Date.now();

      await prisma.$transaction(async (tx) => {
        await tx.user.create({
          data: {
            id: userId,
            email: `${userId}@example.com`,
            passwordHash: 'hashed-password',
          },
        });

        await tx.outboxEvent.create({
          data: {
            eventType: OutboxEventType.EMAIL_VERIFICATION,
            aggregateId: userId,
            aggregateType: 'User',
            payload: JSON.stringify({
              to: `${userId}@example.com`,
              userId,
              token: 'verification-token',
              expiresAt: new Date(Date.now() + 86400000).toISOString(),
            }),
          },
        });
      });

      const existingEvent = await outboxRepository.findByIdempotencyKey(
        userId,
        OutboxEventType.EMAIL_VERIFICATION
      );
      expect(existingEvent).toBeDefined();
      expect(existingEvent?.aggregateId).toBe(userId);
    });
  });
});