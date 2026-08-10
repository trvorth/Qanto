// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract QantoTGE is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    IERC20 public immutable qntoToken;
    uint256 public constant RATE = 10000; // 1 ETH = 10,000 QNTO
    uint256 public constant QNTO_SCALE = 1e9;
    uint256 private constant WEI_PER_ETH = 1e18;
    uint256 public totalRaised;
    // Fail closed: the owner must explicitly start the sale after funding and
    // after the project adopts an audited vesting/distribution policy.
    bool public isPresaleActive;

    event TokensPurchased(address indexed buyer, uint256 ethSpent, uint256 qntoReceived);

    constructor(address _token) Ownable(msg.sender) {
        require(_token != address(0), "TGE token is zero address.");
        qntoToken = IERC20(_token);
    }

    function buyTokens() external payable nonReentrant {
        require(isPresaleActive, "TGE is not active.");
        require(msg.value > 0, "Must send ETH.");

        uint256 tokensToTransfer = (msg.value * RATE * QNTO_SCALE) / WEI_PER_ETH;
        require(tokensToTransfer > 0, "Purchase rounds to zero QNTO.");
        require(qntoToken.balanceOf(address(this)) >= tokensToTransfer, "Insufficient QNTO in TGE contract.");

        totalRaised += msg.value;
        qntoToken.safeTransfer(msg.sender, tokensToTransfer);

        emit TokensPurchased(msg.sender, msg.value, tokensToTransfer);
    }

    function togglePresale() external onlyOwner {
        isPresaleActive = !isPresaleActive;
    }

    function withdrawETH() external onlyOwner {
        (bool success, ) = payable(owner()).call{value: address(this).balance}("");
        require(success, "ETH withdrawal failed.");
    }
}
