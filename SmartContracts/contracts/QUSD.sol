// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Pausable.sol";

/// @notice Controlled QUSD token primitive.
/// @dev This contract deliberately does not claim to implement a dollar peg.
/// Collateral custody, oracle validation, liquidation, and redemption must be
/// specified and audited before QUSD can be deployed as a stablecoin.
contract QUSD is ERC20, ERC20Burnable, ERC20Pausable, AccessControl {
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");

    uint256 public constant MAX_SUPPLY = 1_000_000_000 ether;

    error ZeroAddress();
    error ZeroAmount();
    error SupplyCapExceeded(uint256 requested, uint256 remaining);

    constructor(address admin, address initialMinter) ERC20("Qanto USD", "QUSD") {
        if (admin == address(0) || initialMinter == address(0)) revert ZeroAddress();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MINTER_ROLE, initialMinter);
        _grantRole(PAUSER_ROLE, admin);
        _grantRole(BURNER_ROLE, admin);
    }

    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) whenNotPaused {
        if (to == address(0)) revert ZeroAddress();
        if (amount == 0) revert ZeroAmount();
        uint256 remaining = MAX_SUPPLY - totalSupply();
        if (amount > remaining) revert SupplyCapExceeded(amount, remaining);
        _mint(to, amount);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function remainingMintable() external view returns (uint256) {
        return MAX_SUPPLY - totalSupply();
    }

    function burnFrom(address account, uint256 amount) public override {
        if (hasRole(BURNER_ROLE, msg.sender)) {
            _burn(account, amount);
        } else {
            super.burnFrom(account, amount);
        }
    }

    function _update(address from, address to, uint256 value)
        internal
        override(ERC20, ERC20Pausable)
    {
        super._update(from, to, value);
    }
}
