/**
 * Manual Jest stub for @stellar/freighter-api.
 *
 * The real package uses browser-only globals and hangs in a Node/jsdom
 * environment. This stub provides jest.fn() replacements for every export
 * used by WalletManager so unit tests run without a browser extension.
 *
 * Individual tests override these with mockResolvedValue / mockRejectedValue
 * as needed.
 */

module.exports = {
  isConnected: jest.fn().mockResolvedValue({ isConnected: false }),
  isAllowed: jest.fn().mockResolvedValue({ isAllowed: false }),
  requestAccess: jest.fn().mockResolvedValue({ publicKey: '', error: undefined }),
  getPublicKey: jest.fn().mockResolvedValue({ publicKey: '', error: undefined }),
  getNetwork: jest.fn().mockResolvedValue({ network: 'TESTNET', error: undefined }),
  getNetworkDetails: jest.fn().mockResolvedValue({
    network: 'TESTNET',
    networkUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
    error: undefined,
  }),
  signTransaction: jest.fn().mockResolvedValue({ signedTxXdr: '', error: undefined }),
  addRecentAddress: jest.fn().mockResolvedValue({}),
}
