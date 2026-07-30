import { Router, Request, Response } from 'express';
import { prisma } from '../database';
import { redis } from '../config/redis';
import { redactError } from '../utils/redact';

const router = Router();

type HealthStatus = 'ok' | 'degraded';

type HealthCheck = {
  status: HealthStatus;
  details?: string;
};

const checkPrisma = async (): Promise<HealthCheck> => {
  try {
    await prisma.$queryRawUnsafe('SELECT 1');
    return { status: 'ok' };
  } catch (error) {
    return { status: 'degraded', details: redactError(error) };
  }
};

const checkRedis = (): HealthCheck => {
  if (redis.status === 'ready') {
    return { status: 'ok' };
  }

  return { status: 'degraded', details: `Redis status is ${redis.status}` };
};

router.get('/', (_req: Request, res: Response) => {
  res.json({
    success: true,
    message: 'Vaulty Backend is running',
    uptime: process.uptime(),
    timestamp: new Date().toISOString(),
  });
});

router.get('/ready', async (_req: Request, res: Response) => {
  const [prismaCheck, redisCheck] = await Promise.all([
    checkPrisma(),
    checkRedis(),
  ]);

  const isReady = prismaCheck.status === 'ok' && redisCheck.status === 'ok';

  if (isReady) {
    res.json({
      success: true,
      message: 'Vaulty Backend is ready',
      status: 'ok',
      timestamp: new Date().toISOString(),
      checks: {
        prisma: prismaCheck,
        redis: redisCheck,
      },
    });
    return;
  }

  res.status(503).json({
    success: false,
    message: 'Vaulty Backend is not ready',
    status: 'degraded',
    timestamp: new Date().toISOString(),
    checks: {
      prisma: prismaCheck,
      redis: redisCheck,
    },
  });
});

export const healthRouter = router;