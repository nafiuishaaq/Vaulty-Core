import { createApp } from '../../src/app';
import request from 'supertest';

jest.mock('../../src/database', () => ({
  prisma: {
    $queryRawUnsafe: jest.fn(),
  },
}));

jest.mock('../../src/config/redis', () => ({
  redis: {
    status: 'ready',
  },
}));

const mockPrisma = require('../../src/database').prisma;
const mockRedis = require('../../src/config/redis').redis;

describe('Health Check Integration Tests', () => {
  const app = createApp();

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('GET /health (liveness)', () => {
    it('should return 200 and basic liveness info', async () => {
      const response = await request(app).get('/health');

      expect(response.status).toBe(200);
      expect(response.body).toMatchObject({
        success: true,
        message: 'Vaulty Backend is running',
        uptime: expect.any(Number),
        timestamp: expect.any(String),
      });
    });
  });

  describe('GET /health/ready (readiness)', () => {
    it('should return 200 when all critical dependencies are healthy', async () => {
      mockPrisma.$queryRawUnsafe.mockResolvedValue([{ '1': 1 }]);
      mockRedis.status = 'ready';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(200);
      expect(response.body).toMatchObject({
        success: true,
        message: 'Vaulty Backend is ready',
        status: 'ok',
        timestamp: expect.any(String),
      });
      expect(response.body.checks).toMatchObject({
        prisma: { status: 'ok' },
        redis: { status: 'ok' },
      });
    });

    it('should return 503 when Prisma is unavailable', async () => {
      mockPrisma.$queryRawUnsafe.mockRejectedValue(new Error('Prisma connection timeout'));
      mockRedis.status = 'ready';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      expect(response.body).toMatchObject({
        success: false,
        message: 'Vaulty Backend is not ready',
        status: 'degraded',
        timestamp: expect.any(String),
      });
      expect(response.body.checks.prisma.status).toBe('degraded');
      expect(response.body.checks.redis.status).toBe('ok');
    });

    it('should return 503 when Redis is unavailable', async () => {
      mockPrisma.$queryRawUnsafe.mockResolvedValue([{ '1': 1 }]);
      mockRedis.status = 'close';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      expect(response.body.checks.prisma.status).toBe('ok');
      expect(response.body.checks.redis.status).toBe('degraded');
    });

    it('should return 503 when both critical dependencies are unavailable', async () => {
      mockPrisma.$queryRawUnsafe.mockRejectedValue(new Error('Prisma connection timeout'));
      mockRedis.status = 'close';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      expect(response.body.checks.prisma.status).toBe('degraded');
      expect(response.body.checks.redis.status).toBe('degraded');
    });
  });

  it('should return 404 for non-existent routes', async () => {
    const response = await request(app).get('/non-existent-route');

    expect(response.status).toBe(404);
    expect(response.body).toHaveProperty('success', false);
    expect(response.body).toHaveProperty('message');
  });
});