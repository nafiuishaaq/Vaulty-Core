/** @type {import('jest').Config} */
const config = {
  preset: 'ts-jest',
  testEnvironment: 'jest-environment-jsdom',
  roots: ['<rootDir>/src'],
  testMatch: ['**/__tests__/**/*.{ts,tsx}', '**/?(*.)+(spec|test).{ts,tsx}'],
  transform: {
    '^.+\\.(ts|tsx)$': ['ts-jest', {
      tsconfig: {
        jsx: 'react-jsx',
      },
    }],
  },
  moduleNameMapper: {
    // Handle Next.js path alias
    '^@/(.*)$': '<rootDir>/src/$1',
    // Stub CSS / image imports that Jest cannot process
    '\\.(css|less|scss|sass)$': '<rootDir>/src/__mocks__/styleMock.js',
    '\\.(jpg|jpeg|png|gif|svg|webp)$': '<rootDir>/src/__mocks__/fileMock.js',
    // @stellar/freighter-api uses browser-only globals (window.postMessage, etc.)
    // and hangs in Node/jsdom. Always replace it with a lightweight manual stub.
    '^@stellar/freighter-api$': '<rootDir>/src/__mocks__/freighter-api.js',
  },
  setupFilesAfterEnv: ['<rootDir>/src/test-setup.ts'],
  collectCoverageFrom: [
    'src/**/*.{ts,tsx}',
    '!src/**/*.d.ts',
    '!src/app/**',          // Next.js pages/layouts — UI integration tests out of scope here
    '!src/__mocks__/**',
    '!src/test-setup.ts',
  ],
  coverageDirectory: 'coverage',
  coverageReporters: ['text', 'lcov', 'html'],
  testTimeout: 10000,
}

module.exports = config
