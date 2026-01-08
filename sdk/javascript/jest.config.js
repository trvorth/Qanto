module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  testMatch: ['**/__tests__/**/*.test.ts'],
  modulePaths: ['<rootDir>/src'],
  coverageProvider: 'v8',
  collectCoverageFrom: ['src/**/*.ts', '!src/__tests__/**'],
  setupFilesAfterEnv: ['<rootDir>/jest.setup.ts'],
  coverageThreshold: {
    global: {
      lines: 60,
      functions: 60,
      statements: 60,
    },
  },
};