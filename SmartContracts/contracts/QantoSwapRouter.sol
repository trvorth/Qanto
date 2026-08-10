// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IQantoSwapFactory { function getPair(address tokenA, address tokenB) external view returns (address pair); }

contract QantoSwapRouter {
    error AmmNotImplemented();

    address public immutable factory;
    address public immutable WQNTO;

    constructor(address _factory, address _WQNTO) {
        require(_factory != address(0), 'QantoSwap: ZERO_FACTORY');
        require(_WQNTO != address(0), 'QantoSwap: ZERO_WQNTO');
        factory = _factory;
        WQNTO = _WQNTO;
    }

    receive() external payable {
        assert(msg.sender == WQNTO); // only accept QNTO via fallback from the WQNTO contract
    }

    function swapExactTokensForTokens(
        uint amountIn,
        uint amountOutMin,
        address[] calldata path,
        address to,
        uint deadline
    ) external view returns (uint[] memory) {
        require(path.length >= 2, 'QantoSwap: INVALID_PATH');
        require(deadline >= block.timestamp, 'QantoSwap: EXPIRED');
        amountIn;
        amountOutMin;
        to;
        // Never report a successful swap until transfers, reserves, LP shares,
        // slippage, and the constant-product invariant are fully implemented.
        revert AmmNotImplemented();
    }
}
