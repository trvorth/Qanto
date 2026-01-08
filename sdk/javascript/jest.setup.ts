// Global Jest setup to minimize lingering async handles between tests.

afterEach(() => {
  // Clear any pending fake timers to avoid open handle warnings.
  try {
    jest.clearAllTimers();
  } catch (_) {
    // noop if timers aren’t faked
  }
  // Restore all spies/mocks to avoid cross-test leakage.
  jest.restoreAllMocks();
});