// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract WQNTO is ERC20, ReentrancyGuard {
    event Deposit(address indexed dst, uint wad);
    event Withdrawal(address indexed src, uint wad);

    constructor() ERC20("Wrapped QANTO", "WQNTO") {}

    function decimals() public view virtual override returns (uint8) {
        return 9; // QANTO Layer-0 9-Decimal Standard
    }

    receive() external payable {
        deposit();
    }

    function deposit() public payable {
        _mint(msg.sender, msg.value);
        emit Deposit(msg.sender, msg.value);
    }

    function withdraw(uint wad) public nonReentrant {
        require(balanceOf(msg.sender) >= wad, "WQNTO: Insufficient balance");
        _burn(msg.sender, wad);
        (bool success, ) = payable(msg.sender).call{value: wad}("");
        require(success, "WQNTO: Native transfer failed");
        emit Withdrawal(msg.sender, wad);
    }
}
