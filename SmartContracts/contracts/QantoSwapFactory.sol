// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract QantoSwapFactory {
    error AmmNotImplemented();

    address public feeTo;
    address public feeToSetter;
    mapping(address => mapping(address => address)) public getPair;
    address[] public allPairs;

    event PairCreated(address indexed token0, address indexed token1, address pair, uint);

    constructor(address _feeToSetter) {
        feeToSetter = _feeToSetter;
    }

    function createPair(address tokenA, address tokenB) external pure returns (address pair) {
        tokenA;
        tokenB;
        pair;
        // The pair contract has no liquidity, reserve-update, LP-token, or
        // invariant logic. Refuse deployment instead of creating a fund trap.
        revert AmmNotImplemented();
    }
}

contract QantoSwapPair {
    address public factory;
    address public token0;
    address public token1;
    uint112 private reserve0;
    uint112 private reserve1;
    uint32  private blockTimestampLast;
    uint public kLast;

    constructor() { factory = msg.sender; }

    function initialize(address _token0, address _token1) external {
        require(msg.sender == factory, 'QantoSwap: FORBIDDEN');
        token0 = _token0;
        token1 = _token1;
    }
    // Minimal AMM Pair structure to validate compiling capability on Qanto Testnet
}
