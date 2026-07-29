# Transactional Outbox Implementation — Changes Documentation

## Overview

This document explains the changes made to implement a transactional outbox pattern for reliable financial and notification job processing in the Vaulty backend.

## Problem Statement

Previously, the backend used a direct BullMQ approach where database writes and queue submissions were separate operations. This created a race condition where:

- A database write could succeed while queue submission failed, causing lost verification emails, duplicate payment processing, stale streak calculations, or notifications referring to records that do not exist yet.
- A job could run before its related database transaction committed, leading to inconsistent state.

## Solution: Transactional Outbox Pattern

The transactional outbox pattern ensures that domain changes and outbox events are written atomically in the same Prisma transaction. A dedicated outbox processor then safely publishes unpublished events to BullMQ queues.

## Files Changed

### New Files

| File | Purpose |
|---|---|
| `Backend/src/repositories/outbox.repository.ts` | CRUD operations for `OutboxEvent` records — create, find pending, mark published/failed/dead-letter, reset failed events |
| `Backend/src/services/notification.service.ts` | Notification service with transactional outbox integration — creates notifications and outbox events in the same Prisma transaction |
| `Backend/src/jobs/outbox.processor.ts` | Dedicated processor that polls pending outbox events and publishes them to the correct BullMQ queue with idempotency and retry support |
| `Backend/tests/unit/outbox.repository.test.ts` | Unit tests for the outbox repository — mocking Prisma and verifying CRUD operations |
| `Backend/tests/integration/outbox.integration.test.ts` | Integration tests verifying transactional atomicity, failed queue publication does not lose events, idempotency, and dead-letter transitions |
| `Backend/DOCUMENTATION/outbox-changes.md` | This file — explains all changes made |

### Modified Files

| File | Changes |
|---|---|
| `Backend/prisma/schema.prisma` | Added `OutboxEvent` model with fields: `id`, `eventType`, `aggregateId`, `aggregateType`, `payload`, `attemptCount`, `maxAttempts`, `status`, `nextRetryAt`, `publishedAt`, `failedAt`, `deadLetterReason`, `createdAt`, `updatedAt`. Added `OutboxEventStatus` and `OutboxEventType` enums. Added database indexes on `(status, nextRetryAt)`, `(aggregateId, aggregateType)`, and `(eventType)`. |
| `Backend/src/queues/index.ts` | Added `OUTBOX_PROCESSOR` queue name and `getOutboxProcessorQueue()` getter |
| `Backend/src/jobs/index.ts` | Added `initializeOutboxProcessor()` and `stopOutboxProcessorSafe()` functions; integrated outbox processor lifecycle with server startup/shutdown |
| `Backend/src/services/auth.service.ts` | Replaced direct `queueVerificationEmail`/`queuePasswordResetEmail` calls with Prisma `$transaction` blocks that write outbox events atomically with domain state |
| `Backend/src/services/vault.service.ts` | Wrapped vault creation, deposit, withdrawal, lock, unlock, and close operations in Prisma `$transaction` blocks that write outbox events alongside domain data |
| `Backend/src/services/payment.service.ts` | Wrapped payment initiation (deposit/withdrawal) and instruction requests in Prisma `$transaction` blocks that write outbox events alongside domain data |
| `Backend/src/services/notification.service.ts` | New service — creates notifications and outbox events in the same Prisma transaction |
| `Backend/src/repositories/user.repository.ts` | Added `createOutboxEvent()` helper method for convenience |
| `Backend/src/server.ts` | Added `stopOutboxProcessorSafe()` call in shutdown sequence; imported outbox processor lifecycle functions |

## Architecture

### Write Path

```
Business Service
  └─ prisma.$transaction(async (tx) => {
       ├─ tx.domainModel.create(...)   // e.g., user, payment, vault transaction
       └─ tx.outboxEvent.create(...)   // same transaction, atomic
     })
```

### Publish Path

```
Outbox Processor (continuous loop)
  ├─ Find pending events (status=PENDING, nextRetryAt <= now, attemptCount < 5)
  ├─ For each event:
  │   ├─ Determine target queue from eventType
  │   ├─ Publish to BullMQ queue
  │   ├─ On success: mark PUBLISHED
  │   ├─ On transient failure: mark FAILED with exponential backoff
  │   └─ On max retries: mark DEAD_LETTER
  └─ Sleep 1s when no pending events, then repeat
```

### Shutdown Path

```
SIGTERM / SIGINT
  ├─ stopOutboxProcessorSafe()   // stop polling for new events
  ├─ disconnectPrisma()          // close DB connections
  ├─ disconnectRedis()           // close Redis connections
  └─ closeQueueConnections()     // close BullMQ queues
```

## Idempotency

Each outbox event is keyed by `(aggregateId, eventType)`. The `findByIdempotencyKey` method checks for existing pending or published events before creating a new one, preventing duplicate queue submissions on retries.

## Retry Behavior

- **Max attempts**: 5
- **Backoff**: Exponential starting at 5 seconds, doubling each attempt, capped at 300 seconds
- **Scheduling**: Failed events get a `nextRetryAt` timestamp; the processor only picks up events whose retry time has arrived
- **Terminal state**: After 5 failed attempts, the event is moved to `DEAD_LETTER` with a `deadLetterReason`

## Event Types

| Event Type | Queue | Aggregate |
|---|---|---|
| `EMAIL_VERIFICATION` | `email` | User |
| `PASSWORD_RESET` | `email` | User |
| `EMAIL_RESEND` | `email` | User |
| `PAYMENT_INITIATED` | `payment-processing` | Payment |
| `PAYMENT_INSTRUCTIONS` | `payment-processing` | Payment |
| `PAYMENT_STATUS_UPDATE` | `payment-processing` | Payment |
| `VAULT_DEPOSIT` | `vault-reconciliation` | VaultTransaction |
| `VAULT_WITHDRAWAL` | `vault-reconciliation` | VaultTransaction |
| `VAULT_LOCK` | `vault-reconciliation` | SavingsVault |
| `VAULT_UNLOCK` | `vault-reconciliation` | SavingsVault |
| `VAULT_CLOSE` | `vault-reconciliation` | SavingsVault |
| `NOTIFICATION` | `notifications` | Notification |
| `STREAK_UPDATE` | `streak-calculation` | User |
| `RECONCILIATION` | `stellar-confirmation` | Transaction |

## Monitoring

- **Outbox lag**: Count of `PENDING` events with `nextRetryAt <= now`
- **Dead-letter queue**: Count of `DEAD_LETTER` events (should be zero)
- **Publish failure rate**: Ratio of `FAILED` events to total published events
- **Event age**: `PENDING` events older than 5 minutes indicate processor issues

## Recovery

- **Automatic**: Failed events are retried automatically with exponential backoff
- **Manual**: Dead-letter events can be inspected via SQL and re-published by resetting their status to `PENDING`
- **Reset script**: A one-off script can reset `FAILED` events past their retry time back to `PENDING` status