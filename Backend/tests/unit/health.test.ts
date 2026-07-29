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

describe('Health Check Endpoints', () => {
  const app = createApp();

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('GET /health (liveness)', () => {
    it('should return 200 with uptime and no dependency checks', async () => {
      const response = await request(app).get('/health');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('success', true);
      expect(response.body).toHaveProperty('uptime');
      expect(response.body).toHaveProperty('timestamp');
      expect(response.body).not.toHaveProperty('checks');
      expect(response.body).not.toHaveProperty('ready');
      expect(response.body).not.toHaveProperty('status');
    });
  });

  describe('GET /health/ready (readiness)', () => {
    it('should return 200 when Prisma and Redis are healthy', async () => {
      mockPrisma.$queryRawUnsafe.mockResolvedValue([{ '1': 1 }]);
      mockRedis.status = 'ready';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('success', true);
      expect(response.body).toHaveProperty('status', 'ok');
      expect(response.body).toHaveProperty('checks');
      expect(response.body.checks.prisma).toHaveProperty('status', 'ok');
      expect(response.body.checks.redis).toHaveProperty('status', 'ok');
    });

    it('should return 503 when Prisma is unavailable', async () => {
      mockPrisma.$queryRawUnsafe.mockRejectedValue(new Error('Connection refused'));
      mockRedis.status = 'ready';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      expect(response.body).toHaveProperty('success', false);
      expect(response.body).toHaveProperty('status', 'degraded');
      expect(response.body.checks.prisma).toHaveProperty('status', 'degraded');
      expect(response.body.checks.redis).toHaveProperty('status', 'ok');
    });

    it('should return 503 when Redis is unavailable', async () => {
      mockPrisma.$queryRawUnsafe.mockResolvedValue([{ '1': 1 }]);
      mockRedis.status = 'close';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      expect(response.body).toHaveProperty('success', false);
      expect(response.body).toHaveProperty('status', 'degraded');
      expect(response.body.checks.prisma).toHaveProperty('status', 'ok');
      expect(response.body.checks.redis).toHaveProperty('status', 'degraded');
    });

    it('should return 503 when both Prisma and Redis are unavailable', async () => {
      mockPrisma.$queryRawUnsafe.mockRejectedValue(new Error('Connection refused'));
      mockRedis.status = 'close';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      expect(response.body).toHaveProperty('success', false);
      expect(response.body).toHaveProperty('status', 'degraded');
      expect(response.body.checks.prisma).toHaveProperty('status', 'degraded');
      expect(response.body.checks.redis).toHaveProperty('status', 'degraded');
    });

    it('should not expose connection strings or secrets in error details', async () => {
      mockPrisma.$queryRawUnsafe.mockRejectedValue(
        new Error('Connection refused: postgres://user:secret123@host/db')
      );
      mockRedis.status = 'ready';

      const response = await request(app).get('/health/ready');

      expect(response.status).toBe(503);
      const details = response.body.checks.prisma.details;
      expect(details).not.toContain('secret123');
    });
  });
});