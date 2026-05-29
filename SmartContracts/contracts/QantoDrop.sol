// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract QantoDrop is Ownable, ReentrancyGuard {
    IERC20 public immutable qntoToken;
    bytes32 public merkleRoot;
    mapping(address => bool) public hasClaimed;

    event Claimed(address indexed claimant, uint256 amount);
    event MerkleRootUpdated(bytes32 newRoot);

    constructor(address _token, bytes32 _merkleRoot) Ownable(msg.sender) {
        qntoToken = IERC20(_token);
        merkleRoot = _merkleRoot;
    }

    function updateMerkleRoot(bytes32 _merkleRoot) external onlyOwner {
        merkleRoot = _merkleRoot;
        emit MerkleRootUpdated(_merkleRoot);
    }

    function claim(uint256 amount, bytes32[] calldata merkleProof) external nonReentrant {
        require(!hasClaimed[msg.sender], "QantoDrop: Airdrop already claimed.");
        
        // Verify the merkle proof
        bytes32 node = keccak256(abi.encodePacked(msg.sender, amount));
        require(MerkleProof.verify(merkleProof, merkleRoot, node), "QantoDrop: Invalid proof.");

        hasClaimed[msg.sender] = true;
        require(qntoToken.transfer(msg.sender, amount), "QantoDrop: Transfer failed.");
        
        emit Claimed(msg.sender, amount);
    }

    function withdrawRemaining() external onlyOwner {
        uint256 balance = qntoToken.balanceOf(address(this));
        qntoToken.transfer(owner(), balance);
    }
}
