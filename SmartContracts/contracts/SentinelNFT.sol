// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract SentinelNFT is ERC721URIStorage, Ownable {
    uint256 private _tokenIds;
    
    uint256 public constant MAX_SUPPLY = 10000;

    constructor() ERC721("Qanto Sentinel Matrix", "QSEN") Ownable(msg.sender) {}

    function mintSentinel(address recipient, string memory tokenURI) public onlyOwner returns (uint256) {
        require(_tokenIds < MAX_SUPPLY, "SentinelNFT: Max supply reached");
        
        _tokenIds++;
        uint256 newItemId = _tokenIds;
        
        _mint(recipient, newItemId);
        _setTokenURI(newItemId, tokenURI);
        
        return newItemId;
    }
}
