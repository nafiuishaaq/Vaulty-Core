import { prisma } from '../database';
import { AppError } from '../utils/AppError';
import { OutboxEventType } from '@prisma/client';

export class NotificationService {
  async sendNotification(userId: string, title: string, body: string, type: string) {
    const notification = await prisma.$transaction(async (tx) => {
      const created = await tx.notification.create({
        data: {
          userId,
          title,
          body,
          type,
          read: false,
        },
      });

      await tx.outboxEvent.create({
        data: {
          eventType: OutboxEventType.NOTIFICATION,
          aggregateId: created.id,
          aggregateType: 'Notification',
          payload: JSON.stringify({
            notificationId: created.id,
            userId,
            title,
            body,
            type,
          }),
        },
      });

      return created;
    });

    return notification;
  }

  async getUserNotifications(userId: string, query: { page?: number; limit?: number; read?: boolean }) {
    const page = query.page ?? 1;
    const limit = query.limit ?? 20;
    const skip = (page - 1) * limit;

    const where: any = { userId };
    if (query.read !== undefined) {
      where.read = query.read;
    }

    const [notifications, total] = await Promise.all([
      prisma.notification.findMany({
        where,
        orderBy: { createdAt: 'desc' },
        skip,
        take: limit,
      }),
      prisma.notification.count({ where }),
    ]);

    return {
      notifications,
      pagination: {
        total,
        page,
        limit,
        pages: Math.ceil(total / limit),
      },
    };
  }

  async markAsRead(userId: string, notificationId: string) {
    const notification = await prisma.notification.findUnique({
      where: { id: notificationId },
    });

    if (!notification) {
      throw new AppError('Notification not found', 404);
    }

    if (notification.userId !== userId) {
      throw new AppError('Notification not found', 404);
    }

    const updated = await prisma.notification.update({
      where: { id: notificationId },
      data: { read: true },
    });

    return updated;
  }

  async markAllAsRead(userId: string) {
    await prisma.notification.updateMany({
      where: { userId, read: false },
      data: { read: true },
    });

    return { message: 'All notifications marked as read' };
  }
}

export const notificationService = new NotificationService();