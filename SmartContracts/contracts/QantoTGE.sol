// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract QantoTGE is Ownable, ReentrancyGuard {
    IERC20 public immutable qntoToken;
    uint256 public constant RATE = 10000; // 1 ETH = 10,000 QNTO
    uint256 public totalRaised;
    bool public isPresaleActive = true;

    event TokensPurchased(address indexed buyer, uint256 ethSpent, uint256 qntoReceived);

    constructor(address _token) Ownable(msg.sender) {
        qntoToken = IERC20(_token);
    }

    function buyTokens() external payable nonReentrant {
        require(isPresaleActive, "TGE is not active.");
        require(msg.value > 0, "Must send ETH.");

        uint256 tokensToTransfer = msg.value * RATE;
        require(qntoToken.balanceOf(address(this)) >= tokensToTransfer, "Insufficient QNTO in TGE contract.");

        totalRaised += msg.value;
        qntoToken.transfer(msg.sender, tokensToTransfer);

        emit TokensPurchased(msg.sender, msg.value, tokensToTransfer);
    }

    function togglePresale() external onlyOwner {
        isPresaleActive = !isPresaleActive;
    }

    function withdrawETH() external onlyOwner {
        payable(owner()).transfer(address(this).balance);
    }
}
