// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title QantoFaucet
 * @dev Genesis Asset Faucet for QANTO Layer-0.
 * In a production Genesis 2.0 state, this is part of the zero-layer consensus.
 */
contract QantoFaucet {
    string public name = "Qanto Genesis Token";
    string public symbol = "eQNTO";
    uint8 public decimals = 18;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => uint256) public lastClaimTime;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event GenesisMint(address indexed to, uint256 amount);

    uint256 public constant MINT_AMOUNT = 100 * 10**18;
    uint256 public constant CLAIM_INTERVAL = 24 hours;

    function mintGenesis(address to) external {
        require(block.timestamp >= lastClaimTime[to] + CLAIM_INTERVAL, "Wait for next cycle");
        
        balanceOf[to] += MINT_AMOUNT;
        totalSupply += MINT_AMOUNT;
        lastClaimTime[to] = block.timestamp;
        
        emit GenesisMint(to, MINT_AMOUNT);
        emit Transfer(address(0), to, MINT_AMOUNT);
    }

    function getBalance(address account) external view returns (uint256) {
        return balanceOf[account];
    }
}
