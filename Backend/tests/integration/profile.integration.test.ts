import request from 'supertest';
import { app } from '../../src/app';
import { prisma } from '../../src/database';
import { hashPassword } from '../../src/utils/crypto';
import { normalizePhoneNumber } from '../../src/utils/identity';

describe('Profile Integration Tests', () => {
  let authToken: string;
  let userId: string;
  const testEmail = 'testprofile@example.com';
  const testPassword = 'Test12345';

  beforeEach(async () => {
    // Clean up existing user
    await prisma.user.deleteMany({ where: { email: testEmail } });
    
    // Create test user
    const passwordHash = await hashPassword(testPassword);
    const user = await prisma.user.create({
      data: {
        email: testEmail,
        passwordHash,
        firstName: 'Original',
        lastName: 'Name',
        phoneNumber: null,
      },
    });
    userId = user.id;

    // Login to get auth token
    const loginResponse = await request(app)
      .post('/api/auth/login')
      .send({ email: testEmail, password: testPassword });
    
    authToken = loginResponse.body.data.accessToken;
  });

  afterEach(async () => {
    await prisma.user.deleteMany({ where: { email: testEmail } });
  });

  describe('GET /api/auth/profile', () => {
    it('should return user profile when authenticated', async () => {
      const response = await request(app)
        .get('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`);

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      expect(response.body.data.user.email).toBe(testEmail);
      expect(response.body.data.user.firstName).toBe('Original');
      expect(response.body.data.user.passwordHash).toBeUndefined();
    });

    it('should reject unauthenticated requests', async () => {
      const response = await request(app).get('/api/auth/profile');
      expect(response.status).toBe(401);
    });
  });

  describe('PUT /api/auth/profile', () => {
    it('should update first and last name successfully', async () => {
      const response = await request(app)
        .put('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ firstName: 'Updated', lastName: 'User' });

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      expect(response.body.data.user.firstName).toBe('Updated');
      expect(response.body.data.user.lastName).toBe('User');
    });

    it('should update phone number successfully', async () => {
      const phoneNumber = '08012345678';
      const normalizedPhone = normalizePhoneNumber(phoneNumber);
      
      const response = await request(app)
        .put('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ phoneNumber });

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      expect(response.body.data.user.phoneNumber).toBe(normalizedPhone);
    });

    it('should reject duplicate phone number', async () => {
      // Create second user
      const secondEmail = 'seconduser@example.com';
      const secondPhone = '08098765432';
      const normalizedSecondPhone = normalizePhoneNumber(secondPhone);
      
      await prisma.user.create({
        data: {
          email: secondEmail,
          passwordHash: await hashPassword('Test12345'),
          phoneNumber: normalizedSecondPhone,
        },
      });

      // Try to update first user's phone to the same number
      const response = await request(app)
        .put('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ phoneNumber: secondPhone });

      expect(response.status).toBe(409);
      expect(response.body.success).toBe(false);
      expect(response.body.message).toBe('User with this phone number already exists');

      // Clean up second user
      await prisma.user.deleteMany({ where: { email: secondEmail } });
    });

    it('should allow updating to the same phone number (no conflict)', async () => {
      const phoneNumber = '08012345678';
      const normalizedPhone = normalizePhoneNumber(phoneNumber);
      
      // First set the phone number
      await prisma.user.update({
        where: { id: userId },
        data: { phoneNumber: normalizedPhone },
      });

      // Try to update to the same number again
      const response = await request(app)
        .put('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ phoneNumber });

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
    });

    it('should reject invalid phone number', async () => {
      const response = await request(app)
        .put('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ phoneNumber: 'invalidphone' });

      expect(response.status).toBe(400);
      expect(response.body.success).toBe(false);
    });

    it('should return the same safe user shape as GET profile', async () => {
      const response = await request(app)
        .put('/api/auth/profile')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ firstName: 'TestUpdate' });

      expect(response.status).toBe(200);
      expect(response.body.data.user.passwordHash).toBeUndefined();
      expect(response.body.data.user.id).toBeDefined();
      expect(response.body.data.user.email).toBeDefined();
      expect(response.body.data.user.createdAt).toBeDefined();
      expect(response.body.data.user.updatedAt).toBeDefined();
    });
  });
});