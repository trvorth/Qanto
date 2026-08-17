// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/access/Ownable2Step.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title QantoFaucet
 * @notice Testnet-only faucet for eQNTO genesis tokens.
 *
 * Security controls:
 *  - Owner-gated minting (Ownable2Step for safe transfer)
 *  - Hard supply cap (MAX_SUPPLY) prevents unbounded inflation
 *  - Per-address daily rate limit (CLAIM_INTERVAL + MINT_AMOUNT)
 *  - Global daily drip limit (MAX_DAILY_MINT) prevents bulk abuse
 *  - Pause circuit-breaker for emergency shutdown
 *  - Reentrancy guard on stateful methods
 *  - Production-network guard: reverts on chain IDs 1 (Mainnet) and 137 (Polygon)
 *
 * Deployment policy: MUST NOT be deployed on production networks.
 * Enforced at constructor time via network guard.
 */
contract QantoFaucet is Ownable2Step, Pausable, ReentrancyGuard {
    // ─── Token metadata ──────────────────────────────────────────────────────
    string public constant name     = "Qanto Genesis Token";
    string public constant symbol   = "eQNTO";
    uint8  public constant decimals = 18;

    // ─── Supply controls ─────────────────────────────────────────────────────
    /// @notice Hard cap: 10 million eQNTO total supply
    uint256 public constant MAX_SUPPLY      = 10_000_000 * 10 ** 18;
    /// @notice Per-drip amount per address per cooldown window
    uint256 public constant MINT_AMOUNT     = 100 * 10 ** 18;
    /// @notice Minimum time between successive claims per address
    uint256 public constant CLAIM_INTERVAL  = 24 hours;
    /// @notice Maximum tokens that can be minted globally in any 24 h window
    uint256 public constant MAX_DAILY_MINT  = 500_000 * 10 ** 18;

    // ─── State ────────────────────────────────────────────────────────────────
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => uint256) public lastClaimTime;

    // Daily drip accounting
    uint256 private _dailyWindowStart;
    uint256 private _dailyMintedThisWindow;

    // ─── Events ───────────────────────────────────────────────────────────────
    event Transfer(address indexed from, address indexed to, uint256 value);
    event Claimed(address indexed to, uint256 amount, uint256 newTotalSupply);
    event OwnerMinted(address indexed to, uint256 amount);

    // ─── Errors ───────────────────────────────────────────────────────────────
    error ProductionNetworkForbidden(uint256 chainId);
    error SupplyCapExceeded(uint256 requested, uint256 remaining);
    error DailyLimitExceeded(uint256 requested, uint256 remaining);
    error CooldownNotExpired(uint256 availableAt);
    error ZeroAddress();

    // ─── Constructor ──────────────────────────────────────────────────────────

    constructor() Ownable(msg.sender) {
        // Refuse deployment on known production chain IDs
        uint256 chainId = block.chainid;
        if (chainId == 1 || chainId == 137) {
            revert ProductionNetworkForbidden(chainId);
        }
        _dailyWindowStart = block.timestamp;
    }

    // ─── Public claim ─────────────────────────────────────────────────────────

    /**
     * @notice Claim MINT_AMOUNT eQNTO once per CLAIM_INTERVAL.
     * @dev Anyone can call for themselves; to claim for another address use
     *      `claimFor`. Both are rate-limited per recipient address.
     */
    function claim() external whenNotPaused nonReentrant {
        _drip(msg.sender);
    }

    /**
     * @notice Claim MINT_AMOUNT eQNTO for a specific recipient.
     * @param to Recipient address (must be non-zero).
     */
    function claimFor(address to) external whenNotPaused nonReentrant {
        if (to == address(0)) revert ZeroAddress();
        _drip(to);
    }

    // ─── Owner-only controls ─────────────────────────────────────────────────

    /**
     * @notice Owner-only mint for test bootstrapping. Subject to supply cap.
     * @param to    Recipient address.
     * @param amount Tokens to mint (in wei).
     */
    function ownerMint(address to, uint256 amount)
        external
        onlyOwner
        whenNotPaused
        nonReentrant
    {
        if (to == address(0)) revert ZeroAddress();
        uint256 remaining = MAX_SUPPLY - totalSupply;
        if (amount > remaining) revert SupplyCapExceeded(amount, remaining);

        unchecked {
            balanceOf[to] += amount;
            totalSupply    += amount;
        }
        emit OwnerMinted(to, amount);
        emit Transfer(address(0), to, amount);
    }

    /// @notice Pause the faucet. Only owner.
    function pause() external onlyOwner { _pause(); }

    /// @notice Unpause the faucet. Only owner.
    function unpause() external onlyOwner { _unpause(); }

    // ─── View helpers ─────────────────────────────────────────────────────────

    /// @notice Returns the token balance of `account`.
    function getBalance(address account) external view returns (uint256) {
        return balanceOf[account];
    }

    /// @notice Returns remaining supply headroom before cap.
    function remainingSupply() external view returns (uint256) {
        return MAX_SUPPLY - totalSupply;
    }

    /// @notice Returns the timestamp when `account` can next claim.
    function nextClaimTime(address account) external view returns (uint256) {
        return lastClaimTime[account] + CLAIM_INTERVAL;
    }

    // ─── Internal ─────────────────────────────────────────────────────────────

    function _drip(address to) internal {
        // Per-address cooldown
        uint256 available = lastClaimTime[to] + CLAIM_INTERVAL;
        if (block.timestamp < available) revert CooldownNotExpired(available);

        // Hard supply cap
        uint256 remaining = MAX_SUPPLY - totalSupply;
        if (MINT_AMOUNT > remaining) revert SupplyCapExceeded(MINT_AMOUNT, remaining);

        // Global daily drip limit
        _refreshDailyWindow();
        uint256 dailyRemaining = MAX_DAILY_MINT - _dailyMintedThisWindow;
        if (MINT_AMOUNT > dailyRemaining) revert DailyLimitExceeded(MINT_AMOUNT, dailyRemaining);

        // Commit state
        lastClaimTime[to]          = block.timestamp;
        _dailyMintedThisWindow    += MINT_AMOUNT;
        unchecked {
            balanceOf[to] += MINT_AMOUNT;
            totalSupply    += MINT_AMOUNT;
        }

        emit Claimed(to, MINT_AMOUNT, totalSupply);
        emit Transfer(address(0), to, MINT_AMOUNT);
    }

    function _refreshDailyWindow() internal {
        if (block.timestamp >= _dailyWindowStart + 24 hours) {
            _dailyWindowStart      = block.timestamp;
            _dailyMintedThisWindow = 0;
        }
    }
}
