// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
contract QUSD is ERC20 {
    constructor() ERC20("Qanto USD", "QUSD") {
        _mint(msg.sender, 1000000000 * 10 ** decimals()); // 1B QUSD
    }
}
