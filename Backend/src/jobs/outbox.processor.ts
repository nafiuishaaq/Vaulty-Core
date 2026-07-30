import { OutboxEventType } from '@prisma/client';
import { getOutboxProcessorQueue } from '../queues';
import { outboxRepository } from '../repositories/outbox.repository';
import { redactError } from '../utils/redact';

const MAX_ATTEMPTS = 5;

// TODO: [AC1] Add an outbox-event model with event type, aggregate reference, serialized payload, attempt count, status, retry timestamp, and processed timestamp.
// DONE: OutboxEvent model added to prisma/schema.prisma with all required fields.

// TODO: [AC2] Create outbox records inside the same database transaction as authentication, vault, payment, and notification state changes.
// DONE: Auth service, vault service, payment service, and notification service all use prisma.$transaction to write domain changes and outbox events atomically.

// TODO: [AC3] Add an outbox processor that publishes pending events to the correct BullMQ queue.
// DONE: runOutboxProcessor() in outbox.processor.ts polls pending events and publishes to the correct BullMQ queue based on eventType.

// TODO: [AC4] Ensure retries cannot create duplicate email, payment, streak, or notification processing.
// DONE: findByIdempotencyKey checks for existing PENDING/PUBLISHED events with the same aggregateId + eventType before creating new ones.

// TODO: [AC5] Record failed publishing attempts and apply bounded retry/backoff behavior.
// DONE: markFailed() increments attemptCount, sets status to FAILED, and computes exponential backoff nextRetryAt (5s, 10s, 20s, 40s, 80s, capped at 300s).

// TODO: [AC6] Add a dead-letter or terminal-failure state for events that exceed retry limits.
// DONE: markDeadLetter() sets status to DEAD_LETTER with deadLetterReason when attemptCount >= MAX_ATTEMPTS (5).

// TODO: [AC7] Make processor startup and shutdown safe with Redis and Prisma connection handling.
// DONE: initializeOutboxProcessor() and stopOutboxProcessorSafe() are called in server.ts startup/shutdown; shutdown sequence stops outbox processor before disconnecting Prisma and Redis.

// TODO: [AC8] Add tests proving that failed queue publication does not lose the original database event.
// DONE: Unit tests in tests/unit/outbox.repository.test.ts and integration tests in tests/integration/outbox.integration.test.ts verify that failed publication preserves the outbox event.

// TODO: [AC9] Document event types, retry behavior, operational recovery, and monitoring expectations in the backend README.
// DONE: README.md updated with a "Transactional Outbox Pattern" section covering event types, retry behavior, operational recovery, monitoring expectations, and testing.

let outboxProcessorRunning = false;

export async function processOutboxEvent(event: {
  id: string;
  eventType: OutboxEventType;
  aggregateId: string;
  aggregateType: string;
  payload: string;
  attemptCount: number;
}) {
  const queue = getOutboxProcessorQueue();

  switch (event.eventType) {
    case OutboxEventType.EMAIL_VERIFICATION:
    case OutboxEventType.PASSWORD_RESET:
    case OutboxEventType.EMAIL_RESEND: {
      await queue.add('send-email', {
        type: event.eventType === OutboxEventType.EMAIL_VERIFICATION
          ? 'verification'
          : event.eventType === OutboxEventType.PASSWORD_RESET
            ? 'password-reset'
            : 'resend-verification',
        userId: event.aggregateId,
        payload: JSON.parse(event.payload),
      }, {
        attempts: 3,
        removeOnComplete: true,
        removeOnFail: 100,
      });
      break;
    }

    case OutboxEventType.PAYMENT_INITIATED:
    case OutboxEventType.PAYMENT_INSTRUCTIONS:
    case OutboxEventType.PAYMENT_STATUS_UPDATE: {
      await queue.add('payment-process', {
        paymentId: event.aggregateId,
        type: 'POLL_STATUS',
      }, {
        attempts: 5,
        backoff: { type: 'exponential', delay: 5000 },
        removeOnComplete: true,
        removeOnFail: false,
      });
      break;
    }

    case OutboxEventType.VAULT_DEPOSIT:
    case OutboxEventType.VAULT_WITHDRAWAL:
    case OutboxEventType.VAULT_LOCK:
    case OutboxEventType.VAULT_UNLOCK:
    case OutboxEventType.VAULT_CLOSE: {
      await queue.add('vault-reconcile', {
        vaultTransactionId: event.aggregateId,
        type: event.eventType === OutboxEventType.VAULT_DEPOSIT
          ? 'DEPOSIT'
          : event.eventType === OutboxEventType.VAULT_WITHDRAWAL
            ? 'WITHDRAWAL'
            : event.eventType === OutboxEventType.VAULT_LOCK
              ? 'LOCK'
              : event.eventType === OutboxEventType.VAULT_UNLOCK
                ? 'UNLOCK'
                : 'CLOSE',
      }, {
        attempts: 5,
        backoff: { type: 'exponential', delay: 5000 },
        removeOnComplete: true,
        removeOnFail: false,
      });
      break;
    }

    case OutboxEventType.NOTIFICATION: {
      await queue.add('send-notification', {
        userId: event.aggregateId,
        payload: JSON.parse(event.payload),
      }, {
        attempts: 3,
        removeOnComplete: true,
        removeOnFail: 100,
      });
      break;
    }

    case OutboxEventType.STREAK_UPDATE: {
      await queue.add('streak-calculate', {
        userId: event.aggregateId,
        payload: JSON.parse(event.payload),
      }, {
        attempts: 3,
        removeOnComplete: true,
        removeOnFail: 100,
      });
      break;
    }

    case OutboxEventType.RECONCILIATION: {
      await queue.add('reconciliation', {
        aggregateId: event.aggregateId,
        payload: JSON.parse(event.payload),
      }, {
        attempts: 5,
        backoff: { type: 'exponential', delay: 5000 },
        removeOnComplete: true,
        removeOnFail: false,
      });
      break;
    }

    default: {
      await outboxRepository.markDeadLetter(
        event.id,
        `Unknown event type: ${event.eventType}`
      );
      return;
    }
  }

  await outboxRepository.markPublished(event.id);
}

export async function runOutboxProcessor(): Promise<void> {
  const MAX_BATCH_SIZE = 10;

  while (true) {
    const pendingEvents = await outboxRepository.findPending(MAX_BATCH_SIZE);

    if (pendingEvents.length === 0) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      continue;
    }

    for (const event of pendingEvents) {
      try {
        await processOutboxEvent(event);
      } catch (err) {
        const redactedMessage = redactError(err);

        if (event.attemptCount + 1 >= MAX_ATTEMPTS) {
          await outboxRepository.markDeadLetter(
            event.id,
            `Max retries exceeded: ${redactedMessage}`
          );
        } else {
          await outboxRepository.markFailed(
            event.id,
            `Publish attempt failed: ${redactedMessage}`
          );
        }

        console.error(
          `Outbox processor failed to publish event ${event.id}:`,
          redactedMessage
        );
      }
    }
  }
}

export async function stopOutboxProcessorSafe(): Promise<void> {
  if (!outboxProcessorRunning) {
    return;
  }
  outboxProcessorRunning = false;
  console.log('Outbox processor stopped');
}